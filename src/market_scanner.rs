// Server-Side Market Scanner Module
// Runs independently on the server, continues even when browser is closed

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

use crate::firestore_manager::{GlobalFirestore, ScanRecord};

/// Scanner configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanConfig {
    /// Scan interval in seconds
    pub interval_seconds: u64,
    /// Candle timeframe (granularity) in seconds
    pub candle_timeframe: u64,
    /// Indicator period for CI and ADX
    pub indicator_period: usize,
    /// Stop time (ISO format), empty = run forever until manual stop
    pub stop_time: Option<String>,
    /// Save to Firestore
    #[serde(default = "default_true")]
    pub save_to_firestore: bool,
    /// List of assets to scan
    pub assets: Vec<AssetConfig>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetConfig {
    pub symbol: String,
    pub name: String,
}

/// Scan result for a single asset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetScanResult {
    pub symbol: String,
    pub price: f64,
    pub ci: f64,
    pub adx: f64,
    pub score: f64,
    pub is_bullish: bool,
    pub recent_candles: String,
    pub scan_time: String,
    pub rank: u32,
}

/// Scanner status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScannerStatus {
    pub is_running: bool,
    pub total_scans: u64,
    pub total_records_saved: u64,
    pub last_scan_time: Option<String>,
    pub next_scan_time: Option<String>,
    pub config: Option<ScanConfig>,
    pub last_results: Vec<AssetScanResult>,
    pub errors: Vec<String>,
}

impl Default for ScannerStatus {
    fn default() -> Self {
        Self {
            is_running: false,
            total_scans: 0,
            total_records_saved: 0,
            last_scan_time: None,
            next_scan_time: None,
            config: None,
            last_results: Vec::new(),
            errors: Vec::new(),
        }
    }
}

/// Main Scanner struct
pub struct MarketScanner {
    status: Arc<RwLock<ScannerStatus>>,
    task_handle: Arc<RwLock<Option<JoinHandle<()>>>>,
    firestore: Arc<tokio::sync::Mutex<GlobalFirestore>>,
}

impl MarketScanner {
    pub fn new(firestore: Arc<tokio::sync::Mutex<GlobalFirestore>>) -> Self {
        Self {
            status: Arc::new(RwLock::new(ScannerStatus::default())),
            task_handle: Arc::new(RwLock::new(None)),
            firestore,
        }
    }

    /// Get current status
    pub async fn get_status(&self) -> ScannerStatus {
        self.status.read().await.clone()
    }

    /// Start scanning with given config
    pub async fn start(&self, config: ScanConfig) -> Result<(), String> {
        // Check if already running
        {
            let status = self.status.read().await;
            if status.is_running {
                return Err("Scanner is already running".to_string());
            }
        }

        // Update status
        {
            let mut status = self.status.write().await;
            status.is_running = true;
            status.config = Some(config.clone());
            status.total_scans = 0;
            status.total_records_saved = 0;
            status.errors.clear();
            status.last_results.clear();
        }

        // Clone necessary handles for the async task
        let status_handle = self.status.clone();
        let firestore_handle = self.firestore.clone();
        let config_clone = config.clone();

        // Spawn the scanning task
        let handle = tokio::spawn(async move {
            run_scanner_loop(status_handle, firestore_handle, config_clone).await;
        });

        // Store the task handle
        {
            let mut task = self.task_handle.write().await;
            *task = Some(handle);
        }

        println!(
            "🚀 Market Scanner started with interval: {}s",
            config.interval_seconds
        );
        Ok(())
    }

    /// Stop scanning
    pub async fn stop(&self) -> Result<(), String> {
        {
            let status = self.status.read().await;
            if !status.is_running {
                return Err("Scanner is not running".to_string());
            }
        }

        // Abort the task
        {
            let mut task = self.task_handle.write().await;
            if let Some(handle) = task.take() {
                handle.abort();
            }
        }

        // Update status
        {
            let mut status = self.status.write().await;
            status.is_running = false;
            status.next_scan_time = None;
        }

        println!("⏹️ Market Scanner stopped");
        Ok(())
    }
}

/// Main scanning loop
async fn run_scanner_loop(
    status: Arc<RwLock<ScannerStatus>>,
    firestore: Arc<tokio::sync::Mutex<GlobalFirestore>>,
    config: ScanConfig,
) {
    let interval = tokio::time::Duration::from_secs(config.interval_seconds);

    loop {
        // Check stop time
        if let Some(ref stop_time_str) = config.stop_time {
            // Try to parse the stop time - it should be in ISO 8601 format
            println!(
                "🕐 Checking stop time: {} vs now: {}",
                stop_time_str,
                Utc::now()
            );
            if let Ok(stop_time) = DateTime::parse_from_rfc3339(stop_time_str) {
                let stop_utc = stop_time.with_timezone(&Utc);
                let now = Utc::now();
                println!("🕐 Parsed stop time: {} vs now: {}", stop_utc, now);
                if now >= stop_utc {
                    println!("⏰ Stop time reached. Stopping scanner...");
                    let mut s = status.write().await;
                    s.is_running = false;
                    s.next_scan_time = None;
                    break;
                }
            } else {
                println!("⚠️ Could not parse stop time: {}", stop_time_str);
            }
        }

        // Calculate next scan time
        {
            let mut s = status.write().await;
            let next = Utc::now() + chrono::Duration::seconds(config.interval_seconds as i64);
            s.next_scan_time = Some(next.to_rfc3339());
        }

        // Perform scan
        match perform_scan(&config, &status, &firestore).await {
            Ok(results) => {
                let mut s = status.write().await;
                s.total_scans += 1;
                s.total_records_saved += results.len() as u64;
                s.last_scan_time = Some(Utc::now().to_rfc3339());
                s.last_results = results;
                println!(
                    "✅ Scan #{} completed. {} records saved.",
                    s.total_scans,
                    s.last_results.len()
                );
            }
            Err(e) => {
                let mut s = status.write().await;
                s.errors
                    .push(format!("{}: {}", Utc::now().format("%H:%M:%S"), e));
                // Keep only last 10 errors
                if s.errors.len() > 10 {
                    s.errors.remove(0);
                }
                println!("❌ Scan error: {}", e);
            }
        }

        // Wait for next interval
        tokio::time::sleep(interval).await;
    }
}

/// Perform a single scan of all assets
async fn perform_scan(
    config: &ScanConfig,
    _status: &Arc<RwLock<ScannerStatus>>,
    firestore: &Arc<tokio::sync::Mutex<GlobalFirestore>>,
) -> Result<Vec<AssetScanResult>, String> {
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::connect_async;
    use tokio_tungstenite::tungstenite::Message;

    let url = "wss://ws.derivws.com/websockets/v3?app_id=66726";

    let (ws_stream, _) = connect_async(url)
        .await
        .map_err(|e| format!("WebSocket connection failed: {}", e))?;

    let (mut write, mut read) = ws_stream.split();

    let mut results: Vec<AssetScanResult> = Vec::new();
    let scan_time = Utc::now().to_rfc3339();

    for asset in &config.assets {
        // Request candle history
        let req = serde_json::json!({
            "ticks_history": asset.symbol,
            "adjust_start_time": 1,
            "count": 100,
            "end": "latest",
            "style": "candles",
            "granularity": config.candle_timeframe
        });

        write
            .send(Message::Text(req.to_string()))
            .await
            .map_err(|e| format!("Failed to send request: {}", e))?;

        // Wait for response with timeout
        let timeout = tokio::time::Duration::from_secs(10);
        let response = tokio::time::timeout(timeout, read.next())
            .await
            .map_err(|_| format!("Timeout waiting for {} data", asset.symbol))?
            .ok_or_else(|| format!("No response for {}", asset.symbol))?
            .map_err(|e| format!("WebSocket error: {}", e))?;

        if let Message::Text(text) = response {
            let json: serde_json::Value =
                serde_json::from_str(&text).map_err(|e| format!("JSON parse error: {}", e))?;

            if let Some(_error) = json.get("error") {
                continue; // Skip this asset but continue with others
            }

            if let Some(candles) = json.get("candles").and_then(|c| c.as_array()) {
                let result = calculate_indicators(
                    candles,
                    &asset.symbol,
                    &scan_time,
                    config.indicator_period,
                );
                results.push(result);
            }
        }

        // Small delay between requests to avoid rate limiting
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    }

    // Close WebSocket
    let _ = write.close().await;

    // Sort results by score (highest = best, rank 1)
    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Assign ranks after sorting
    for (index, result) in results.iter_mut().enumerate() {
        result.rank = (index + 1) as u32;
    }

    // Save to Firestore with ranks
    if config.save_to_firestore {
        let fs = firestore.lock().await;
        for result in &results {
            let record = ScanRecord {
                scan_time: result.scan_time.clone(),
                timeframe: config.candle_timeframe.to_string(),
                period: config.indicator_period.to_string(),
                symbol: result.symbol.clone(),
                price: result.price,
                ci: result.ci,
                adx: result.adx,
                score: result.score,
                is_bullish: result.is_bullish,
                recent_candles: result.recent_candles.clone(),
                rank: result.rank,
            };
            let _ = fs.save_scan(&record).await;
        }
        println!("🔥 Saved {} scan records to Firestore", results.len());
    } else {
        println!("⚠️ save_to_firestore is false -> Skipping Firestore save");
    }

    Ok(results)
}

/// Calculate CI, ADX, and other indicators from candle data
fn calculate_indicators(
    candles: &[serde_json::Value],
    symbol: &str,
    scan_time: &str,
    period: usize,
) -> AssetScanResult {
    let closes: Vec<f64> = candles
        .iter()
        .filter_map(|c| c.get("close").and_then(|v| v.as_f64()))
        .collect();
    let highs: Vec<f64> = candles
        .iter()
        .filter_map(|c| c.get("high").and_then(|v| v.as_f64()))
        .collect();
    let lows: Vec<f64> = candles
        .iter()
        .filter_map(|c| c.get("low").and_then(|v| v.as_f64()))
        .collect();
    let opens: Vec<f64> = candles
        .iter()
        .filter_map(|c| c.get("open").and_then(|v| v.as_f64()))
        .collect();

    let ci = calculate_ci(&highs, &lows, &closes, period);
    let adx = calculate_adx(&highs, &lows, &closes, period);
    let score = adx + (100.0 - ci);

    let latest_close = *closes.last().unwrap_or(&0.0);
    let latest_open = *opens.last().unwrap_or(&0.0);
    let is_bullish = latest_close >= latest_open;

    // Get last 10 candle colors
    let recent: Vec<&str> = closes
        .iter()
        .rev()
        .take(10)
        .zip(opens.iter().rev().take(10))
        .map(|(c, o)| if c >= o { "up" } else { "down" })
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    AssetScanResult {
        symbol: symbol.to_string(),
        price: latest_close,
        ci,
        adx,
        score,
        is_bullish,
        recent_candles: recent.join(","),
        scan_time: scan_time.to_string(),
        rank: 0, // Will be assigned after sorting
    }
}

/// Calculate Choppiness Index
fn calculate_ci(high: &[f64], low: &[f64], close: &[f64], period: usize) -> f64 {
    if high.len() < period + 1 || low.len() < period + 1 || close.len() < period + 1 {
        return 50.0;
    }

    let tr = calculate_tr(high, low, close);
    let len = tr.len();

    if len < period {
        return 50.0;
    }

    // Sum TR for last period
    let sum_tr: f64 = tr.iter().rev().take(period).sum();

    // Max high and min low for period
    let max_high = high
        .iter()
        .rev()
        .take(period)
        .fold(f64::NEG_INFINITY, |a, &b| a.max(b));
    let min_low = low
        .iter()
        .rev()
        .take(period)
        .fold(f64::INFINITY, |a, &b| a.min(b));

    let range = max_high - min_low;
    if range == 0.0 {
        return 50.0;
    }

    100.0 * (sum_tr / range).log10() / (period as f64).log10()
}

/// Calculate True Range
fn calculate_tr(high: &[f64], low: &[f64], close: &[f64]) -> Vec<f64> {
    let mut tr = Vec::new();
    for i in 0..high.len() {
        if i == 0 {
            tr.push(high[i] - low[i]);
        } else {
            let h = high[i];
            let l = low[i];
            let pc = close[i - 1];
            tr.push((h - l).max((h - pc).abs()).max((l - pc).abs()));
        }
    }
    tr
}

/// Calculate ADX
fn calculate_adx(high: &[f64], low: &[f64], close: &[f64], period: usize) -> f64 {
    if high.len() < period * 2 {
        return 25.0;
    }

    let tr = calculate_tr(high, low, close);
    let mut plus_dm = Vec::new();
    let mut minus_dm = Vec::new();

    for i in 0..high.len() {
        if i == 0 {
            plus_dm.push(0.0);
            minus_dm.push(0.0);
            continue;
        }
        let up = high[i] - high[i - 1];
        let down = low[i - 1] - low[i];

        plus_dm.push(if up > down && up > 0.0 { up } else { 0.0 });
        minus_dm.push(if down > up && down > 0.0 { down } else { 0.0 });
    }

    let tr_smooth = rma(&tr, period);
    let plus_dm_smooth = rma(&plus_dm, period);
    let minus_dm_smooth = rma(&minus_dm, period);

    let mut dx_list = Vec::new();
    for i in 0..tr_smooth.len() {
        if tr_smooth[i] == 0.0 {
            continue;
        }
        let p_di = 100.0 * (plus_dm_smooth[i] / tr_smooth[i]);
        let m_di = 100.0 * (minus_dm_smooth[i] / tr_smooth[i]);
        let sum = p_di + m_di;
        dx_list.push(if sum == 0.0 {
            0.0
        } else {
            100.0 * (p_di - m_di).abs() / sum
        });
    }

    let adx = rma(&dx_list, period);
    *adx.last().unwrap_or(&25.0)
}

/// RMA (Wilder's Smoothing)
fn rma(data: &[f64], period: usize) -> Vec<f64> {
    let mut results = Vec::new();
    let mut prev_rma = 0.0;
    let mut init = false;

    for i in 0..data.len() {
        if !init {
            if i < period - 1 {
                results.push(0.0);
            } else {
                let sum: f64 = data[..=i].iter().rev().take(period).sum();
                prev_rma = sum / period as f64;
                results.push(prev_rma);
                init = true;
            }
        } else {
            prev_rma = (prev_rma * (period as f64 - 1.0) + data[i]) / period as f64;
            results.push(prev_rma);
        }
    }
    results
}
