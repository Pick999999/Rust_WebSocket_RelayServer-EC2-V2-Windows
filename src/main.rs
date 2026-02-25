use axum::{
    extract::Request,
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    http::StatusCode,
    response::Response,
    response::{Html, IntoResponse, Redirect},
    routing::{get, post},
    Router,
};
use chrono::prelude::*;
use chrono::Local;
use futures_util::{SinkExt, StreamExt};
use indicator_math::{
    ehma, ema, generate_analysis_data, get_action_by_cut_type, get_action_by_simple, hma, sma, wma,
    Candle as IndicatorCandle, CutStrategy, MaType,
};
// New parallel analysis lib (RustLib/indicator_math)
use indicator_math_v2::{
    AnalysisGenerator as V2AnalysisGenerator, AnalysisOptions as V2AnalysisOptions,
    Candle as V2Candle, CandleMasterCode,
};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use sysinfo::{Pid, System}; // NEW
use time::Duration;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tokio_tungstenite::{connect_async, tungstenite::Message as TungsteniteMessage};
use tower::ServiceExt;
use tower_http::services::ServeDir;
use tower_sessions::{Expiry, MemoryStore, Session, SessionManagerLayer};

// Firestore Module
mod firestore_manager;
use firestore_manager::{GlobalFirestore, ScanRecord, TradeRecord};

// Market Scanner Module
mod market_scanner;
use market_scanner::{AssetConfig, MarketScanner, ScanConfig};

// Version tracking
const VERSION: &str = "1.2.0";

#[derive(Debug, Deserialize)]
pub struct LoginPayload {
    username: String,
    password: String,
}

// NEW: System Status Struct
#[derive(Debug, Serialize)]
pub struct SystemResources {
    pub memory_used_mb: u64,
    pub total_memory_mb: u64,
    pub cpu_usage: f32,
    pub uptime_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candle {
    pub symbol: String,
    pub time: u64,
    pub open_time: u64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerTime {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub server_time: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeOpened {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub contract_id: String,
    pub asset: String,
    pub trade_type: String,
    pub stake: f64,
    pub time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeResult {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub status: String,
    pub balance: f64,
    pub stake: f64,
    pub profit: f64,
    pub contract_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeUpdate {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub contract_id: String,
    pub asset: String,
    pub trade_type: String,
    pub current_spot: f64,
    pub entry_spot: f64,
    pub profit: f64,
    pub profit_percentage: f64,
    pub is_sold: bool,
    pub is_expired: bool,
    pub payout: f64,
    pub buy_price: f64,
    pub date_expiry: u64,
    pub date_start: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmaData {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub short_ema: Vec<EmaPoint>,
    pub medium_ema: Vec<EmaPoint>,
    pub long_ema: Vec<EmaPoint>,
    pub short_period: usize,
    pub medium_period: usize,
    pub long_period: usize,
    pub short_type: String,
    pub medium_type: String,
    pub long_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceMessage {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub balance: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmaPoint {
    pub time: u64,
    pub value: f64,
}

// Enhanced Analysis data with ALL fields from generate_analysis_data (v0.7.1)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisData {
    #[serde(rename = "type")]
    pub msg_type: String,

    // Current candle info
    pub time: u64,
    pub index: usize,
    pub color_candle: String,
    pub next_color_candle: String,

    // Short EMA Analysis
    pub ema_short_value: f64,
    pub ema_short_slope_value: f64,
    pub ema_short_slope_direction: String,
    pub is_ema_short_turn_type: String,
    pub ema_short_cut_position: String,

    // Medium EMA Analysis
    pub ema_medium_value: f64,
    pub ema_medium_slope_direction: String,

    // Long EMA Analysis
    pub ema_long_value: f64,
    pub ema_long_slope_direction: String,

    // Relationships
    pub ema_above: String,
    pub ema_long_above: String,

    // MACD Values
    pub macd_12: f64,
    pub macd_23: f64,

    // Previous Values
    pub previous_ema_short_value: f64,
    pub previous_ema_medium_value: f64,
    pub previous_ema_long_value: f64,
    pub previous_macd_12: f64,
    pub previous_macd_23: f64,

    // Convergence Types
    pub ema_convergence_type: String,
    pub ema_long_convergence_type: String,

    // Short/Medium Crossover
    pub ema_cut_short_type: String,
    pub candles_since_short_cut: usize,

    // Medium/Long Crossover
    pub ema_cut_long_type: String,
    pub candles_since_ema_cut: usize,

    // Historical
    pub previous_color_back1: String,
    pub previous_color_back3: String,

    // Action
    pub action: String,
    pub action_source: String, // "simple", "cut_type_short", "cut_type_long", "slope_fallback"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct IndicatorConfig {
    indicators: IndicatorsSection,
    #[serde(default)]
    chart: ChartSection,
    #[serde(default)]
    trading: TradingSection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct IndicatorsSection {
    short_ema_type: String,
    short_ema_period: usize,
    #[serde(default = "default_medium_ema_type")]
    medium_ema_type: String,
    #[serde(default = "default_medium_ema_period")]
    medium_ema_period: usize,
    long_ema_type: String,
    long_ema_period: usize,
    #[serde(default = "default_action_mode")]
    action_mode: String, // "simple", "cut_type_short", "cut_type_long"
}

fn default_medium_ema_type() -> String {
    "ema".to_string()
}
fn default_medium_ema_period() -> usize {
    8
}
fn default_action_mode() -> String {
    "cut_type_long".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ChartSection {
    #[serde(default = "default_short_color")]
    short_ema_color: String,
    #[serde(default = "default_long_color")]
    long_ema_color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct TradingSection {
    #[serde(default = "default_target_profit")]
    target_grand_profit: f64,
    #[serde(default = "default_target_win")]
    target_win_count: u32,
}

fn default_short_color() -> String {
    "#00BFFF".to_string()
}
fn default_long_color() -> String {
    "#FF6347".to_string()
}
fn default_target_profit() -> f64 {
    10.0
}
fn default_target_win() -> u32 {
    5
}

fn load_indicator_config() -> IndicatorConfig {
    match fs::read_to_string("config.toml") {
        Ok(content) => match toml::from_str::<IndicatorConfig>(&content) {
            Ok(config) => {
                println!("📊 Loaded config: short {} period {}, medium {} period {}, long {} period {}, action_mode: {}",
                        config.indicators.short_ema_type, config.indicators.short_ema_period,
                        config.indicators.medium_ema_type, config.indicators.medium_ema_period,
                        config.indicators.long_ema_type, config.indicators.long_ema_period,
                        config.indicators.action_mode);
                config
            }
            Err(e) => {
                println!("⚠️ Config parse error, using defaults: {}", e);
                default_indicator_config()
            }
        },
        Err(_) => {
            println!("⚠️ config.toml not found, using defaults");
            default_indicator_config()
        }
    }
}

fn save_indicator_config(config: &IndicatorConfig) {
    match toml::to_string_pretty(config) {
        Ok(toml_str) => {
            if let Err(e) = fs::write("config.toml", toml_str) {
                println!("❌ Failed to save config: {}", e);
            } else {
                println!("💾 Config saved successfully.");
            }
        }
        Err(e) => println!("❌ Failed to serialize config: {}", e),
    }
}

fn default_indicator_config() -> IndicatorConfig {
    IndicatorConfig {
        indicators: IndicatorsSection {
            short_ema_type: "ema".to_string(),
            short_ema_period: 3,
            medium_ema_type: "ema".to_string(),
            medium_ema_period: 8,
            long_ema_type: "ema".to_string(),
            long_ema_period: 21,
            action_mode: "cut_type_long".to_string(),
        },
        chart: ChartSection {
            short_ema_color: "#00BFFF".to_string(),
            long_ema_color: "#FF6347".to_string(),
        },
        trading: TradingSection {
            target_grand_profit: 10.0,
            target_win_count: 5,
        },
    }
}

fn calculate_indicator(
    candles: &[IndicatorCandle],
    indicator_type: &str,
    period: usize,
) -> Vec<EmaPoint> {
    let result = match indicator_type.to_lowercase().as_str() {
        "sma" => sma(candles, period),
        "wma" => wma(candles, period),
        "hma" => hma(candles, period),
        "ehma" => ehma(candles, period),
        _ => ema(candles, period), // default to EMA
    };

    result
        .into_iter()
        .map(|v| EmaPoint {
            time: v.time,
            value: v.value,
        })
        .collect()
}

// Parse string to MaType enum
fn parse_ma_type(type_str: &str) -> MaType {
    match type_str.to_lowercase().as_str() {
        "sma" => MaType::SMA,
        "wma" => MaType::WMA,
        "hma" => MaType::HMA,
        "ehma" => MaType::EHMA,
        _ => MaType::EMA,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeObject {
    #[serde(rename = "LotNo")]
    pub lot_no: u32,
    #[serde(rename = "TradeNoOnThisLot")]
    pub trade_no_on_this_lot: u32,
    #[serde(rename = "TradeTime")]
    pub trade_time: String,
    pub asset: String,
    pub action: String,
    #[serde(rename = "moneyTrade")]
    pub money_trade: f64,
    #[serde(rename = "MoneyTradeType")]
    pub money_trade_type: String,
    #[serde(rename = "WinStatus")]
    pub win_status: String,
    #[serde(rename = "Profit")]
    pub profit: f64,
    #[serde(rename = "BalanceOnLot")]
    pub balance_on_lot: f64,
    #[serde(rename = "winCon")]
    pub win_con: String,
    #[serde(rename = "lossCon")]
    pub loss_con: String,
    #[serde(rename = "isstopTrade")]
    pub is_stop_trade: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LotLog {
    #[serde(rename = "LotNo")]
    pub lot_no: u32,
    #[serde(rename = "TradeObjectList")]
    pub trade_object_list: Vec<TradeObject>,
}

// ==================== NEW DAY TRADE LOGGING ====================
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DayTradeEntry {
    #[serde(rename = "No")]
    pub no: u32,
    #[serde(rename = "ContractID")]
    pub contract_id: String,
    #[serde(rename = "Symbol")]
    pub symbol: String,
    #[serde(rename = "StatusCode")]
    pub status_code: String,
    #[serde(rename = "Type")]
    pub trade_type: String,
    #[serde(rename = "BuyPrice")]
    pub buy_price: f64,
    #[serde(rename = "Payout")]
    pub payout: f64,
    #[serde(rename = "BuyTime")]
    pub buy_time: String,
    #[serde(rename = "Expiry")]
    pub expiry: String,
    #[serde(rename = "Remaining")]
    pub remaining: String,
    #[serde(rename = "MinProfit")]
    pub min_profit: f64,
    #[serde(rename = "MaxProfit")]
    pub max_profit: f64,
    #[serde(rename = "Profit")]
    pub profit: f64,
    #[serde(rename = "Action")]
    pub action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DayTradeData {
    #[serde(rename = "LotNoCurrent")]
    pub lot_no_current: u32,
    #[serde(rename = "DayTrade")]
    pub day_trade: String,
    #[serde(rename = "StartTradeOfDay")]
    pub start_trade_of_day: String,
    #[serde(rename = "LastTradeOfDay")]
    pub last_trade_of_day: String,
    #[serde(rename = "TotalTradeOnThisDay")]
    pub total_trade_on_this_day: u32,
    #[serde(rename = "TotalProfit")]
    pub total_profit: f64,
    #[serde(rename = "StatusofTrade")]
    pub status_of_trade: String,
    #[serde(rename = "CurrentProfit")]
    pub current_profit: f64,
    #[serde(rename = "DayTradeList")]
    pub day_trade_list: Vec<DayTradeEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DayTradeWrapper {
    #[serde(rename = "DayTrade")]
    pub day_trade: DayTradeData,
}

pub fn ensure_trade_history_folder(folder_date: &str) -> String {
    let path = format!("tradeHistory/{}", folder_date);
    if !Path::new(&path).exists() {
        let _ = fs::create_dir_all(&path);
    }
    path
}

pub fn save_day_trade_log(wrapper: &DayTradeWrapper) {
    let folder_date = &wrapper.day_trade.day_trade;
    let folder_path = ensure_trade_history_folder(folder_date);
    let file_path = format!("{}/trade.json", folder_path);

    if let Ok(json) = serde_json::to_string_pretty(wrapper) {
        let _ = fs::write(file_path, json);
    }
}
// ===============================================================

pub fn get_daily_folder_name() -> String {
    let local: DateTime<Local> = Local::now();
    local.format("%Y-%m-%d").to_string()
}

pub fn ensure_daily_folder(folder_name: &str) -> String {
    let path = format!("logs/{}", folder_name);
    if !Path::new(&path).exists() {
        let _ = fs::create_dir_all(&path);
    }
    path
}

pub fn get_next_lot_no(folder_path: &str) -> u32 {
    let mut max_lot = 0;
    if let Ok(entries) = fs::read_dir(folder_path) {
        for entry in entries {
            if let Ok(entry) = entry {
                let path = entry.path();
                if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                    if file_name.starts_with("lot_") && file_name.ends_with(".json") {
                        if let Some(num_part) = file_name
                            .trim_start_matches("lot_")
                            .trim_end_matches(".json")
                            .parse::<u32>()
                            .ok()
                        {
                            if num_part > max_lot {
                                max_lot = num_part;
                            }
                        }
                    }
                }
            }
        }
    }
    max_lot + 1
}

pub fn save_lot_log(folder_path: &str, lot_log: &LotLog) {
    let file_path = format!("{}/lot_{}.json", folder_path, lot_log.lot_no);
    if let Ok(json) = serde_json::to_string_pretty(lot_log) {
        let _ = fs::write(file_path, json);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LotStatus {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub grand_profit: f64,
    pub win_count: u32,
    pub target_profit: f64,
    pub target_win: u32,
    pub lot_active: bool,
    #[serde(default)]
    pub balance: f64,
}

// ============ Multi-Asset Structs ============
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeSignalEntry {
    pub id: String,
    #[serde(rename = "assetCode")]
    pub asset_code: String,
    #[serde(rename = "PUTSignal")]
    pub put_signal: String,
    #[serde(rename = "CallSigNal")]
    pub call_signal: String,
    #[serde(rename = "isActive")]
    pub is_active: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetSignalResult {
    pub asset: String,
    pub status_code: String,
    pub status_desc: String,
    pub decision: String, // "call", "put", "idle"
    pub reason: String,
    pub close_price: f64,
    pub ema_short_dir: String,
    pub ema_medium_dir: String,
    pub ema_long_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiAnalysisMessage {
    #[serde(rename = "type")]
    pub msg_type: String, // "multi_analysis"
    pub timestamp: u64,
    pub assets: Vec<AssetSignalResult>,
}

#[derive(Serialize, Clone, Debug, Deserialize)]
pub struct CompactAnalysis {
    pub time: u64,
    pub action: String,
    pub status_code: String,
}

#[derive(Serialize, Clone, Debug, Deserialize)]
pub struct HistoricalAnalysis {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub symbol: String,
    pub results: Vec<CompactAnalysis>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BroadcastMessage {
    Candle(Candle),
    ServerTime(ServerTime),
    TradeOpened(TradeOpened),
    TradeResult(TradeResult),
    TradeUpdate(TradeUpdate),
    EmaData(EmaData),
    Balance(BalanceMessage),
    Analysis(AnalysisData),
    LotStatus(LotStatus),
    MultiAnalysis(MultiAnalysisMessage),
    AutoTradeStatus(AutoTradeStatusMessage),
    HistoricalAnalysis(HistoricalAnalysis),
}

#[derive(Deserialize, Debug, Clone)]
struct ClientCommand {
    command: String,
    #[serde(default)]
    asset: String,
    #[serde(default)]
    assets: Vec<String>, // Multi-asset list from checkboxes
    #[serde(default)]
    trade_mode: String,
    #[serde(default)]
    money_mode: String,
    #[serde(default)]
    initial_stake: f64,
    #[serde(default)]
    app_id: String,
    #[serde(default)]
    api_token: String,
    #[serde(default)]
    duration: u64,
    #[serde(default)]
    duration_unit: String,
    #[serde(default)]
    contract_id: String,
    #[serde(default)]
    target_profit: f64,
    #[serde(default)]
    target_win: u32,
}

// Auto-trade status message (sent to browser if connected)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoTradeEntry {
    pub asset: String,
    pub direction: String, // "CALL" or "PUT"
    pub status_code: String,
    pub stake: f64,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoTradeStatusMessage {
    #[serde(rename = "type")]
    pub msg_type: String, // "auto_trade_status"
    pub active: bool,
    pub entries: Vec<AutoTradeEntry>,
    pub grand_profit: f64,
    pub trade_count: u32,
    pub message: String,
}

struct AppState {
    tx: broadcast::Sender<BroadcastMessage>,
    current_conn: Arc<Mutex<Option<(JoinHandle<()>, tokio::sync::mpsc::Sender<String>)>>>,
    firestore: Arc<tokio::sync::Mutex<GlobalFirestore>>,
    scanner: Arc<tokio::sync::RwLock<Option<MarketScanner>>>,
    // Auto-trade handle — persists beyond browser disconnect
    auto_trade: Arc<Mutex<Option<(JoinHandle<()>, tokio::sync::mpsc::Sender<String>)>>>,
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();

    // Session setup
    let session_store = MemoryStore::default();
    let session_layer = SessionManagerLayer::new(session_store)
        .with_secure(false)
        .with_expiry(Expiry::OnInactivity(Duration::minutes(60)));

    let (tx, _) = broadcast::channel::<BroadcastMessage>(1024);

    // Initialize Firestore
    let mut firestore = GlobalFirestore::new();
    let project_id =
        env::var("FIRESTORE_PROJECT_ID").unwrap_or_else(|_| "your-project-id".to_string());
    if project_id != "your-project-id" {
        if let Err(e) = firestore.initialize(&project_id).await {
            println!("⚠️ Firestore initialization warning: {}", e);
        }
    } else {
        println!("⚠️ FIRESTORE_PROJECT_ID not set in .env - Firestore disabled");
    }

    let firestore_arc = Arc::new(tokio::sync::Mutex::new(firestore));

    // Initialize Market Scanner
    let scanner = MarketScanner::new(firestore_arc.clone());
    println!("📊 Market Scanner initialized");

    let state = Arc::new(AppState {
        tx,
        current_conn: Arc::new(Mutex::new(None)),
        firestore: firestore_arc,
        scanner: Arc::new(tokio::sync::RwLock::new(Some(scanner))),
        auto_trade: Arc::new(Mutex::new(None)),
    });

    let app = Router::new()
        .route("/login", get(serve_login_html).post(login_handler))
        .route("/logout", get(logout_handler))
        .route("/ws", get(websocket_handler))
        // Protected fallback using manual handler
        .route("/save_tick_history", post(save_tick_history_handler))
        .route("/api/save_scan", post(save_scan_handler))
        // Scanner API endpoints
        .route("/api/scanner/start", post(scanner_start_handler))
        .route("/api/scanner/stop", post(scanner_stop_handler))
        .route("/api/scanner/status", get(scanner_status_handler))
        .route("/api/system/resources", get(system_resources_handler)) // NEW
        // Trade Logging API endpoint
        .route("/api/save-trade", post(save_trade_handler))
        .route(
            "/api/trade_history/today",
            get(get_today_trade_history_handler),
        )
        // Trading Config API endpoints
        .route(
            "/api/trading-config",
            get(get_trading_config_handler).post(save_trading_config_handler),
        )
        .fallback(get(protected_file_handler))
        .layer(session_layer)
        .with_state(state);

    println!("-----------------------------------------");
    println!(
        "🚀 RELAY SERVER v{} STARTING AT http://localhost:8080",
        VERSION
    );
    println!("🔐 Authentication Enabled (User from .env)");
    println!("📂 Make sure 'public/index.html' exists!");
    println!("-----------------------------------------");

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

// Auth Handlers
async fn serve_login_html() -> Html<String> {
    match fs::read_to_string("public/login.html") {
        Ok(content) => Html(content),
        Err(_) => Html("<h1>Login page not found</h1>".to_string()),
    }
}

async fn login_handler(
    session: Session,
    axum::Json(payload): axum::Json<LoginPayload>,
) -> Response {
    let env_user = env::var("APP_USER").unwrap_or_else(|_| "admin".to_string());
    let env_pass = env::var("APP_PASSWORD").unwrap_or_else(|_| "password".to_string());

    if payload.username == env_user && payload.password == env_pass {
        let _ = session.insert("user", "authenticated").await;
        // Return 200 OK
        return Response::builder().status(200).body("OK".into()).unwrap();
    }

    Response::builder()
        .status(401)
        .body("Invalid credentials".into())
        .unwrap()
}

async fn logout_handler(session: Session) -> Redirect {
    let _ = session.delete().await;
    Redirect::to("/login")
}

async fn protected_file_handler(session: Session, req: Request) -> Response {
    if session
        .get::<String>("user")
        .await
        .unwrap_or(None)
        .is_some()
    {
        let service = ServeDir::new("public");
        match service.oneshot(req).await {
            Ok(res) => res.into_response(),
            Err(err) => {
                (StatusCode::INTERNAL_SERVER_ERROR, format!("Error: {}", err)).into_response()
            }
        }
    } else {
        Redirect::to("/login").into_response()
    }
}

async fn websocket_handler(
    ws: WebSocketUpgrade,
    session: Session,
    State(state): State<Arc<AppState>>,
) -> Response {
    if session
        .get::<String>("user")
        .await
        .unwrap_or(None)
        .is_none()
    {
        return Redirect::to("/login").into_response();
    }
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    println!("🔌 New Browser connected");
    let (mut sender, mut receiver) = socket.split();
    let mut rx = state.tx.subscribe();

    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if let Ok(json) = serde_json::to_string(&msg) {
                if sender.send(Message::Text(json)).await.is_err() {
                    break;
                }
            }
        }
    });

    let state_clone = state.clone();
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if let Message::Text(text) = msg {
                println!("📥 Browser sent: {}", text);

                match serde_json::from_str::<ClientCommand>(&text) {
                    Ok(req) => {
                        if req.command == "START_DERIV" {
                            println!("🎯 Valid Command! Requesting Asset: {}", req.asset);

                            let old_conn = {
                                let mut conn_guard = state_clone.current_conn.lock().unwrap();
                                conn_guard.take()
                            };

                            if let Some((old_handle, old_tx)) = old_conn {
                                println!("🛑 Stopping old connection...");
                                let _ = old_tx.send("FORGET".to_string()).await;
                                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                                old_handle.abort();
                            }

                            let tx = state_clone.tx.clone();
                            let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel::<String>(10);
                            let firestore = state_clone.firestore.clone();

                            let handle = tokio::spawn(async move {
                                connect_to_deriv(tx, req, cmd_rx, firestore).await;
                            });

                            {
                                let mut conn_guard = state_clone.current_conn.lock().unwrap();
                                *conn_guard = Some((handle, cmd_tx));
                            }
                        } else if req.command == "UPDATE_MODE" {
                            println!("🔄 Request to Update Trade Mode: {}", req.trade_mode);
                            // Send to current_conn (single asset viewer)
                            let cmd_tx = {
                                state_clone
                                    .current_conn
                                    .lock()
                                    .unwrap()
                                    .as_ref()
                                    .map(|(_, tx)| tx.clone())
                            };
                            if let Some(tx) = cmd_tx {
                                let _ = tx.send(format!("MODE:{}", req.trade_mode)).await;
                            }

                            // ALSO send to auto_trade task as JSON
                            let auto_tx = {
                                state_clone
                                    .auto_trade
                                    .lock()
                                    .unwrap()
                                    .as_ref()
                                    .map(|(_, tx)| tx.clone())
                            };
                            if let Some(tx) = auto_tx {
                                let _ = tx.send(text.clone()).await;
                            }
                        } else if req.command == "UPDATE_PARAMS" {
                            println!("🔄 Request to Update Params: Money={}, Stake={}, Duration={} {}, Targets=P:{}/W:{}",
                                req.money_mode, req.initial_stake, req.duration, req.duration_unit, req.target_profit, req.target_win);

                            // Load config, update, and save
                            let mut config = load_indicator_config();
                            config.trading.target_grand_profit = req.target_profit;
                            config.trading.target_win_count = req.target_win;
                            save_indicator_config(&config);

                            // Send to current_conn (single asset)
                            let cmd_tx = {
                                state_clone
                                    .current_conn
                                    .lock()
                                    .unwrap()
                                    .as_ref()
                                    .map(|(_, tx)| tx.clone())
                            };
                            if let Some(tx) = cmd_tx {
                                let _ = tx
                                    .send(format!(
                                        "PARAMS:{}:{}:{}:{}:{}:{}",
                                        req.money_mode,
                                        req.initial_stake,
                                        req.duration,
                                        req.duration_unit,
                                        req.target_profit,
                                        req.target_win
                                    ))
                                    .await;
                            }

                            // ALSO send to auto_trade task as JSON
                            let auto_tx = {
                                state_clone
                                    .auto_trade
                                    .lock()
                                    .unwrap()
                                    .as_ref()
                                    .map(|(_, tx)| tx.clone())
                            };
                            if let Some(tx) = auto_tx {
                                let _ = tx.send(text.clone()).await;
                            }
                        } else if req.command == "SELL" {
                            println!("🔻 Request to Sell Contract: {}", req.contract_id);
                            // Send to current_conn
                            let cmd_tx = {
                                state_clone
                                    .current_conn
                                    .lock()
                                    .unwrap()
                                    .as_ref()
                                    .map(|(_, tx)| tx.clone())
                            };
                            if let Some(tx) = cmd_tx {
                                let _ = tx.send(format!("SELL:{}", req.contract_id)).await;
                            }

                            // ALSO send to auto_trade task
                            let auto_tx = {
                                state_clone
                                    .auto_trade
                                    .lock()
                                    .unwrap()
                                    .as_ref()
                                    .map(|(_, tx)| tx.clone())
                            };
                            if let Some(tx) = auto_tx {
                                let _ = tx.send(format!("SELL:{}", req.contract_id)).await;
                            }
                        } else if req.command == "STOP_STREAMS" {
                            println!("🛑 Request to Stop Streams (Keep Alive)");
                            let cmd_tx = {
                                state_clone
                                    .current_conn
                                    .lock()
                                    .unwrap()
                                    .as_ref()
                                    .map(|(_, tx)| tx.clone())
                            };
                            if let Some(tx) = cmd_tx {
                                let _ = tx.send("STOP_STREAMS".to_string()).await;
                            } else {
                                println!("⚠️ No active connection to update.");
                            }
                        } else if req.command == "START_MULTI_TRADE" {
                            println!(
                                "🎯 START_MULTI_TRADE: Starting multi-asset parallel analysis"
                            );

                            // Stop existing connection if any
                            let old_conn = {
                                let mut conn_guard = state_clone.current_conn.lock().unwrap();
                                conn_guard.take()
                            };
                            if let Some((old_handle, old_tx)) = old_conn {
                                println!("🛑 Stopping old connection...");
                                let _ = old_tx.send("FORGET".to_string()).await;
                                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                                old_handle.abort();
                            }

                            let tx = state_clone.tx.clone();
                            let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel::<String>(10);
                            let firestore = state_clone.firestore.clone();

                            let handle = tokio::spawn(async move {
                                connect_multi_asset(tx, req, cmd_rx, firestore).await;
                            });

                            {
                                let mut conn_guard = state_clone.current_conn.lock().unwrap();
                                *conn_guard = Some((handle, cmd_tx));
                            }
                        } else if req.command == "START_AUTO_MULTI" {
                            println!(
                                "🎯 START_AUTO_MULTI: Starting browser-independent multi-asset auto-trade"
                            );
                            println!("   Assets: {:?}", req.assets);

                            // Stop existing auto-trade if any
                            let old_auto = {
                                let mut auto_guard = state_clone.auto_trade.lock().unwrap();
                                auto_guard.take()
                            };
                            if let Some((old_handle, old_tx)) = old_auto {
                                println!("🛑 Stopping previous auto-trade...");
                                let _ = old_tx.send("STOP".to_string()).await;
                                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                                old_handle.abort();
                            }

                            let tx = state_clone.tx.clone();
                            let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel::<String>(10);
                            let firestore = state_clone.firestore.clone();

                            let handle = tokio::spawn(async move {
                                auto_multi_trade(tx, req, cmd_rx, firestore).await;
                            });

                            {
                                let mut auto_guard = state_clone.auto_trade.lock().unwrap();
                                *auto_guard = Some((handle, cmd_tx));
                            }
                        } else if req.command == "SYNC_STATUS" {
                            println!("🔄 Request to Sync Status from Browser");
                            let single_tx = {
                                state_clone
                                    .current_conn
                                    .lock()
                                    .unwrap()
                                    .as_ref()
                                    .map(|(_, tx)| tx.clone())
                            };
                            if let Some(tx) = single_tx {
                                let _ = tx.send("SYNC".to_string()).await;
                            }

                            let auto_tx = {
                                state_clone
                                    .auto_trade
                                    .lock()
                                    .unwrap()
                                    .as_ref()
                                    .map(|(_, tx)| tx.clone())
                            };
                            if let Some(tx) = auto_tx {
                                let _ = tx.send("SYNC".to_string()).await;
                            }
                        } else if req.command == "STOP_AUTO_TRADE" {
                            println!("🛑 STOP_AUTO_TRADE: Stopping auto-trade task");
                            let old_auto = {
                                let mut auto_guard = state_clone.auto_trade.lock().unwrap();
                                auto_guard.take()
                            };
                            if let Some((old_handle, old_tx)) = old_auto {
                                let _ = old_tx.send("STOP".to_string()).await;
                                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                                old_handle.abort();
                                println!("✅ Auto-trade stopped.");
                            } else {
                                println!("⚠️ No auto-trade running.");
                            }
                        }
                    }
                    Err(e) => println!("⚠️ JSON Parse Error: {}", e),
                }
            }
        }
    });

    tokio::select! {
        _ = (&mut send_task) => println!("📤 Send task ended"),
        _ = (&mut recv_task) => println!("📥 Receive task ended"),
    };
}

async fn connect_to_deriv(
    tx: broadcast::Sender<BroadcastMessage>,
    config: ClientCommand,
    mut cmd_rx: tokio::sync::mpsc::Receiver<String>,
    firestore: Arc<tokio::sync::Mutex<GlobalFirestore>>,
) {
    let app_id = if config.app_id.is_empty() {
        "66726".to_string()
    } else {
        config.app_id
    };
    let url = format!("wss://ws.derivws.com/websockets/v3?app_id={}", app_id);
    println!("🌐 Connecting to Deriv API for asset: {}...", config.asset);

    match connect_async(&url).await {
        Ok((ws_stream, _)) => {
            println!("✅ Connected to Deriv: {}", config.asset);
            let (mut write, mut read) = ws_stream.split();

            let mut tick_sub_id: Option<String> = None;
            let mut candle_sub_id: Option<String> = None;

            // Trading state
            let mut balance = 1000.0;
            let martingale_stakes = vec![1.0, 2.0, 6.0, 18.0, 54.0, 162.0, 384.0, 800.0, 1600.0];
            let mut current_stake_index = 0;
            let mut last_trade_minute: Option<u64> = None;
            let mut _pending_contract_id: Option<String> = None;
            let mut pending_contract_type: Option<String> = None;
            let mut current_trade_mode = config.trade_mode.clone();
            let mut current_money_mode = config.money_mode.clone();

            // Set defaults if missing (though they have defaults in struct) or 0
            let mut current_duration = if config.duration == 0 {
                55
            } else {
                config.duration
            };
            let mut current_duration_unit = if config.duration_unit.is_empty() {
                "s".to_string()
            } else {
                config.duration_unit.clone()
            };
            let mut current_initial_stake = if config.initial_stake > 0.0 {
                config.initial_stake
            } else {
                1.0
            };

            // Load indicator config
            let mut indicator_config = load_indicator_config();
            let mut candles_for_ema: Vec<IndicatorCandle> = Vec::new();
            let mut last_ema_minute: Option<u64> = None;
            let mut last_analysis_time: Option<u64> = None; // For 2-second analysis interval
            let mut last_action: Option<String> = None; // Store last action from analysis ("call", "put", "hold")

            // LotNo State
            let mut lot_grand_profit = 0.0;
            let mut lot_win_count = 0;
            let mut lot_active = true;

            // Daily Lot Logging
            let mut daily_folder = ensure_daily_folder(&get_daily_folder_name());
            let mut current_lot_no = get_next_lot_no(&daily_folder);
            let mut trades_for_lot: Vec<TradeObject> = Vec::new();
            let mut trade_count_in_lot = 0;

            println!(
                "📂 Current Daily Folder: {}, Starting Lot No: {}",
                daily_folder, current_lot_no
            );

            // Broadcast initial Lot State
            let _ = tx.send(BroadcastMessage::LotStatus(LotStatus {
                msg_type: "lot_status".to_string(),
                grand_profit: lot_grand_profit,
                win_count: lot_win_count,
                target_profit: indicator_config.trading.target_grand_profit,
                target_win: indicator_config.trading.target_win_count,
                lot_active,
                balance: 0.0,
            }));

            // Authorize if token provided
            if !config.api_token.is_empty() {
                let auth_msg = serde_json::json!({
                    "authorize": config.api_token
                });
                let _ = write
                    .send(TungsteniteMessage::Text(auth_msg.to_string()))
                    .await;
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            }

            // Subscribe tick
            let tick_msg = serde_json::json!({
                "ticks": "R_100",
                "subscribe": 1
            });
            // println!("📊 Subscribe  Tick candles");
            println!("🔥🔥🔥 SUBSCRIBE TICK EXECUTED 🔥🔥🔥");

            let _ = write
                .send(TungsteniteMessage::Text(tick_msg.to_string()))
                .await;
            tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

            // Subscribe candles
            let sub_msg = serde_json::json!({
                "ticks_history": config.asset,
                "subscribe": 1,
                "style": "candles",
                "granularity": 60,
                "count": 50,
                "end": "latest",
                "adjust_start_time": 1
            });
            let _ = write
                .send(TungsteniteMessage::Text(sub_msg.to_string()))
                .await;

            loop {
                tokio::select! {
                    cmd = cmd_rx.recv() => {
                        if let Some(cmd) = cmd {
                            if cmd == "FORGET" {
                                println!("📤 Sending forget for all subscriptions...");
                                if let Some(id) = tick_sub_id.take() {
                                    let forget_msg = serde_json::json!({"forget": id});
                                    let _ = write.send(TungsteniteMessage::Text(forget_msg.to_string())).await;
                                }
                                if let Some(id) = candle_sub_id.take() {
                                    let forget_msg = serde_json::json!({"forget": id});
                                    let _ = write.send(TungsteniteMessage::Text(forget_msg.to_string())).await;
                                }
                                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                                break;
                            } else if cmd == "STOP_STREAMS" {
                                println!("📤 Sending forget for all subscriptions (Connection Kept Alive)...");
                                if let Some(id) = tick_sub_id.take() {
                                    let forget_msg = serde_json::json!({"forget": id});
                                    let _ = write.send(TungsteniteMessage::Text(forget_msg.to_string())).await;
                                }
                                if let Some(id) = candle_sub_id.take() {
                                    let forget_msg = serde_json::json!({"forget": id});
                                    let _ = write.send(TungsteniteMessage::Text(forget_msg.to_string())).await;
                                }
                                // Reset IDs
                                tick_sub_id = None;
                                candle_sub_id = None;

                                // Set mode to IDLE so we don't try to trade on old data or if streams somehow resume
                                current_trade_mode = "idle".to_string();
                                lot_active = false; // Stop lot logging

                            } else if cmd.starts_with("MODE:") {
                                let new_mode = cmd.replace("MODE:", "");

                                // Reset lot if starting fresh
                                if new_mode != "idle" && current_trade_mode == "idle" {
                                    println!("🆕 Starting new Lot session");
                                    // Update folder and Lot No
                                    daily_folder = ensure_daily_folder(&get_daily_folder_name());
                                    current_lot_no = get_next_lot_no(&daily_folder);
                                    trades_for_lot.clear();
                                    trade_count_in_lot = 0;

                                    lot_grand_profit = 0.0;
                                    lot_win_count = 0;
                                    lot_active = true;

                                    println!("🔢 New Lot No: {}", current_lot_no);

                                    let _ = tx.send(BroadcastMessage::LotStatus(LotStatus {
                                        msg_type: "lot_status".to_string(),
                                        grand_profit: lot_grand_profit,
                                        win_count: lot_win_count,
                                        target_profit: indicator_config.trading.target_grand_profit,
                                        target_win: indicator_config.trading.target_win_count,
                                        lot_active,
                                        balance: 0.0,
                                    }));
                                }

                                current_trade_mode = new_mode;
                                println!("🔄 Trade Mode Updated to: {}", current_trade_mode);

                            } else if cmd.starts_with("PARAMS:") {
                                let parts: Vec<&str> = cmd.split(':').collect();
                                if parts.len() >= 7 {
                                    // format: PARAMS:money_mode:initial_stake:duration:duration_unit:target_profit:target_win
                                    current_money_mode = parts[1].to_string();
                                    if let Ok(stake) = parts[2].parse::<f64>() {
                                        current_initial_stake = stake;
                                    }
                                    if let Ok(d) = parts[3].parse::<u64>() {
                                        current_duration = d;
                                    }
                                    current_duration_unit = parts[4].to_string();

                                    if let Ok(tp) = parts[5].parse::<f64>() {
                                        indicator_config.trading.target_grand_profit = tp;
                                    }
                                    if let Ok(tw) = parts[6].parse::<u32>() {
                                        indicator_config.trading.target_win_count = tw;
                                    }

                                    println!("✅ Params Updated: Money={}, Stake={}, Duration={} {}, T.Profit={}, T.Win={}",
                                        current_money_mode, current_initial_stake, current_duration, current_duration_unit,
                                        indicator_config.trading.target_grand_profit, indicator_config.trading.target_win_count);

                                    // Broadcast new targets
                                    let _ = tx.send(BroadcastMessage::LotStatus(LotStatus {
                                        msg_type: "lot_status".to_string(),
                                        grand_profit: lot_grand_profit,
                                        win_count: lot_win_count,
                                        target_profit: indicator_config.trading.target_grand_profit,
                                        target_win: indicator_config.trading.target_win_count,
                                        lot_active,
                                        balance: 0.0,
                                    }));

                                    // Reset martingale index if switching to fix
                                    if current_money_mode == "fix" {
                                        current_stake_index = 0;
                                    }
                                }
                            } else if cmd.starts_with("SELL:") {
                                let contract_id = cmd.replace("SELL:", "");
                                println!("🔻 Sending Sell Request for: {}", contract_id);
                                let sell_msg = serde_json::json!({
                                    "sell": contract_id,
                                    "price": 0
                                });
                                let _ = write.send(TungsteniteMessage::Text(sell_msg.to_string())).await;
                            } else if cmd == "SYNC" {
                                let _ = tx.send(BroadcastMessage::LotStatus(LotStatus {
                                    msg_type: "lot_status".to_string(),
                                    grand_profit: lot_grand_profit,
                                    win_count: lot_win_count,
                                    target_profit: indicator_config.trading.target_grand_profit,
                                    target_win: indicator_config.trading.target_win_count,
                                    lot_active,
                                    balance: 0.0,
                                }));
                                // Optional auto trade status message, even though this is single trade mode, but good for completeness
                            }
                        }
                    }

                    msg = read.next() => {
                        println!("📊 Received Msg");
                        if let Some(Ok(TungsteniteMessage::Text(raw_text))) = msg {
                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&raw_text) {

                                if let Some(error) = json.get("error") {
                                    println!("❌ API Error: {}", error.get("message").unwrap_or(&serde_json::json!("Unknown error")));
                                    break;
                                }

                                // Get balance from authorize response
                                if let Some(authorize) = json.get("authorize") {
                                    // let mut initial_analysis_sent = false;
                                    if let Some(bal) = authorize.get("balance").and_then(|b| b.as_f64()) {
                                        balance = bal;
                                        println!("💰 Current Balance: {}", balance);

                                        // Send balance to frontend
                                        let balance_msg = BalanceMessage {
                                            msg_type: "balance".to_string(),
                                            balance,
                                        };
                                        let _ = tx.send(BroadcastMessage::Balance(balance_msg));
                                    }
                                }

                                // Handle subscription IDs
                                if let Some(sub) = json.get("subscription") {
                                    if let Some(id) = sub.get("id").and_then(|i| i.as_str()) {
                                        if tick_sub_id.is_none() && json.get("tick").is_some() {
                                            tick_sub_id = Some(id.to_string());
                                        }
                                        if candle_sub_id.is_none() && (json.get("candles").is_some() || json.get("ohlc").is_some()) {
                                            candle_sub_id = Some(id.to_string());
                                        }
                                    }
                                }

                                // Server time
                                if let Some(tick) = json.get("tick") {
                                    if let Some(epoch) = tick.get("epoch").and_then(|e| e.as_u64()) {
                                        println!("📊 Received Tick candles");
                                        let time_msg = ServerTime {
                                            msg_type: "server_time".to_string(),
                                            server_time: epoch,
                                        };
                                        let _ = tx.send(BroadcastMessage::ServerTime(time_msg));
                                    }
                                }

                                // Historical candles
                                if let Some(candles) = json.get("candles").and_then(|c| c.as_array()) {
                                    println!("📊 Received {} historical candles", candles.len());

                                    // Clear and rebuild candles_for_ema with historical data
                                    //candles_for_ema.clear();

                                    for candle_data in candles {
                                        if let Ok(mut candle) = parse_flexible(candle_data) {
                                            candle.symbol = config.asset.clone();
                                            let _ = tx.send(BroadcastMessage::Candle(candle.clone()));

                                            // Store for EMA calculation
                                            candles_for_ema.push(IndicatorCandle {
                                                time: candle.time,
                                                open: candle.open,
                                                high: candle.high,
                                                low: candle.low,
                                                close: candle.close,
                                            });
                                        }
                                    }

                                    // Calculate and send initial EMA data
                                    if candles_for_ema.len() >= indicator_config.indicators.long_ema_period {
                                        let short_ema = calculate_indicator(
                                            &candles_for_ema,
                                            &indicator_config.indicators.short_ema_type,
                                            indicator_config.indicators.short_ema_period
                                        );
                                        let medium_ema = calculate_indicator(
                                            &candles_for_ema,
                                            &indicator_config.indicators.medium_ema_type,
                                            indicator_config.indicators.medium_ema_period
                                        );
                                        let long_ema = calculate_indicator(
                                            &candles_for_ema,
                                            &indicator_config.indicators.long_ema_type,
                                            indicator_config.indicators.long_ema_period
                                        );

                                        let ema_msg = EmaData {
                                            msg_type: "ema_data".to_string(),
                                            short_ema,
                                            medium_ema,
                                            long_ema,
                                            short_period: indicator_config.indicators.short_ema_period,
                                            medium_period: indicator_config.indicators.medium_ema_period,
                                            long_period: indicator_config.indicators.long_ema_period,
                                            short_type: indicator_config.indicators.short_ema_type.clone(),
                                            medium_type: indicator_config.indicators.medium_ema_type.clone(),
                                            long_type: indicator_config.indicators.long_ema_type.clone(),
                                        };

                                        println!("📈 Sending initial EMA data: short {} points, medium {} points, long {} points",
                                            ema_msg.short_ema.len(), ema_msg.medium_ema.len(), ema_msg.long_ema.len());
                                        let _ = tx.send(BroadcastMessage::EmaData(ema_msg));

                                        // === V2 Analysis: Use V2AnalysisGenerator for REAL status codes ===
                                        // This replaces the old generate_analysis_data + mock code approach
                                        let v2_master_codes = build_candle_master_codes();
                                        let v2_master_codes_arc = std::sync::Arc::new(v2_master_codes);
                                        let v2_opts = V2AnalysisOptions::default();
                                        let mut v2_gen = V2AnalysisGenerator::new(v2_opts, v2_master_codes_arc);

                                        // Load tradeSignal.json for signal matching
                                        let signal_entries_for_hist: Vec<TradeSignalEntry> = match std::fs::read_to_string("tradeSignal.json") {
                                            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
                                            Err(_) => Vec::new(),
                                        };

                                        let mut v2_history = Vec::new();
                                        for ic in &candles_for_ema {
                                            let v2_result = v2_gen.append_candle(V2Candle {
                                                time: ic.time,
                                                open: ic.open,
                                                high: ic.high,
                                                low: ic.low,
                                                close: ic.close,
                                            });

                                            // Determine call/put/idle from tradeSignal.json
                                            let mut decision = "idle".to_string();
                                            if let Some(entry) = signal_entries_for_hist.iter().find(|e| e.asset_code == config.asset) {
                                                let call_codes: Vec<&str> = entry.call_signal.split(',').map(|s| s.trim()).collect();
                                                let put_codes: Vec<&str> = entry.put_signal.split(',').map(|s| s.trim()).collect();
                                                if call_codes.contains(&v2_result.status_code.as_str()) {
                                                    decision = "call".to_string();
                                                } else if put_codes.contains(&v2_result.status_code.as_str()) {
                                                    decision = "put".to_string();
                                                }
                                            }

                                            v2_history.push(CompactAnalysis {
                                                time: ic.time,
                                                action: decision,
                                                status_code: v2_result.status_code,
                                            });
                                        }

                                        // Keep last 1000 markers
                                        if v2_history.len() > 1000 {
                                            let skip_amt = v2_history.len() - 1000;
                                            v2_history = v2_history.into_iter().skip(skip_amt).collect();
                                        }

                                        println!("🔍 V2 Historical Analysis: {} markers for {}", v2_history.len(), config.asset);

                                        if !v2_history.is_empty() {
                                            let hist_msg = HistoricalAnalysis {
                                                msg_type: "historical_analysis".to_string(),
                                                symbol: config.asset.clone(),
                                                results: v2_history,
                                            };
                                            let _ = tx.send(BroadcastMessage::HistoricalAnalysis(hist_msg));
                                        }

                                        // Also send initial analysis_data for the signal strip (uses old analysis engine)
                                        let short_ma_type = parse_ma_type(&indicator_config.indicators.short_ema_type);
                                        let medium_ma_type = parse_ma_type(&indicator_config.indicators.medium_ema_type);
                                        let long_ma_type = parse_ma_type(&indicator_config.indicators.long_ema_type);

                                        let analysis_result = generate_analysis_data(
                                            &candles_for_ema,
                                            indicator_config.indicators.short_ema_period,
                                            indicator_config.indicators.medium_ema_period,
                                            indicator_config.indicators.long_ema_period,
                                            short_ma_type,
                                            medium_ma_type,
                                            long_ma_type,
                                        );

                                        if !analysis_result.is_empty() {
                                            let last_index = analysis_result.len() - 1;
                                            let latest = &analysis_result[last_index];

                                            // Determine action based on action_mode config
                                            let (action_str, action_source) = match indicator_config.indicators.action_mode.as_str() {
                                                "simple" => {
                                                    let action = get_action_by_simple(&analysis_result, last_index);
                                                    (action.to_string(), "simple".to_string())
                                                }
                                                "cut_type_short" => {
                                                    let action = get_action_by_cut_type(&analysis_result, last_index, CutStrategy::ShortCut);
                                                    if action == "hold" {
                                                        // Fallback to ema_medium_slope_direction
                                                        let fallback = if latest.ema_medium_slope_direction == "Up" { "call" } else { "put" };
                                                        (fallback.to_string(), "slope_fallback".to_string())
                                                    } else {
                                                        (action.to_string(), "cut_type_short".to_string())
                                                    }
                                                }
                                                _ => { // "cut_type_long" (default)
                                                    let action = get_action_by_cut_type(&analysis_result, last_index, CutStrategy::LongCut);
                                                    if action == "hold" {
                                                        // Fallback to ema_medium_slope_direction
                                                        let fallback = if latest.ema_medium_slope_direction == "Up" { "call" } else { "put" };
                                                        (fallback.to_string(), "slope_fallback".to_string())
                                                    } else {
                                                        (action.to_string(), "cut_type_long".to_string())
                                                    }
                                                }
                                            };

                                            let analysis_msg = AnalysisData {
                                                msg_type: "analysis_data".to_string(),
                                                time: latest.time_candle,
                                                index: latest.index,
                                                color_candle: latest.color_candle.clone(),
                                                next_color_candle: latest.next_color_candle.clone(),

                                                // Short EMA
                                                ema_short_value: latest.ema_short_value,
                                                ema_short_slope_value: latest.ema_short_slope_value,
                                                ema_short_slope_direction: latest.ema_short_slope_direction.clone(),
                                                is_ema_short_turn_type: latest.is_ema_short_turn_type.clone(),
                                                ema_short_cut_position: latest.ema_short_cut_position.clone(),

                                                // Medium EMA
                                                ema_medium_value: latest.ema_medium_value,
                                                ema_medium_slope_direction: latest.ema_medium_slope_direction.clone(),

                                                // Long EMA
                                                ema_long_value: latest.ema_long_value,
                                                ema_long_slope_direction: latest.ema_long_slope_direction.clone(),

                                                // Relationships
                                                ema_above: latest.ema_above.clone(),
                                                ema_long_above: latest.ema_long_above.clone(),

                                                // MACD
                                                macd_12: latest.macd_12,
                                                macd_23: latest.macd_23,

                                                // Previous
                                                previous_ema_short_value: latest.previous_ema_short_value,
                                                previous_ema_medium_value: latest.previous_ema_medium_value,
                                                previous_ema_long_value: latest.previous_ema_long_value,
                                                previous_macd_12: latest.previous_macd_12,
                                                previous_macd_23: latest.previous_macd_23,

                                                // Convergence
                                                ema_convergence_type: latest.ema_convergence_type.clone(),
                                                ema_long_convergence_type: latest.ema_long_convergence_type.clone(),

                                                // Crossovers
                                                ema_cut_short_type: latest.ema_cut_short_type.clone(),
                                                candles_since_short_cut: latest.candles_since_short_cut,
                                                ema_cut_long_type: latest.ema_cut_long_type.clone(),
                                                candles_since_ema_cut: latest.candles_since_ema_cut,

                                                // Historical
                                                previous_color_back1: latest.previous_color_back1.clone(),
                                                previous_color_back3: latest.previous_color_back3.clone(),

                                                // Action
                                                action: action_str.clone(),
                                                action_source: action_source.clone(),
                                            };

                                            println!("📊 Initial Analysis Sent: Action={}, Source={}, MediumSlope={}",
                                                action_str, action_source, latest.ema_medium_slope_direction);

                                            let _ = tx.send(BroadcastMessage::Analysis(analysis_msg));
                                        }
                                    }
                                }

                                // Real-time OHLC
                                else if let Some(ohlc) = json.get("ohlc") {
                                    if let Ok(mut candle) = parse_flexible(ohlc) {
                                        candle.symbol = config.asset.clone();
                                        let _ = tx.send(BroadcastMessage::Candle(candle.clone()));

                                        let current_minute = candle.time / 60;
                                        let seconds = candle.time % 60;

                                        // Update or add candle to the EMA calculation buffer
                                        let indicator_candle = IndicatorCandle {
                                            time: (current_minute * 60), // Normalize to minute boundary
                                            open: candle.open,
                                            high: candle.high,
                                            low: candle.low,
                                            close: candle.close,
                                        };

                                        // Find if this minute's candle exists
                                        if let Some(existing) = candles_for_ema.iter_mut().find(|c| c.time / 60 == current_minute) {
                                            // Update existing candle (use same open, update high/low/close)
                                            existing.high = existing.high.max(candle.high);
                                            existing.low = existing.low.min(candle.low);
                                            existing.close = candle.close;
                                        } else {
                                            // Add new candle
                                            candles_for_ema.push(indicator_candle);
                                            // Keep only last 200 candles
                                            if candles_for_ema.len() > 200 {
                                                candles_for_ema.remove(0);
                                            }
                                        }

                                        // Send EMA update when candle closes (new minute starts)
                                        // Check if we're in the first few seconds of a new minute
                                        if seconds <= 5 && Some(current_minute) != last_ema_minute {
                                            last_ema_minute = Some(current_minute);

                                            if candles_for_ema.len() >= indicator_config.indicators.long_ema_period {
                                                let short_ema = calculate_indicator(
                                                    &candles_for_ema,
                                                    &indicator_config.indicators.short_ema_type,
                                                    indicator_config.indicators.short_ema_period
                                                );
                                                let medium_ema = calculate_indicator(
                                                    &candles_for_ema,
                                                    &indicator_config.indicators.medium_ema_type,
                                                    indicator_config.indicators.medium_ema_period
                                                );
                                                let long_ema = calculate_indicator(
                                                    &candles_for_ema,
                                                    &indicator_config.indicators.long_ema_type,
                                                    indicator_config.indicators.long_ema_period
                                                );

                                                let ema_msg = EmaData {
                                                    msg_type: "ema_data".to_string(),
                                                    short_ema,
                                                    medium_ema,
                                                    long_ema,
                                                    short_period: indicator_config.indicators.short_ema_period,
                                                    medium_period: indicator_config.indicators.medium_ema_period,
                                                    long_period: indicator_config.indicators.long_ema_period,
                                                    short_type: indicator_config.indicators.short_ema_type.clone(),
                                                    medium_type: indicator_config.indicators.medium_ema_type.clone(),
                                                    long_type: indicator_config.indicators.long_ema_type.clone(),
                                                };

                                                println!("📈 EMA updated at candle close: minute {}", current_minute);
                                                let _ = tx.send(BroadcastMessage::EmaData(ema_msg));
                                            }
                                        }

                                        // Send analysis data every 2 seconds with FULL fields
                                        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
                                        let should_send_analysis = match last_analysis_time {
                                            Some(last_time) => now >= last_time + 2,
                                            None => true,
                                        };

                                        // Real analysis logic
                                        // Need at least 2x long_period for reliable analysis (safety margin)
                                        let min_candles = indicator_config.indicators.long_ema_period;

                                        if should_send_analysis && candles_for_ema.len() >= min_candles {
                                            last_analysis_time = Some(now);

                                            // Parse MA types for all 3 EMAs
                                            let short_ma_type = parse_ma_type(&indicator_config.indicators.short_ema_type);
                                            let medium_ma_type = parse_ma_type(&indicator_config.indicators.medium_ema_type);
                                            let long_ma_type = parse_ma_type(&indicator_config.indicators.long_ema_type);

                                            // Run generate_analysis_data with 3 EMA lines
                                            let analysis_result = generate_analysis_data(
                                                &candles_for_ema,
                                                indicator_config.indicators.short_ema_period,
                                                indicator_config.indicators.medium_ema_period,
                                                indicator_config.indicators.long_ema_period,
                                                short_ma_type,
                                                medium_ma_type,
                                                long_ma_type,
                                            );

                                            // Get the latest analysis with ALL fields
                                            if !analysis_result.is_empty() {
                                                let last_index = analysis_result.len() - 1;
                                                let latest = &analysis_result[last_index];

                                                // Determine action based on action_mode config
                                                let (action_str, action_source) = match indicator_config.indicators.action_mode.as_str() {
                                                    "simple" => {
                                                        let action = get_action_by_simple(&analysis_result, last_index);
                                                        (action.to_string(), "simple".to_string())
                                                    }
                                                    "cut_type_short" => {
                                                        let action = get_action_by_cut_type(&analysis_result, last_index, CutStrategy::ShortCut);
                                                        if action == "hold" {
                                                            // Fallback to ema_medium_slope_direction
                                                            let fallback = if latest.ema_medium_slope_direction == "Up" { "call" } else { "put" };
                                                            (fallback.to_string(), "slope_fallback".to_string())
                                                        } else {
                                                            (action.to_string(), "cut_type_short".to_string())
                                                        }
                                                    }
                                                    _ => { // "cut_type_long" (default)
                                                        let action = get_action_by_cut_type(&analysis_result, last_index, CutStrategy::LongCut);
                                                        if action == "hold" {
                                                            // Fallback to ema_medium_slope_direction
                                                            let fallback = if latest.ema_medium_slope_direction == "Up" { "call" } else { "put" };
                                                            (fallback.to_string(), "slope_fallback".to_string())
                                                        } else {
                                                            (action.to_string(), "cut_type_long".to_string())
                                                        }
                                                    }
                                                };

                                                // Store action for trading
                                                last_action = Some(action_str.clone());

                                                let analysis_msg = AnalysisData {
                                                    msg_type: "analysis_data".to_string(),
                                                    time: candle.time, // Current time
                                                    index: latest.index,
                                                    color_candle: latest.color_candle.clone(),
                                                    next_color_candle: latest.next_color_candle.clone(),

                                                    // Short EMA
                                                    ema_short_value: latest.ema_short_value,
                                                    ema_short_slope_value: latest.ema_short_slope_value,
                                                    ema_short_slope_direction: latest.ema_short_slope_direction.clone(),
                                                    is_ema_short_turn_type: latest.is_ema_short_turn_type.clone(),
                                                    ema_short_cut_position: latest.ema_short_cut_position.clone(),

                                                    // Medium EMA
                                                    ema_medium_value: latest.ema_medium_value,
                                                    ema_medium_slope_direction: latest.ema_medium_slope_direction.clone(),

                                                    // Long EMA
                                                    ema_long_value: latest.ema_long_value,
                                                    ema_long_slope_direction: latest.ema_long_slope_direction.clone(),

                                                    // Relationships
                                                    ema_above: latest.ema_above.clone(),
                                                    ema_long_above: latest.ema_long_above.clone(),

                                                    // MACD
                                                    macd_12: latest.macd_12,
                                                    macd_23: latest.macd_23,

                                                    // Previous
                                                    previous_ema_short_value: latest.previous_ema_short_value,
                                                    previous_ema_medium_value: latest.previous_ema_medium_value,
                                                    previous_ema_long_value: latest.previous_ema_long_value,
                                                    previous_macd_12: latest.previous_macd_12,
                                                    previous_macd_23: latest.previous_macd_23,

                                                    // Convergence
                                                    ema_convergence_type: latest.ema_convergence_type.clone(),
                                                    ema_long_convergence_type: latest.ema_long_convergence_type.clone(),

                                                    // Crossovers
                                                    ema_cut_short_type: latest.ema_cut_short_type.clone(),
                                                    candles_since_short_cut: latest.candles_since_short_cut,
                                                    ema_cut_long_type: latest.ema_cut_long_type.clone(),
                                                    candles_since_ema_cut: latest.candles_since_ema_cut,

                                                    // Historical
                                                    previous_color_back1: latest.previous_color_back1.clone(),
                                                    previous_color_back3: latest.previous_color_back3.clone(),

                                                    // Action
                                                    action: action_str.clone(),
                                                    action_source: action_source.clone(),
                                                };

                                                let _ = tx.send(BroadcastMessage::Analysis(analysis_msg));
                                            }
                                        }

                                        // Debug log to see trading conditions
                                        if seconds <= 5 {
                                            println!("⏰ Time: {}:{:02} | Mode: {} | Token: {} | LastMin: {:?} | CurMin: {}",
                                                candle.time / 60 % 60, seconds,
                                                current_trade_mode,
                                                if config.api_token.is_empty() { "NO" } else { "YES" },
                                                last_trade_minute,
                                                current_minute
                                            );
                                        }

                                        // Trading logic - trade at second 0-2 of each minute (more flexible)
                                        if current_trade_mode != "idle" && !config.api_token.is_empty() {
                                            // Trade at second 0-2 of each minute, once per minute
                                            if seconds <= 2 && Some(current_minute) != last_trade_minute {
                                                // Determine contract type based on mode
                                                let contract_type = if current_trade_mode == "auto" {
                                                    // AUTO mode: use action from analysis
                                                    match last_action.as_deref() {
                                                        Some("call") => Some("CALL"),
                                                        Some("put") => Some("PUT"),
                                                        Some("hold") | None => None, // Don't trade on Hold
                                                        _ => None,
                                                    }
                                                } else if current_trade_mode == "call" {
                                                    Some("CALL")
                                                } else if current_trade_mode == "put" {
                                                    Some("PUT")
                                                } else {
                                                    None
                                                };

                                                if let Some(ct) = contract_type {
                                                    last_trade_minute = Some(current_minute);

                                                    let stake = if current_money_mode == "martingale" {
                                                        martingale_stakes[current_stake_index.min(martingale_stakes.len() - 1)]
                                                    } else {
                                                        current_initial_stake
                                                    };

                                                    if balance >= stake {
                                                        let buy_msg = serde_json::json!({
                                                            "buy": "1",
                                                            "price": stake,
                                                            "parameters": {
                                                                "contract_type": ct,
                                                                "symbol": config.asset,
                                                                "duration": current_duration,
                                                                "duration_unit": current_duration_unit,
                                                                "basis": "stake",
                                                                "amount": stake,
                                                                "currency": "USD"
                                                            }
                                                        });

                                                        println!("📈 [{}] Placing {} trade with stake: {} (balance: {})",
                                                            if current_trade_mode == "auto" { "AUTO" } else { "MANUAL" },
                                                            ct, stake, balance);
                                                        pending_contract_type = Some(ct.to_string());
                                                        let _ = write.send(TungsteniteMessage::Text(buy_msg.to_string())).await;
                                                    } else {
                                                        println!("⚠️ Insufficient balance: {} < stake: {}", balance, stake);
                                                    }
                                                } else if current_trade_mode == "auto" {
                                                    println!("⏸️ AUTO mode: Hold signal, skipping trade");
                                                }
                                            }
                                        }
                                    }
                                }

                                // Handle buy response - support both string and number contract_id
                                if let Some(buy) = json.get("buy") {
                                    // Try to get contract_id as string first, then as number
                                    let contract_id = buy.get("contract_id")
                                        .and_then(|c| c.as_str().map(|s| s.to_string()))
                                        .or_else(|| buy.get("contract_id").and_then(|c| c.as_u64().map(|n| n.to_string())));

                                    if let Some(contract_id) = contract_id {
                                        _pending_contract_id = Some(contract_id.clone());
                                        println!("✅ Contract opened: {}", contract_id);

                                        // ส่งข้อมูล trade ที่เปิดไปให้ frontend
                                        let now = Local::now();
                                        let stake = buy.get("buy_price").and_then(|p| p.as_f64()).unwrap_or(0.0);

                                        let trade_opened = TradeOpened {
                                            msg_type: "trade_opened".to_string(),
                                            contract_id: contract_id.clone(),
                                            asset: config.asset.clone(),
                                            trade_type: pending_contract_type.clone().unwrap_or_else(|| current_trade_mode.to_uppercase()),
                                            stake,
                                            time: now.format("%H:%M:%S").to_string(),
                                        };
                                        let _ = tx.send(BroadcastMessage::TradeOpened(trade_opened));

                                        // Subscribe to contract for result
                                        let proposal_msg = serde_json::json!({
                                            "proposal_open_contract": 1,
                                            "contract_id": contract_id,
                                            "subscribe": 1
                                        });
                                        let _ = write.send(TungsteniteMessage::Text(proposal_msg.to_string())).await;
                                    } else {
                                        println!("⚠️ Buy response received but no contract_id found: {}", serde_json::to_string_pretty(&buy).unwrap_or_default());
                                    }
                                } else if json.get("error").is_none() && json.get("msg_type").and_then(|m| m.as_str()) == Some("buy") {
                                    // Log if we got a buy response but couldn't parse it
                                    println!("⚠️ Unexpected buy response format: {}", raw_text);
                                }

                                // Handle contract updates and result
                                if let Some(proposal) = json.get("proposal_open_contract") {
                                    let contract_id = proposal.get("contract_id")
                                        .and_then(|c| c.as_str().map(|s| s.to_string()))
                                        .or_else(|| proposal.get("contract_id").and_then(|c| c.as_u64().map(|n| n.to_string())))
                                        .unwrap_or_default();

                                    let status = proposal.get("status").and_then(|s| s.as_str()).unwrap_or("open");
                                    let is_sold = proposal.get("is_sold").and_then(|v| v.as_u64()).unwrap_or(0) == 1;
                                    let is_expired = proposal.get("is_expired").and_then(|v| v.as_u64()).unwrap_or(0) == 1;

                                    let asset = proposal.get("underlying").and_then(|s| s.as_str()).unwrap_or("").to_string();
                                    let trade_type = proposal.get("contract_type").and_then(|s| s.as_str()).unwrap_or("").to_string();

                                    // Send real-time updates while contract is open
                                    if status == "open" {
                                        let current_spot = proposal.get("current_spot").and_then(|v| v.as_f64())
                                            .or_else(|| proposal.get("current_spot").and_then(|v| v.as_str()?.parse().ok()))
                                            .unwrap_or(0.0);
                                        let entry_spot = proposal.get("entry_spot").and_then(|v| v.as_f64())
                                            .or_else(|| proposal.get("entry_spot").and_then(|v| v.as_str()?.parse().ok()))
                                            .unwrap_or(0.0);
                                        let profit = proposal.get("profit").and_then(|p| p.as_f64()).unwrap_or(0.0);
                                        let profit_percentage = proposal.get("profit_percentage").and_then(|p| p.as_f64()).unwrap_or(0.0);
                                        let payout = proposal.get("payout").and_then(|p| p.as_f64()).unwrap_or(0.0);
                                        let buy_price = proposal.get("buy_price").and_then(|p| p.as_f64()).unwrap_or(0.0);
                                        let date_expiry = proposal.get("date_expiry").and_then(|d| d.as_u64()).unwrap_or(0);
                                        let date_start = proposal.get("date_start").and_then(|d| d.as_u64()).unwrap_or(0);

                                        let trade_update = TradeUpdate {
                                            msg_type: "trade_update".to_string(),
                                            contract_id: contract_id.clone(),
                                            asset: asset.clone(),
                                            trade_type: trade_type.clone(),
                                            current_spot,
                                            entry_spot,
                                            profit,
                                            profit_percentage,
                                            is_sold,
                                            is_expired,
                                            payout,
                                            buy_price,
                                            date_expiry,
                                            date_start,
                                        };
                                        let _ = tx.send(BroadcastMessage::TradeUpdate(trade_update));
                                    }

                                    // Handle final result
                                    if status == "sold" || status == "won" || status == "lost" {
                                        let profit = proposal.get("profit").and_then(|p| p.as_f64()).unwrap_or(0.0);
                                        let stake = proposal.get("buy_price").and_then(|p| p.as_f64()).unwrap_or(0.0);

                                        balance += profit;

                                        let is_win = profit > 0.0;

                                        if lot_active {
                                            lot_grand_profit += profit;
                                            if is_win {
                                                lot_win_count += 1;
                                            }
                                            trade_count_in_lot += 1;
                                        }

                                        if is_win {
                                            current_stake_index = 0;
                                            println!("🎉 WIN! Profit: {}, Balance: {}", profit, balance);
                                        } else {
                                            if current_money_mode == "martingale" {
                                                current_stake_index = (current_stake_index + 1).min(martingale_stakes.len() - 1);
                                            }
                                            println!("❌ LOSS! Loss: {}, Balance: {}", profit, balance);
                                        }

                                        let result = TradeResult {
                                            msg_type: "trade_result".to_string(),
                                            status: if is_win { "win".to_string() } else { "loss".to_string() },
                                            balance,
                                            stake,
                                            profit,
                                            contract_id: Some(contract_id.clone()),
                                        };

                                        let _ = tx.send(BroadcastMessage::TradeResult(result));

                                        // Check Stop Conditions
                                        let mut stop_trading = false;
                                        let mut stop_reason = String::new();

                                        if lot_active {
                                            if current_money_mode == "fix" {
                                                if lot_grand_profit >= indicator_config.trading.target_grand_profit {
                                                    stop_trading = true;
                                                    stop_reason = format!("Target Grand Profit ({}) Reached", indicator_config.trading.target_grand_profit);
                                                }
                                            } else if current_money_mode == "martingale" {
                                                if lot_win_count >= indicator_config.trading.target_win_count {
                                                    stop_trading = true;
                                                    stop_reason = format!("Target Win Count ({}) Reached", indicator_config.trading.target_win_count);
                                                }
                                            }
                                        }

                                        // Save Trade History
                                        if lot_active {
                                            let trade_obj = TradeObject {
                                                lot_no: current_lot_no,
                                                trade_no_on_this_lot: trade_count_in_lot,
                                                trade_time: Local::now().format("%d-%m-%Y %H:%M:%S").to_string(),
                                                asset: config.asset.clone(),
                                                action: trade_type.to_lowercase(),
                                                money_trade: stake,
                                                money_trade_type: if current_money_mode == "fix" { "Fixed".to_string() } else { "Martingale".to_string() },
                                                win_status: if is_win { "win".to_string() } else { "loss".to_string() },
                                                profit,
                                                balance_on_lot: lot_grand_profit,
                                                win_con: indicator_config.trading.target_win_count.to_string(),
                                                loss_con: indicator_config.trading.target_grand_profit.to_string(),
                                                is_stop_trade: stop_trading,
                                            };

                                            trades_for_lot.push(trade_obj);

                                            let lot_log = LotLog {
                                                lot_no: current_lot_no,
                                                trade_object_list: trades_for_lot.clone(),
                                            };
                                            save_lot_log(&ensure_daily_folder(&get_daily_folder_name()), &lot_log);
                                            println!("💾 Saved Trade History for Lot {}", current_lot_no);

                                            // Save to Firestore
                                            let date_start_val = proposal.get("date_start").and_then(|d| d.as_u64()).unwrap_or(0);
                                            let date_expiry_val = proposal.get("date_expiry").and_then(|d| d.as_u64()).unwrap_or(0);
                                            let payout_val = proposal.get("payout").and_then(|p| p.as_f64()).unwrap_or(0.0);
                                            let entry_spot_val = proposal.get("entry_spot").and_then(|v| v.as_f64())
                                                .or_else(|| proposal.get("entry_spot").and_then(|v| v.as_str()?.parse().ok()))
                                                .unwrap_or(0.0);
                                            let exit_spot_val = proposal.get("exit_spot").and_then(|v| v.as_f64())
                                                .or_else(|| proposal.get("exit_spot").and_then(|v| v.as_str()?.parse().ok()))
                                                .unwrap_or(0.0);

                                            let trade_record = TradeRecord {
                                                order_no: trade_count_in_lot,
                                                contract_id: contract_id.clone(),
                                                symbol: config.asset.clone(),
                                                trade_type: trade_type.clone(),
                                                buy_price: stake,
                                                payout: payout_val,
                                                profit_loss: profit,
                                                buy_time: date_start_val,
                                                expiry_time: date_expiry_val,
                                                time_remaining: 0, // Trade has ended
                                                min_profit: profit, // Final value
                                                max_profit: profit, // Final value
                                                status: if is_win { "win".to_string() } else { "loss".to_string() },
                                                entry_spot: entry_spot_val,
                                                exit_spot: exit_spot_val,
                                                lot_no: current_lot_no,
                                                trade_no_in_lot: trade_count_in_lot,
                                                trade_date: Local::now().format("%Y-%m-%d").to_string(),
                                                created_at: Local::now().format("%Y-%m-%dT%H:%M:%S").to_string(),
                                            };

                                            // Save to Firestore asynchronously
                                            let fs = firestore.lock().await;
                                            match fs.save_trade(&trade_record).await {
                                                Ok(doc_id) => println!("🔥 Trade saved to Firestore: {}", doc_id),
                                                Err(e) => println!("⚠️ Firestore save error: {}", e),
                                            }
                                        }

                                        if stop_trading {
                                            println!("🛑 STOPPING TRADE: {}", stop_reason);
                                            current_trade_mode = "idle".to_string();
                                            lot_active = false;
                                        }

                                        // Broadcast Lot Status
                                        let _ = tx.send(BroadcastMessage::LotStatus(LotStatus {
                                            msg_type: "lot_status".to_string(),
                                            grand_profit: lot_grand_profit,
                                            win_count: lot_win_count,
                                            target_profit: indicator_config.trading.target_grand_profit,
                                            target_win: indicator_config.trading.target_win_count,
                                            lot_active, // Frontend should switch to IDLE if this is false
                                            balance: 0.0,
                                        }));
                                    }
                                }
                            }
                        } else {
                            break;
                        }
                    }
                }
            }

            println!("🔌 Deriv connection closed for: {}", config.asset);
        }
        Err(e) => println!("❌ Deriv Connection Failed: {}", e),
    }
}

// ============================================================================
//  MULTI-ASSET PARALLEL ANALYSIS (V2)
//  Uses incremental V2 AnalysisGenerator from RustLib/indicator_math.
//  Each asset gets its own generator, processing candles incrementally.
//  CandleMasterCode is used for StatusCode resolution.
// ============================================================================

/// Build CandleMasterCode list from the same mapping as old lib's build_status_code_map
fn build_candle_master_codes() -> Vec<CandleMasterCode> {
    let pairs: Vec<(&str, u32)> = vec![
        ("L-D-D-G-C", 2),
        ("L-D-D-G-D", 3),
        ("L-D-D-G-N", 4),
        ("L-D-D-R-C", 5),
        ("L-D-D-R-D", 6),
        ("L-D-D-R-N", 7),
        ("L-D-F-G-C", 8),
        ("L-D-F-G-D", 9),
        ("L-D-F-G-N", 10),
        ("L-D-F-R-C", 11),
        ("L-D-F-R-D", 12),
        ("L-D-F-R-N", 13),
        ("L-D-U-G-C", 14),
        ("L-D-U-G-D", 15),
        ("L-D-U-G-N", 16),
        ("L-D-U-R-C", 17),
        ("L-D-U-R-D", 18),
        ("L-D-U-R-N", 19),
        ("L-F-D-G-C", 20),
        ("L-F-D-G-N", 21),
        ("L-F-D-R-C", 22),
        ("L-F-D-R-N", 23),
        ("L-F-F-G-C", 24),
        ("L-F-F-G-N", 25),
        ("L-F-F-R-N", 26),
        ("L-F-U-G-C", 27),
        ("L-F-U-G-D", 28),
        ("L-F-U-G-N", 29),
        ("L-F-U-R-D", 30),
        ("L-F-U-R-N", 31),
        ("L-U-D-G-C", 32),
        ("L-U-D-G-N", 33),
        ("L-U-D-R-C", 34),
        ("L-U-D-R-N", 35),
        ("L-U-F-G-C", 36),
        ("L-U-F-G-N", 37),
        ("L-U-U-G-C", 38),
        ("L-U-U-G-D", 39),
        ("L-U-U-G-N", 40),
        ("L-U-U-R-D", 41),
        ("L-U-U-R-N", 42),
        ("M-D-D-G-C", 43),
        ("M-D-D-G-D", 44),
        ("M-D-D-G-N", 45),
        ("M-D-D-R-C", 46),
        ("M-D-D-R-D", 47),
        ("M-D-D-R-N", 48),
        ("M-D-F-G-C", 49),
        ("M-D-F-G-N", 50),
        ("M-D-F-R-C", 51),
        ("M-D-F-R-N", 52),
        ("M-D-U-G-C", 53),
        ("M-D-U-G-N", 54),
        ("M-D-U-R-C", 55),
        ("M-D-U-R-N", 56),
        ("M-F-D-G-C", 57),
        ("M-F-D-G-D", 58),
        ("M-F-D-G-N", 59),
        ("M-F-D-R-D", 60),
        ("M-F-D-R-N", 61),
        ("M-F-U-G-C", 62),
        ("M-F-U-G-N", 63),
        ("M-F-U-R-C", 64),
        ("M-F-U-R-N", 65),
        ("M-U-D-G-C", 67),
        ("M-U-D-G-D", 68),
        ("M-U-D-G-N", 69),
        ("M-U-D-R-C", 70),
        ("M-U-D-R-D", 71),
        ("M-U-D-R-N", 72),
        ("M-U-F-G-C", 73),
        ("M-U-F-G-D", 74),
        ("M-U-F-G-N", 75),
        ("M-U-F-R-D", 76),
        ("M-U-U-G-C", 79),
        ("M-U-U-G-D", 80),
        ("M-U-U-G-N", 81),
        ("M-U-U-R-C", 82),
        ("M-U-U-R-D", 83),
        ("M-U-U-R-N", 84),
    ];
    pairs
        .into_iter()
        .map(|(desc, code)| CandleMasterCode {
            status_desc: desc.to_string(),
            status_code: code.to_string(),
        })
        .collect()
}

async fn connect_multi_asset(
    tx: broadcast::Sender<BroadcastMessage>,
    config: ClientCommand,
    mut cmd_rx: tokio::sync::mpsc::Receiver<String>,
    _firestore: Arc<tokio::sync::Mutex<GlobalFirestore>>,
) {
    // 1. Load tradeSignal.json
    let signal_entries: Vec<TradeSignalEntry> = match fs::read_to_string("tradeSignal.json") {
        Ok(content) => match serde_json::from_str(&content) {
            Ok(entries) => entries,
            Err(e) => {
                println!("❌ Failed to parse tradeSignal.json: {}", e);
                return;
            }
        },
        Err(e) => {
            println!("❌ Failed to read tradeSignal.json: {}", e);
            return;
        }
    };

    let active_assets: Vec<&TradeSignalEntry> = signal_entries
        .iter()
        .filter(|e| e.is_active == "y")
        .collect();

    let asset_symbols: Vec<String> = active_assets.iter().map(|a| a.asset_code.clone()).collect();
    println!(
        "📊 Multi-Asset V2: {} active assets: {:?}",
        asset_symbols.len(),
        asset_symbols
    );

    // Build CandleMasterCode list for StatusCode resolution
    let master_codes = build_candle_master_codes();
    let master_codes_arc = std::sync::Arc::new(master_codes);
    let v2_options = V2AnalysisOptions::default();

    // 2. Connect to Deriv API
    let app_id = if config.app_id.is_empty() {
        "66726".to_string()
    } else {
        config.app_id
    };
    let url = format!("wss://ws.derivws.com/websockets/v3?app_id={}", app_id);
    println!("🌐 Multi-Asset V2: Connecting to Deriv API...");

    match tokio_tungstenite::connect_async(&url).await {
        Ok((ws_stream, _)) => {
            println!("✅ Multi-Asset V2: Connected to Deriv");
            let (mut write, mut read) = ws_stream.split();

            // Authorize if token provided
            if !config.api_token.is_empty() {
                let auth_msg = serde_json::json!({ "authorize": config.api_token });
                let _ = write
                    .send(TungsteniteMessage::Text(auth_msg.to_string()))
                    .await;

                // Wait and clear the auth response so it doesn't break ticks_history
                if let Ok(Some(Ok(TungsteniteMessage::Text(text)))) =
                    tokio::time::timeout(tokio::time::Duration::from_secs(3), read.next()).await
                {
                    println!(
                        "🔑 Auth response: {}",
                        text.chars().take(200).collect::<String>()
                    );
                }
            }

            // 3. Fetch historical candles and initialize V2 generators per asset
            let mut generators: std::collections::HashMap<String, V2AnalysisGenerator> =
                std::collections::HashMap::new();
            // Track current forming candle per asset (open_time -> OHLC)
            let mut current_candle: std::collections::HashMap<String, (u64, f64, f64, f64, f64)> =
                std::collections::HashMap::new();

            // === PARALLEL FETCH: Send ALL history requests at once, then collect responses ===
            // Instead of: send req → wait → send next → wait (sequential: ~3s × 10 = 30s)
            // Now:         send ALL reqs → collect ALL responses (parallel: ~3-5s total)

            // Step A: Blast out all ticks_history requests at once
            for asset in &asset_symbols {
                println!("📥 Requesting history for {}...", asset);
                let req = serde_json::json!({
                    "ticks_history": asset,
                    "adjust_start_time": 1,
                    "count": 1000,
                    "end": "latest",
                    "style": "candles",
                    "granularity": 60
                });
                let _ = write.send(TungsteniteMessage::Text(req.to_string())).await;
                // Small delay to avoid rate limiting
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            }

            // Step B: Collect ALL responses — match each to its asset via echo_req
            let mut pending_assets: std::collections::HashSet<String> =
                asset_symbols.iter().cloned().collect();
            let fetch_timeout = tokio::time::Duration::from_secs(30);
            let fetch_start = tokio::time::Instant::now();

            while !pending_assets.is_empty() && fetch_start.elapsed() < fetch_timeout {
                if let Ok(Some(Ok(TungsteniteMessage::Text(text)))) =
                    tokio::time::timeout(tokio::time::Duration::from_secs(5), read.next()).await
                {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                        if let Some(error) = json.get("error") {
                            println!("❌ AutoTrade history error: {}", error);
                        }

                        // Check if this is a candles response
                        if let Some(candles_arr) = json.get("candles").and_then(|c| c.as_array()) {
                            let resp_asset = json
                                .get("echo_req")
                                .and_then(|e| e.get("ticks_history"))
                                .and_then(|a| a.as_str())
                                .unwrap_or("")
                                .to_string();

                            if !pending_assets.contains(&resp_asset) {
                                continue;
                            }

                            // Create V2 generator and feed historical candles
                            let mut gen = V2AnalysisGenerator::new(
                                v2_options.clone(),
                                master_codes_arc.clone(),
                            );
                            let mut count = 0;
                            let mut historical_results = Vec::new();

                            for c in candles_arr {
                                let time = c.get("epoch").and_then(|v| v.as_u64()).unwrap_or(0);
                                let open = c
                                    .get("open")
                                    .and_then(|v| v.as_f64())
                                    .or_else(|| {
                                        c.get("open").and_then(|v| v.as_str()?.parse().ok())
                                    })
                                    .unwrap_or(0.0);
                                let high = c
                                    .get("high")
                                    .and_then(|v| v.as_f64())
                                    .or_else(|| {
                                        c.get("high").and_then(|v| v.as_str()?.parse().ok())
                                    })
                                    .unwrap_or(0.0);
                                let low = c
                                    .get("low")
                                    .and_then(|v| v.as_f64())
                                    .or_else(|| c.get("low").and_then(|v| v.as_str()?.parse().ok()))
                                    .unwrap_or(0.0);
                                let close = c
                                    .get("close")
                                    .and_then(|v| v.as_f64())
                                    .or_else(|| {
                                        c.get("close").and_then(|v| v.as_str()?.parse().ok())
                                    })
                                    .unwrap_or(0.0);

                                let final_state = gen.append_candle(V2Candle {
                                    time,
                                    open,
                                    high,
                                    low,
                                    close,
                                });

                                let mut decision = "idle".to_string();
                                if let Some(entry) =
                                    signal_entries.iter().find(|e| e.asset_code == resp_asset)
                                {
                                    let call_codes: Vec<&str> =
                                        entry.call_signal.split(',').map(|s| s.trim()).collect();
                                    let put_codes: Vec<&str> =
                                        entry.put_signal.split(',').map(|s| s.trim()).collect();
                                    if call_codes.contains(&final_state.status_code.as_str()) {
                                        decision = "call".to_string();
                                    } else if put_codes.contains(&final_state.status_code.as_str())
                                    {
                                        decision = "put".to_string();
                                    }
                                }

                                historical_results.push(CompactAnalysis {
                                    time,
                                    action: decision,
                                    status_code: final_state.status_code,
                                });
                                count += 1;
                            }

                            if let Some(ref last) = gen.state.last_analysis {
                                println!(
                                    "  ✅ {} loaded {} candles | StatusCode={} StatusDesc={}",
                                    resp_asset, count, last.status_code, last.status_desc
                                );
                            } else {
                                println!(
                                    "  ✅ {} loaded {} candles (no analysis yet)",
                                    resp_asset, count
                                );
                            }

                            // Send history to frontend
                            if historical_results.len() > 1000 {
                                let skip_amt = historical_results.len() - 1000;
                                historical_results =
                                    historical_results.into_iter().skip(skip_amt).collect();
                            }
                            let hist_msg = HistoricalAnalysis {
                                msg_type: "historical_analysis".to_string(),
                                symbol: resp_asset.clone(),
                                results: historical_results,
                            };
                            let _ = tx.send(BroadcastMessage::HistoricalAnalysis(hist_msg));

                            generators.insert(resp_asset.clone(), gen);
                            pending_assets.remove(&resp_asset);
                            println!(
                                "  📊 {} remaining: {:?}",
                                pending_assets.len(),
                                pending_assets
                            );
                        }
                    }
                } else {
                    println!(
                        "⏱️ Timeout waiting for history response, {} pending",
                        pending_assets.len()
                    );
                    break;
                }
            }

            if !pending_assets.is_empty() {
                println!("⚠️ Failed to load history for: {:?}", pending_assets);
            }
            println!(
                "⚡ Parallel fetch completed in {:.1}s — {} generators ready",
                fetch_start.elapsed().as_secs_f64(),
                generators.len()
            );

            println!(
                "📊 All {} generators ready. Subscribing to live candles...",
                generators.len()
            );

            // 4. Subscribe to live candles for ALL assets
            let mut sub_ids: Vec<String> = Vec::new();
            for asset in &asset_symbols {
                let sub_msg = serde_json::json!({
                    "ticks_history": asset,
                    "subscribe": 1,
                    "style": "candles",
                    "granularity": 60,
                    "count": 1,
                    "end": "latest",
                    "adjust_start_time": 1
                });
                let _ = write
                    .send(TungsteniteMessage::Text(sub_msg.to_string()))
                    .await;
                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
            }

            let mut last_check_minute: Option<u64> = None;
            let mut balance = 1000.0;
            let _ = balance; // silence unused assignment warning

            // 5. Main event loop
            loop {
                tokio::select! {
                    cmd = cmd_rx.recv() => {
                        if let Some(cmd) = cmd {
                            if cmd == "FORGET" || cmd == "STOP_STREAMS" {
                                println!("🛑 Multi-Asset V2: Stopping all streams...");
                                for id in &sub_ids {
                                    let forget_msg = serde_json::json!({"forget": id});
                                    let _ = write.send(TungsteniteMessage::Text(forget_msg.to_string())).await;
                                }
                                break;
                            }
                        }
                    }

                    msg = read.next() => {
                        if let Some(Ok(TungsteniteMessage::Text(raw_text))) = msg {
                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&raw_text) {

                                // Track subscription IDs
                                if let Some(sub) = json.get("subscription") {
                                    if let Some(id) = sub.get("id").and_then(|i| i.as_str()) {
                                        if !sub_ids.contains(&id.to_string()) {
                                            sub_ids.push(id.to_string());
                                        }
                                    }
                                }

                                // Get balance from authorize
                                if let Some(authorize) = json.get("authorize") {
                                    if let Some(bal) = authorize.get("balance").and_then(|b| b.as_f64()) {
                                        balance = bal;
                                        let balance_msg = BalanceMessage {
                                            msg_type: "balance".to_string(),
                                            balance,
                                        };
                                        let _ = tx.send(BroadcastMessage::Balance(balance_msg));
                                    }
                                }

                                // Handle OHLC updates (real-time candle for subscribed assets)
                                if let Some(ohlc) = json.get("ohlc") {
                                    let symbol = ohlc.get("symbol").and_then(|s| s.as_str()).unwrap_or("").to_string();
                                    let epoch = ohlc.get("epoch").and_then(|v| v.as_u64())
                                        .or_else(|| ohlc.get("epoch").and_then(|v| v.as_str()?.parse().ok()))
                                        .unwrap_or(0);
                                    let open_time = ohlc.get("open_time").and_then(|v| v.as_u64())
                                        .or_else(|| ohlc.get("open_time").and_then(|v| v.as_str()?.parse().ok()))
                                        .unwrap_or(0);
                                    let o = ohlc.get("open").and_then(|v| v.as_f64())
                                        .or_else(|| ohlc.get("open").and_then(|v| v.as_str()?.parse().ok())).unwrap_or(0.0);
                                    let h = ohlc.get("high").and_then(|v| v.as_f64())
                                        .or_else(|| ohlc.get("high").and_then(|v| v.as_str()?.parse().ok())).unwrap_or(0.0);
                                    let l = ohlc.get("low").and_then(|v| v.as_f64())
                                        .or_else(|| ohlc.get("low").and_then(|v| v.as_str()?.parse().ok())).unwrap_or(0.0);
                                    let c = ohlc.get("close").and_then(|v| v.as_f64())
                                        .or_else(|| ohlc.get("close").and_then(|v| v.as_str()?.parse().ok())).unwrap_or(0.0);

                                    if !symbol.is_empty() && open_time > 0 {
                                        // Update current forming candle
                                        let prev_open_time = current_candle.get(&symbol).map(|cc| cc.0).unwrap_or(0);

                                        if open_time != prev_open_time && prev_open_time > 0 {
                                            // New candle started → previous candle is complete
                                            // Feed completed candle to V2 generator (incremental)
                                            if let Some((pt, po, ph, pl, pc)) = current_candle.get(&symbol) {
                                                if let Some(gen) = generators.get_mut(&symbol) {
                                                    let completed = V2Candle {
                                                        time: *pt, open: *po, high: *ph,
                                                        low: *pl, close: *pc,
                                                    };
                                                    let result = gen.append_candle(completed);
                                                    println!(
                                                        "  📊 {} candle closed | StatusCode={} Desc={}",
                                                        symbol, result.status_code, result.status_desc
                                                    );
                                                }
                                            }
                                        }

                                        // Update current candle
                                        current_candle.insert(symbol.clone(), (open_time, o, h, l, c));

                                        // Check if we are at second 0-2 of a new minute
                                        let current_minute = epoch / 60;
                                        let seconds = epoch % 60;

                                        if seconds <= 2 && Some(current_minute) != last_check_minute {
                                            last_check_minute = Some(current_minute);

                                            // === SIGNAL CHECK for ALL assets ===
                                            let mut signal_results: Vec<AssetSignalResult> = Vec::new();

                                            for entry in &signal_entries {
                                                if entry.is_active != "y" { continue; }

                                                let asset_code = &entry.asset_code;
                                                // Get latest analysis from V2 generator
                                                if let Some(gen) = generators.get(asset_code) {
                                                    if let Some(ref analysis) = gen.state.last_analysis {
                                                        let code_str = &analysis.status_code;

                                                        // Parse signal codes from tradeSignal.json
                                                        let call_codes: Vec<&str> = entry.call_signal.split(',').map(|s| s.trim()).collect();
                                                        let put_codes: Vec<&str> = entry.put_signal.split(',').map(|s| s.trim()).collect();

                                                        let (decision, reason) = if call_codes.contains(&code_str.as_str()) {
                                                            ("call".to_string(), format!("StatusCode {} matched CallSignal", code_str))
                                                        } else if put_codes.contains(&code_str.as_str()) {
                                                            ("put".to_string(), format!("StatusCode {} matched PutSignal", code_str))
                                                        } else {
                                                            ("idle".to_string(), format!("StatusCode {} — no match", code_str))
                                                        };

                                                        println!("  📊 {} | Code={} | Desc={} | Decision={}",
                                                            asset_code, code_str, analysis.status_desc, decision);

                                                        signal_results.push(AssetSignalResult {
                                                            asset: asset_code.clone(),
                                                            status_code: code_str.clone(),
                                                            status_desc: analysis.status_desc.clone(),
                                                            decision,
                                                            reason,
                                                            close_price: analysis.close,
                                                            ema_short_dir: analysis.ema_short_direction.clone(),
                                                            ema_medium_dir: analysis.ema_medium_direction.clone(),
                                                            ema_long_dir: analysis.ema_long_direction.clone(),
                                                        });
                                                    }
                                                }
                                            }

                                            if !signal_results.is_empty() {
                                                println!("📡 Broadcasting multi_analysis: {} assets at minute {}",
                                                    signal_results.len(), current_minute);

                                                let multi_msg = MultiAnalysisMessage {
                                                    msg_type: "multi_analysis".to_string(),
                                                    timestamp: epoch,
                                                    assets: signal_results,
                                                };
                                                let _ = tx.send(BroadcastMessage::MultiAnalysis(multi_msg));
                                            }
                                        }
                                    }
                                }

                                // Handle errors
                                if let Some(error) = json.get("error") {
                                    println!("❌ Multi-Asset API Error: {}",
                                        error.get("message").unwrap_or(&serde_json::json!("Unknown")));
                                }
                            }
                        } else {
                            break;
                        }
                    }
                }
            }

            println!("🔌 Multi-Asset V2: Connection closed.");
        }
        Err(e) => println!("❌ Multi-Asset V2: Connection Failed: {}", e),
    }
}

// ============================================================================
//  AUTO MULTI-ASSET TRADE (Browser-Independent)
//  Runs as a detached tokio task. When browser disconnects, trading continues.
//  Uses indicator_math_v2 (RustLib/indicator_math) for parallel analysis.
//  At second=0 of each minute, checks StatusCode against tradeSignal.json.
//  Trades assets with matching CALL/PUT signals until conditions are met.
// ============================================================================

async fn auto_multi_trade(
    tx: broadcast::Sender<BroadcastMessage>,
    config: ClientCommand,
    mut cmd_rx: tokio::sync::mpsc::Receiver<String>,
    firestore: Arc<tokio::sync::Mutex<GlobalFirestore>>,
) {
    println!("🤖 ====== AUTO MULTI-TRADE STARTED ======");
    println!("   Assets: {:?}", config.assets);
    println!(
        "   Stake: {}, Mode: {}, Duration: {}{}",
        config.initial_stake, config.money_mode, config.duration, config.duration_unit
    );
    println!(
        "   Target Profit: {}, Target Win: {}",
        config.target_profit, config.target_win
    );

    // 1. Load tradeSignal.json
    let signal_entries: Vec<TradeSignalEntry> = match fs::read_to_string("tradeSignal.json") {
        Ok(content) => match serde_json::from_str(&content) {
            Ok(entries) => entries,
            Err(e) => {
                println!("❌ AutoTrade: Failed to parse tradeSignal.json: {}", e);
                return;
            }
        },
        Err(e) => {
            println!("❌ AutoTrade: Failed to read tradeSignal.json: {}", e);
            return;
        }
    };

    // Use assets from command (checked checkboxes), filtered by tradeSignal active entries
    let asset_symbols: Vec<String> = if config.assets.is_empty() {
        // Fallback: use all active from tradeSignal.json
        signal_entries
            .iter()
            .filter(|e| e.is_active == "y")
            .map(|e| e.asset_code.clone())
            .collect()
    } else {
        config.assets.clone()
    };

    if asset_symbols.is_empty() {
        println!("❌ AutoTrade: No assets selected!");
        return;
    }

    println!(
        "📊 AutoTrade: {} assets to analyze: {:?}",
        asset_symbols.len(),
        asset_symbols
    );

    // Build CandleMasterCode list for StatusCode resolution
    let master_codes = build_candle_master_codes();
    let master_codes_arc = std::sync::Arc::new(master_codes);
    let v2_options = V2AnalysisOptions::default();

    // 2. Connect to Deriv API
    let app_id = if config.app_id.is_empty() {
        "66726".to_string()
    } else {
        config.app_id.clone()
    };
    let url = format!("wss://ws.derivws.com/websockets/v3?app_id={}", app_id);
    println!("🌐 AutoTrade: Connecting to Deriv API...");

    match tokio_tungstenite::connect_async(&url).await {
        Ok((ws_stream, _)) => {
            println!("✅ AutoTrade: Connected to Deriv");
            let (mut write, mut read) = ws_stream.split();

            // Authorize if token provided
            if !config.api_token.is_empty() {
                let auth_msg = serde_json::json!({ "authorize": config.api_token });
                let _ = write
                    .send(TungsteniteMessage::Text(auth_msg.to_string()))
                    .await;

                // Wait and clear the auth response so it doesn't break ticks_history
                if let Ok(Some(Ok(TungsteniteMessage::Text(text)))) =
                    tokio::time::timeout(tokio::time::Duration::from_secs(3), read.next()).await
                {
                    println!(
                        "🔑 Auth response: {}",
                        text.chars().take(200).collect::<String>()
                    );
                }
            }

            // 3. Fetch historical candles and initialize V2 generators per asset (PARALLEL)
            let mut generators: std::collections::HashMap<String, V2AnalysisGenerator> =
                std::collections::HashMap::new();
            let mut current_candle: std::collections::HashMap<String, (u64, f64, f64, f64, f64)> =
                std::collections::HashMap::new();

            for asset in &asset_symbols {
                println!("📥 AutoTrade: Fetching history for {}...", asset);
                let req = serde_json::json!({
                    "ticks_history": asset,
                    "adjust_start_time": 1,
                    "count": 1000,
                    "end": "latest",
                    "style": "candles",
                    "granularity": 60
                });
                let _ = write.send(TungsteniteMessage::Text(req.to_string())).await;

                // Wait for response, skipping non-matching messages
                let timeout = tokio::time::Duration::from_secs(10);
                let start_wait = tokio::time::Instant::now();

                while start_wait.elapsed() < timeout {
                    if let Ok(Some(Ok(TungsteniteMessage::Text(text)))) =
                        tokio::time::timeout(tokio::time::Duration::from_secs(2), read.next()).await
                    {
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                            if let Some(error) = json.get("error") {
                                println!("❌ AutoTrade history error: {}", error);
                            }
                            if let Some(candles_arr) =
                                json.get("candles").and_then(|c| c.as_array())
                            {
                                // Confirm matching asset
                                let resp_asset = json
                                    .get("echo_req")
                                    .and_then(|e| e.get("ticks_history"))
                                    .and_then(|a| a.as_str())
                                    .unwrap_or("");
                                if resp_asset != *asset {
                                    continue;
                                }

                                let mut gen = V2AnalysisGenerator::new(
                                    v2_options.clone(),
                                    master_codes_arc.clone(),
                                );
                                let mut count = 0;
                                let mut historical_results = Vec::new();

                                for c in candles_arr {
                                    let time = c.get("epoch").and_then(|v| v.as_u64()).unwrap_or(0);
                                    let open = c
                                        .get("open")
                                        .and_then(|v| v.as_f64())
                                        .or_else(|| {
                                            c.get("open").and_then(|v| v.as_str()?.parse().ok())
                                        })
                                        .unwrap_or(0.0);
                                    let high = c
                                        .get("high")
                                        .and_then(|v| v.as_f64())
                                        .or_else(|| {
                                            c.get("high").and_then(|v| v.as_str()?.parse().ok())
                                        })
                                        .unwrap_or(0.0);
                                    let low = c
                                        .get("low")
                                        .and_then(|v| v.as_f64())
                                        .or_else(|| {
                                            c.get("low").and_then(|v| v.as_str()?.parse().ok())
                                        })
                                        .unwrap_or(0.0);
                                    let close = c
                                        .get("close")
                                        .and_then(|v| v.as_f64())
                                        .or_else(|| {
                                            c.get("close").and_then(|v| v.as_str()?.parse().ok())
                                        })
                                        .unwrap_or(0.0);

                                    let final_state = gen.append_candle(V2Candle {
                                        time,
                                        open,
                                        high,
                                        low,
                                        close,
                                    });

                                    // Collect history for markers (keep last 100)
                                    let mut decision = "idle".to_string();
                                    if let Some(entry) =
                                        signal_entries.iter().find(|e| e.asset_code == *asset)
                                    {
                                        let call_codes: Vec<&str> = entry
                                            .call_signal
                                            .split(',')
                                            .map(|s| s.trim())
                                            .collect();
                                        let put_codes: Vec<&str> =
                                            entry.put_signal.split(',').map(|s| s.trim()).collect();
                                        if call_codes.contains(&final_state.status_code.as_str()) {
                                            decision = "call".to_string();
                                        } else if put_codes
                                            .contains(&final_state.status_code.as_str())
                                        {
                                            decision = "put".to_string();
                                        }
                                    }

                                    historical_results.push(CompactAnalysis {
                                        time,
                                        action: decision,
                                        status_code: final_state.status_code,
                                    });
                                    count += 1;
                                }

                                if let Some(ref last) = gen.state.last_analysis {
                                    println!(
                                        "  ✅ {} loaded {} candles | StatusCode={} StatusDesc={}",
                                        asset, count, last.status_code, last.status_desc
                                    );
                                } else {
                                    println!(
                                        "  ✅ {} loaded {} candles (no analysis yet)",
                                        asset, count
                                    );
                                }

                                // Send history to frontend
                                if historical_results.len() > 1000 {
                                    let skip_amt = historical_results.len() - 1000;
                                    historical_results =
                                        historical_results.into_iter().skip(skip_amt).collect();
                                }
                                let hist_msg = HistoricalAnalysis {
                                    msg_type: "historical_analysis".to_string(),
                                    symbol: asset.clone(),
                                    results: historical_results,
                                };
                                let _ = tx.send(BroadcastMessage::HistoricalAnalysis(hist_msg));

                                generators.insert(asset.clone(), gen);
                                break; // Success, break while loop for this asset
                            }
                        }
                    } else {
                        break; // Timeout or stream closed
                    }
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
            }

            println!(
                "📊 AutoTrade: All {} generators ready. Subscribing to live candles...",
                generators.len()
            );

            // Broadcast initial status
            let _ = tx.send(BroadcastMessage::AutoTradeStatus(AutoTradeStatusMessage {
                msg_type: "auto_trade_status".to_string(),
                active: true,
                entries: vec![],
                grand_profit: 0.0,
                trade_count: 0,
                message: format!("Auto-trade started with {} assets", generators.len()),
            }));

            // 4. Subscribe to live candles for ALL assets
            let mut sub_ids: Vec<String> = Vec::new();
            for asset in &asset_symbols {
                let sub_msg = serde_json::json!({
                    "ticks_history": asset,
                    "subscribe": 1,
                    "style": "candles",
                    "granularity": 60,
                    "count": 1,
                    "end": "latest",
                    "adjust_start_time": 1
                });
                let _ = write
                    .send(TungsteniteMessage::Text(sub_msg.to_string()))
                    .await;
                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
            }

            // Trading state
            let mut last_check_minute: Option<u64> = None;
            let mut balance: f64 = 1000.0;
            let mut grand_profit: f64 = 0.0;
            let mut win_count: u32 = 0;
            let mut trade_count: u32 = 0;
            let mut lot_active = true;
            let martingale_stakes = vec![1.0, 2.0, 6.0, 18.0, 54.0, 162.0, 384.0, 800.0, 1600.0];
            let mut stake_index_per_asset: std::collections::HashMap<String, usize> =
                std::collections::HashMap::new();
            let mut pending_contracts: std::collections::HashMap<String, String> =
                std::collections::HashMap::new(); // contract_id -> asset

            // === NEW DAY TRADE LOGGING STATE ===
            let mut day_trade_entries: Vec<DayTradeEntry> = Vec::new();
            let mut first_trade_time: Option<String> = None;
            let current_date = Local::now().format("%Y-%m-%d").to_string();

            let mut current_duration = if config.duration == 0 {
                55
            } else {
                config.duration
            };
            let mut current_duration_unit = if config.duration_unit.is_empty() {
                "s".to_string()
            } else {
                config.duration_unit.clone()
            };
            let mut current_initial_stake = if config.initial_stake > 0.0 {
                config.initial_stake
            } else {
                1.0
            };
            let mut target_profit = if config.target_profit > 0.0 {
                config.target_profit
            } else {
                10.0
            };
            let mut target_win = if config.target_win > 0 {
                config.target_win
            } else {
                5
            };
            let mut current_money_mode = config.money_mode.clone();

            // Lot logging
            let folder_name = get_daily_folder_name();
            let folder_path = ensure_daily_folder(&folder_name);
            let lot_no = get_next_lot_no(&folder_path);
            let mut trades_for_lot: Vec<TradeObject> = Vec::new();

            println!("🤖 AutoTrade: Entering main trading loop (Lot #{})", lot_no);
            let mut initial_check_done = false; // Force first signal check immediately

            // 5. Main event loop — browser independent!
            loop {
                tokio::select! {
                    cmd = cmd_rx.recv() => {
                        if let Some(cmd) = cmd {
                            if cmd == "STOP" {
                                println!("🛑 AutoTrade: Stop command received");
                                // Unsubscribe all
                                for id in &sub_ids {
                                    let forget_msg = serde_json::json!({"forget": id});
                                    let _ = write.send(TungsteniteMessage::Text(forget_msg.to_string())).await;
                                }
                                let _ = tx.send(BroadcastMessage::AutoTradeStatus(AutoTradeStatusMessage {
                                    msg_type: "auto_trade_status".to_string(),
                                    active: false,
                                    entries: vec![],
                                    grand_profit,
                                    trade_count,
                                    message: "Auto-trade stopped by user".to_string(),
                                }));
                                break;
                            } else if cmd == "SYNC" {
                                let _ = tx.send(BroadcastMessage::AutoTradeStatus(AutoTradeStatusMessage {
                                    msg_type: "auto_trade_status".to_string(),
                                    active: lot_active,
                                    entries: vec![],
                                    grand_profit,
                                    trade_count,
                                    message: format!("Sync: Auto-trade is {} with P/L: ${:.2}", if lot_active { "Active" } else { "Stopped" }, grand_profit),
                                }));
                                // Send current balance to browser
                                let _ = tx.send(BroadcastMessage::Balance(BalanceMessage {
                                    msg_type: "balance".to_string(),
                                    balance,
                                }));
                                let _ = tx.send(BroadcastMessage::LotStatus(LotStatus {
                                    msg_type: "lot_status".to_string(),
                                    grand_profit,
                                    win_count,
                                    target_profit,
                                    target_win,
                                    lot_active,
                                    balance,
                                }));

                                // Re-broadcast historical_analysis for ALL assets so browser gets markers
                                for (asset_sym, gen) in &generators {
                                    // Rebuild CompactAnalysis from generator's analysis_array
                                    let history_results: Vec<CompactAnalysis> = gen.analysis_array.iter().map(|res| {
                                        let mut decision = "idle".to_string();
                                        if let Some(entry) = signal_entries.iter().find(|e| e.asset_code == *asset_sym) {
                                            let call_codes: Vec<&str> = entry.call_signal.split(',').map(|s| s.trim()).collect();
                                            let put_codes: Vec<&str> = entry.put_signal.split(',').map(|s| s.trim()).collect();
                                            if call_codes.contains(&res.status_code.as_str()) {
                                                decision = "call".to_string();
                                            } else if put_codes.contains(&res.status_code.as_str()) {
                                                decision = "put".to_string();
                                            }
                                        }
                                        CompactAnalysis {
                                            time: res.candletime,
                                            action: decision,
                                            status_code: res.status_code.clone(),
                                        }
                                    }).collect();

                                    if !history_results.is_empty() {
                                        let hist_msg = HistoricalAnalysis {
                                            msg_type: "historical_analysis".to_string(),
                                            symbol: asset_sym.clone(),
                                            results: history_results,
                                        };
                                        let _ = tx.send(BroadcastMessage::HistoricalAnalysis(hist_msg));
                                    }
                                }
                                println!("📡 SYNC: Re-broadcast historical_analysis for {} assets", generators.len());
                            } else if cmd.starts_with("SELL:") {
                                // Handle SELL command forwarded from handle_socket
                                let contract_id = cmd.trim_start_matches("SELL:").to_string();
                                println!("🔻 AutoTrade: Selling contract {}", contract_id);
                                let sell_msg = serde_json::json!({
                                    "sell": contract_id,
                                    "price": 0
                                });
                                let _ = write.send(TungsteniteMessage::Text(sell_msg.to_string())).await;
                            } else if let Ok(json_cmd) = serde_json::from_str::<serde_json::Value>(&cmd) {
                                if let Some(command) = json_cmd.get("command").and_then(|c| c.as_str()) {
                                    if command == "UPDATE_PARAMS" {
                                        if let Some(tp) = json_cmd.get("target_profit").and_then(|v| v.as_f64()) { target_profit = tp; }
                                        if let Some(tw) = json_cmd.get("target_win").and_then(|v| v.as_u64()) { target_win = tw as u32; }
                                        if let Some(stake) = json_cmd.get("initial_stake").and_then(|v| v.as_f64()) { current_initial_stake = stake; }
                                        if let Some(dur) = json_cmd.get("duration").and_then(|v| v.as_u64()) { current_duration = dur; }
                                        if let Some(mm) = json_cmd.get("money_mode").and_then(|v| v.as_str()) { current_money_mode = mm.to_string(); }
                                        if let Some(du) = json_cmd.get("duration_unit").and_then(|v| v.as_str()) { current_duration_unit = du.to_string(); }

                                        println!("🔄 AutoTrade: Settings updated -> Target: ${}, Win: {}, Stake: ${}, Mode: {}, Dur: {}{}",
                                            target_profit, target_win, current_initial_stake, current_money_mode, current_duration, current_duration_unit);

                                        // Broadcast updated lot status to browser
                                        let _ = tx.send(BroadcastMessage::LotStatus(LotStatus {
                                            msg_type: "lot_status".to_string(),
                                            grand_profit,
                                            win_count,
                                            target_profit,
                                            target_win,
                                            lot_active,
                                            balance,
                                        }));
                                    } else if command == "UPDATE_MODE" {
                                        if let Some(tm) = json_cmd.get("trade_mode").and_then(|v| v.as_str()) {
                                            if tm == "idle" {
                                                lot_active = false;
                                                println!("⏸️ AutoTrade: Set to IDLE (Paused).");
                                            } else if tm == "auto" {
                                                lot_active = true;
                                                println!("▶️ AutoTrade: Set to AUTO (Resumed).");
                                            }
                                            // Broadcast mode change to browser
                                            let _ = tx.send(BroadcastMessage::LotStatus(LotStatus {
                                                msg_type: "lot_status".to_string(),
                                                grand_profit,
                                                win_count,
                                                target_profit,
                                                target_win,
                                                lot_active,
                                                balance,
                                            }));
                                        }
                                    }
                                }
                            }
                        }
                    }

                    msg = read.next() => {
                        if let Some(Ok(TungsteniteMessage::Text(raw_text))) = msg {
                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&raw_text) {

                                // Track subscription IDs
                                if let Some(sub) = json.get("subscription") {
                                    if let Some(id) = sub.get("id").and_then(|i| i.as_str()) {
                                        if !sub_ids.contains(&id.to_string()) {
                                            sub_ids.push(id.to_string());
                                        }
                                    }
                                }

                                // Get balance from authorize
                                if let Some(authorize) = json.get("authorize") {
                                    if let Some(bal) = authorize.get("balance").and_then(|b| b.as_f64()) {
                                        balance = bal;
                                        let balance_msg = BalanceMessage {
                                            msg_type: "balance".to_string(),
                                            balance,
                                        };
                                        let _ = tx.send(BroadcastMessage::Balance(balance_msg));
                                        println!("💰 AutoTrade: Balance = {}", balance);
                                    }
                                }

                                // Handle OHLC updates
                                if let Some(ohlc) = json.get("ohlc") {
                                    let symbol = ohlc.get("symbol").and_then(|s| s.as_str()).unwrap_or("").to_string();
                                    let epoch = ohlc.get("epoch").and_then(|v| v.as_u64())
                                        .or_else(|| ohlc.get("epoch").and_then(|v| v.as_str()?.parse().ok()))
                                        .unwrap_or(0);
                                    let open_time = ohlc.get("open_time").and_then(|v| v.as_u64())
                                        .or_else(|| ohlc.get("open_time").and_then(|v| v.as_str()?.parse().ok()))
                                        .unwrap_or(0);
                                    let o = ohlc.get("open").and_then(|v| v.as_f64())
                                        .or_else(|| ohlc.get("open").and_then(|v| v.as_str()?.parse().ok())).unwrap_or(0.0);
                                    let h = ohlc.get("high").and_then(|v| v.as_f64())
                                        .or_else(|| ohlc.get("high").and_then(|v| v.as_str()?.parse().ok())).unwrap_or(0.0);
                                    let l = ohlc.get("low").and_then(|v| v.as_f64())
                                        .or_else(|| ohlc.get("low").and_then(|v| v.as_str()?.parse().ok())).unwrap_or(0.0);
                                    let c = ohlc.get("close").and_then(|v| v.as_f64())
                                        .or_else(|| ohlc.get("close").and_then(|v| v.as_str()?.parse().ok())).unwrap_or(0.0);

                                    if !symbol.is_empty() && open_time > 0 {
                                        // Feed completed candle to generator when new candle starts
                                        let prev_open_time = current_candle.get(&symbol).map(|cc| cc.0).unwrap_or(0);

                                        if open_time != prev_open_time && prev_open_time > 0 {
                                            if let Some((pt, po, ph, pl, pc)) = current_candle.get(&symbol) {
                                                if let Some(gen) = generators.get_mut(&symbol) {
                                                    let completed = V2Candle {
                                                        time: *pt, open: *po, high: *ph,
                                                        low: *pl, close: *pc,
                                                    };
                                                    let result = gen.append_candle(completed);
                                                    println!(
                                                        "  📊 AutoTrade {} candle closed | StatusCode={} Desc={}",
                                                        symbol, result.status_code, result.status_desc
                                                    );
                                                }
                                            }
                                        }

                                        current_candle.insert(symbol.clone(), (open_time, o, h, l, c));

                                        // === CHECK AT SECOND 0-5 OF MINUTE (or first check immediately) ===
                                        let current_minute = epoch / 60;
                                        let seconds = epoch % 60;

                                        // Force first check immediately regardless of seconds,
                                        // then check at seconds 0-5 for subsequent minutes
                                        let should_check = if !initial_check_done {
                                            initial_check_done = true;
                                            true
                                        } else {
                                            seconds <= 5 && Some(current_minute) != last_check_minute
                                        };

                                        if should_check && Some(current_minute) != last_check_minute && lot_active {
                                            last_check_minute = Some(current_minute);

                                            // Signal check for ALL selected assets
                                            let mut trade_entries: Vec<AutoTradeEntry> = Vec::new();

                                            for entry in &signal_entries {
                                                if entry.is_active != "y" { continue; }
                                                if !asset_symbols.contains(&entry.asset_code) { continue; }

                                                let asset_code = &entry.asset_code;
                                                if let Some(gen) = generators.get(asset_code) {
                                                    if let Some(ref analysis) = gen.state.last_analysis {
                                                        let code_str = &analysis.status_code;

                                                        // Parse signal codes from tradeSignal.json
                                                        let call_codes: Vec<&str> = entry.call_signal.split(',').map(|s| s.trim()).collect();
                                                        let put_codes: Vec<&str> = entry.put_signal.split(',').map(|s| s.trim()).collect();

                                                        let (decision, _reason) = if call_codes.contains(&code_str.as_str()) {
                                                            ("CALL".to_string(), format!("StatusCode {} matched CallSignal", code_str))
                                                        } else if put_codes.contains(&code_str.as_str()) {
                                                            ("PUT".to_string(), format!("StatusCode {} matched PutSignal", code_str))
                                                        } else {
                                                            ("IDLE".to_string(), format!("StatusCode {} — no match", code_str))
                                                        };

                                                        println!("  📊 AutoTrade {} | Code={} | Desc={} | Decision={}",
                                                            asset_code, code_str, analysis.status_desc, decision);

                                                        // Execute trade if CALL or PUT
                                                        if decision == "CALL" || decision == "PUT" {
                                                            let stake_idx = stake_index_per_asset.get(asset_code).copied().unwrap_or(0);
                                                            let stake = if current_money_mode == "martingale" {
                                                                martingale_stakes[stake_idx.min(martingale_stakes.len() - 1)]
                                                            } else {
                                                                current_initial_stake
                                                            };

                                                            if balance >= stake {
                                                                let buy_msg = serde_json::json!({
                                                                    "buy": "1",
                                                                    "price": stake,
                                                                    "parameters": {
                                                                        "contract_type": decision,
                                                                        "symbol": asset_code,
                                                                        "duration": current_duration,
                                                                        "duration_unit": current_duration_unit,
                                                                        "basis": "stake",
                                                                        "amount": stake,
                                                                        "currency": "USD"
                                                                    }
                                                                });

                                                                println!("📈 AutoTrade: Placing {} on {} with stake ${}", decision, asset_code, stake);
                                                                let _ = write.send(TungsteniteMessage::Text(buy_msg.to_string())).await;

                                                                trade_entries.push(AutoTradeEntry {
                                                                    asset: asset_code.clone(),
                                                                    direction: decision.clone(),
                                                                    status_code: code_str.clone(),
                                                                    stake,
                                                                    timestamp: Local::now().format("%H:%M:%S").to_string(),
                                                                });

                                                                // Small delay between multiple buy orders
                                                                tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
                                                            } else {
                                                                println!("⚠️ AutoTrade: Insufficient balance for {} (need {}, have {})", asset_code, stake, balance);
                                                            }
                                                        }
                                                    }
                                                }
                                            }

                                            // Broadcast trade entries to browser (if connected)
                                            if !trade_entries.is_empty() {
                                                println!("🔥 AutoTrade: {} trades placed this minute", trade_entries.len());
                                                let _ = tx.send(BroadcastMessage::AutoTradeStatus(AutoTradeStatusMessage {
                                                    msg_type: "auto_trade_status".to_string(),
                                                    active: true,
                                                    entries: trade_entries,
                                                    grand_profit,
                                                    trade_count,
                                                    message: format!("Trades placed at minute {}", current_minute),
                                                }));
                                            }

                                            // ALWAYS broadcast multi_analysis every minute for chart markers & signal strip
                                            {
                                                let mut signal_results: Vec<AssetSignalResult> = Vec::new();
                                                for entry in &signal_entries {
                                                    if entry.is_active != "y" { continue; }
                                                    if !asset_symbols.contains(&entry.asset_code) { continue; }
                                                    if let Some(gen) = generators.get(&entry.asset_code) {
                                                        if let Some(ref analysis) = gen.state.last_analysis {
                                                            let code_str = &analysis.status_code;
                                                            let call_codes: Vec<&str> = entry.call_signal.split(',').map(|s| s.trim()).collect();
                                                            let put_codes: Vec<&str> = entry.put_signal.split(',').map(|s| s.trim()).collect();
                                                            let decision = if call_codes.contains(&code_str.as_str()) {
                                                                "call"
                                                            } else if put_codes.contains(&code_str.as_str()) {
                                                                "put"
                                                            } else {
                                                                "idle"
                                                            };
                                                            signal_results.push(AssetSignalResult {
                                                                asset: entry.asset_code.clone(),
                                                                status_code: code_str.clone(),
                                                                status_desc: analysis.status_desc.clone(),
                                                                decision: decision.to_string(),
                                                                reason: format!("AutoTrade StatusCode {}", code_str),
                                                                close_price: analysis.close,
                                                                ema_short_dir: analysis.ema_short_direction.clone(),
                                                                ema_medium_dir: analysis.ema_medium_direction.clone(),
                                                                ema_long_dir: analysis.ema_long_direction.clone(),
                                                            });
                                                        }
                                                    }
                                                }
                                                if !signal_results.is_empty() {
                                                    let _ = tx.send(BroadcastMessage::MultiAnalysis(MultiAnalysisMessage {
                                                        msg_type: "multi_analysis".to_string(),
                                                        timestamp: epoch,
                                                        assets: signal_results,
                                                    }));
                                                }
                                            }
                                        }
                                    }
                                }

                                // Handle buy response
                                if let Some(buy) = json.get("buy") {
                                    let contract_id = buy.get("contract_id")
                                        .and_then(|c| c.as_str().map(|s| s.to_string()))
                                        .or_else(|| buy.get("contract_id").and_then(|c| c.as_u64().map(|n| n.to_string())));

                                    if let Some(cid) = contract_id {
                                        // Find asset from echo_req parameters
                                        let asset_for_contract = json.get("echo_req")
                                            .and_then(|e| e.get("parameters"))
                                            .and_then(|p| p.get("symbol"))
                                            .and_then(|s| s.as_str())
                                            .unwrap_or("")
                                            .to_string();

                                        pending_contracts.insert(cid.clone(), asset_for_contract.clone());
                                        trade_count += 1;

                                        let stake = buy.get("buy_price").and_then(|p| p.as_f64()).unwrap_or(0.0);
                                        println!("✅ AutoTrade: Contract {} opened for {} (stake: ${})", cid, asset_for_contract, stake);

                                        // Broadcast trade_opened
                                        let trade_opened_time = Local::now().format("%H:%M:%S").to_string();
                                        if first_trade_time.is_none() {
                                            first_trade_time = Some(trade_opened_time.clone());
                                        }

                                        let trade_opened = TradeOpened {
                                            msg_type: "trade_opened".to_string(),
                                            contract_id: cid.clone(),
                                            asset: asset_for_contract.clone(),
                                            trade_type: "AUTO".to_string(),
                                            stake,
                                            time: trade_opened_time,
                                        };
                                        let _ = tx.send(BroadcastMessage::TradeOpened(trade_opened));

                                        // Subscribe to contract for result
                                        let proposal_msg = serde_json::json!({
                                            "proposal_open_contract": 1,
                                            "contract_id": cid,
                                            "subscribe": 1
                                        });
                                        let _ = write.send(TungsteniteMessage::Text(proposal_msg.to_string())).await;
                                    }
                                }

                                // Handle contract result
                                if let Some(proposal) = json.get("proposal_open_contract") {
                                    let contract_id = proposal.get("contract_id")
                                        .and_then(|c| c.as_str().map(|s| s.to_string()))
                                        .or_else(|| proposal.get("contract_id").and_then(|c| c.as_u64().map(|n| n.to_string())))
                                        .unwrap_or_default();
                                    let status = proposal.get("status").and_then(|s| s.as_str()).unwrap_or("open");
                                    let is_sold = proposal.get("is_sold").and_then(|v| v.as_u64()).unwrap_or(0) == 1;
                                    let is_expired = proposal.get("is_expired").and_then(|v| v.as_u64()).unwrap_or(0) == 1;
                                    let asset = proposal.get("underlying").and_then(|s| s.as_str()).unwrap_or("").to_string();
                                    let trade_type = proposal.get("contract_type").and_then(|s| s.as_str()).unwrap_or("").to_string();

                                    // Send real-time updates while contract is open
                                    if status == "open" {
                                        let current_spot = proposal.get("current_spot").and_then(|v| v.as_f64())
                                            .or_else(|| proposal.get("current_spot").and_then(|v| v.as_str()?.parse().ok()))
                                            .unwrap_or(0.0);
                                        let entry_spot = proposal.get("entry_spot").and_then(|v| v.as_f64())
                                            .or_else(|| proposal.get("entry_spot").and_then(|v| v.as_str()?.parse().ok()))
                                            .unwrap_or(0.0);
                                        let profit = proposal.get("profit").and_then(|p| p.as_f64()).unwrap_or(0.0);
                                        let profit_percentage = proposal.get("profit_percentage").and_then(|p| p.as_f64()).unwrap_or(0.0);
                                        let payout = proposal.get("payout").and_then(|p| p.as_f64()).unwrap_or(0.0);
                                        let buy_price = proposal.get("buy_price").and_then(|p| p.as_f64()).unwrap_or(0.0);
                                        let date_expiry = proposal.get("date_expiry").and_then(|d| d.as_u64()).unwrap_or(0);
                                        let date_start = proposal.get("date_start").and_then(|d| d.as_u64()).unwrap_or(0);

                                        let trade_update = TradeUpdate {
                                            msg_type: "trade_update".to_string(),
                                            contract_id: contract_id.clone(),
                                            asset: asset.clone(),
                                            trade_type: trade_type.clone(),
                                            current_spot,
                                            entry_spot,
                                            profit,
                                            profit_percentage,
                                            is_sold,
                                            is_expired,
                                            payout,
                                            buy_price,
                                            date_expiry,
                                            date_start,
                                        };
                                        let _ = tx.send(BroadcastMessage::TradeUpdate(trade_update));
                                    }

                                    if status == "sold" || status == "won" || status == "lost" {
                                        let profit = proposal.get("profit").and_then(|p| p.as_f64()).unwrap_or(0.0);
                                        let stake = proposal.get("buy_price").and_then(|p| p.as_f64()).unwrap_or(0.0);
                                        let trade_type = proposal.get("contract_type").and_then(|s| s.as_str()).unwrap_or("").to_string();

                                        balance += profit;
                                        grand_profit += profit;
                                        let is_win = profit > 0.0;
                                        if is_win { win_count += 1; }

                                        // Get asset for this contract
                                        let asset_for_contract = pending_contracts.remove(&contract_id).unwrap_or_default();

                                        // Update martingale state per asset
                                        if is_win {
                                            stake_index_per_asset.insert(asset_for_contract.clone(), 0);
                                        } else if current_money_mode == "martingale" {
                                            let idx = stake_index_per_asset.get(&asset_for_contract).copied().unwrap_or(0);
                                            stake_index_per_asset.insert(asset_for_contract.clone(), (idx + 1).min(martingale_stakes.len() - 1));
                                        }

                                        let icon = if is_win { "🎉" } else { "❌" };
                                        println!("{} AutoTrade: {} {} | Profit: ${:.2} | Balance: ${:.2} | Grand: ${:.2} | Wins: {}",
                                            icon, asset_for_contract, if is_win { "WIN" } else { "LOSS" }, profit, balance, grand_profit, win_count);

                                        // Broadcast result
                                        let _ = tx.send(BroadcastMessage::TradeResult(TradeResult {
                                            msg_type: "trade_result".to_string(),
                                            status: if is_win { "win".to_string() } else { "loss".to_string() },
                                            balance,
                                            stake,
                                            profit,
                                            contract_id: Some(contract_id.clone()),
                                        }));

                                        // Save lot log
                                        let trade_no = trades_for_lot.len() as u32 + 1;
                                        trades_for_lot.push(TradeObject {
                                            lot_no,
                                            trade_no_on_this_lot: trade_no,
                                            trade_time: Local::now().format("%d-%m-%Y %H:%M:%S").to_string(),
                                            asset: asset_for_contract.clone(),
                                            action: trade_type.to_lowercase(),
                                            money_trade: stake,
                                            money_trade_type: if current_money_mode == "fix" { "Fixed".to_string() } else { "Martingale".to_string() },
                                            win_status: if is_win { "win".to_string() } else { "loss".to_string() },
                                            profit,
                                            balance_on_lot: grand_profit,
                                            win_con: target_win.to_string(),
                                            loss_con: target_profit.to_string(),
                                            is_stop_trade: false,
                                        });

                                        let lot_log = LotLog {
                                            lot_no,
                                            trade_object_list: trades_for_lot.clone(),
                                        };
                                        save_lot_log(&ensure_daily_folder(&get_daily_folder_name()), &lot_log);

                                        // Save to Firestore
                                        let date_start_val = proposal.get("date_start").and_then(|d| d.as_u64()).unwrap_or(0);
                                        let date_expiry_val = proposal.get("date_expiry").and_then(|d| d.as_u64()).unwrap_or(0);
                                        let payout_val = proposal.get("payout").and_then(|p| p.as_f64()).unwrap_or(0.0);
                                        let entry_spot_val = proposal.get("entry_spot").and_then(|v| v.as_f64())
                                            .or_else(|| proposal.get("entry_spot").and_then(|v| v.as_str()?.parse().ok()))
                                            .unwrap_or(0.0);
                                            let _exit_spot_val = proposal.get("exit_spot").and_then(|v| v.as_f64())
                                                .or_else(|| proposal.get("exit_spot").and_then(|v| v.as_str()?.parse().ok()))
                                                .unwrap_or(0.0);

                                            let trade_record = TradeRecord {
                                                order_no: trade_no,
                                                contract_id: contract_id.clone(),
                                                symbol: asset_for_contract.clone(),
                                                trade_type: trade_type.clone(),
                                                buy_price: stake,
                                                payout: payout_val,
                                                profit_loss: profit,
                                                buy_time: date_start_val,
                                                expiry_time: date_expiry_val,
                                                time_remaining: 0,
                                                min_profit: profit,
                                                max_profit: profit,
                                                status: if is_win { "win".to_string() } else { "loss".to_string() },
                                                entry_spot: entry_spot_val,
                                                exit_spot: _exit_spot_val,
                                                lot_no,
                                                trade_no_in_lot: trade_no,
                                                trade_date: Local::now().format("%Y-%m-%d").to_string(),
                                                created_at: Local::now().format("%Y-%m-%dT%H:%M:%S").to_string(),
                                            };

                                            // === UPDATE DAY TRADE HISTORY LOGGING ===
                                            let action_str = if is_win { "WIN ✅".to_string() } else { "LOSS ❌".to_string() };
                                            let _status_code_val = entry_spot_val; // For now we keep it simple or zero, ideally it should come from state

                                        // fetch close code from generator state
                                        let mut code_str = String::from("-");
                                        if let Some(gen) = generators.get(&asset_for_contract) {
                                            if let Some(ref analysis) = gen.state.last_analysis {
                                                code_str = analysis.status_code.clone();
                                            }
                                        }

                                        day_trade_entries.push(DayTradeEntry {
                                            no: trade_no,
                                            contract_id: contract_id.clone(),
                                            symbol: asset_for_contract.clone(),
                                            status_code: code_str,
                                            trade_type: trade_type.clone(),
                                            buy_price: stake,
                                            payout: payout_val,
                                            buy_time: date_start_val.to_string(),
                                            expiry: date_expiry_val.to_string(),
                                            remaining: "00:00".to_string(),
                                            min_profit: -stake,
                                            max_profit: payout_val - stake,
                                            profit,
                                            action: action_str,
                                        });

                                        let status_of_trade = if lot_active { "กำลังเทรดอยู่".to_string() } else { "สิ้นสุดการเทรด".to_string() };

                                        let day_trade_wrapper = DayTradeWrapper {
                                            day_trade: DayTradeData {
                                                lot_no_current: lot_no,
                                                day_trade: current_date.clone(),
                                                start_trade_of_day: first_trade_time.clone().unwrap_or_default(),
                                                last_trade_of_day: Local::now().format("%H:%M:%S").to_string(),
                                                total_trade_on_this_day: day_trade_entries.len() as u32,
                                                total_profit: grand_profit,
                                                status_of_trade,
                                                current_profit: profit,
                                                day_trade_list: day_trade_entries.clone(),
                                            }
                                        };
                                        save_day_trade_log(&day_trade_wrapper);
                                        // ========================================

                                        let fs = firestore.lock().await;
                                        match fs.save_trade(&trade_record).await {
                                            Ok(doc_id) => println!("🔥 AutoTrade: Trade saved to Firestore: {}", doc_id),
                                            Err(e) => println!("⚠️ AutoTrade: Firestore save error: {}", e),
                                        }

                                        // Broadcast lot status
                                        let _ = tx.send(BroadcastMessage::LotStatus(LotStatus {
                                            msg_type: "lot_status".to_string(),
                                            grand_profit,
                                            win_count,
                                            target_profit,
                                            target_win,
                                            lot_active,
                                            balance,
                                        }));

                                        // Check stop conditions
                                        let mut stop_trading = false;
                                        if current_money_mode == "fix" {
                                            if grand_profit >= target_profit {
                                                stop_trading = true;
                                                println!("🏆 AutoTrade: TARGET PROFIT REACHED! ${:.2} >= ${:.2}", grand_profit, target_profit);
                                            }
                                        } else if current_money_mode == "martingale" {
                                            if win_count >= target_win {
                                                stop_trading = true;
                                                println!("🏆 AutoTrade: TARGET WIN COUNT REACHED! {} >= {}", win_count, target_win);
                                            }
                                        }

                                        if stop_trading {
                                            let _lot_active = false;
                                            println!("🛑 AutoTrade: STOPPING — conditions met!");

                                            // Unsubscribe
                                            for id in &sub_ids {
                                                let forget_msg = serde_json::json!({"forget": id});
                                                let _ = write.send(TungsteniteMessage::Text(forget_msg.to_string())).await;
                                            }

                                            let _ = tx.send(BroadcastMessage::AutoTradeStatus(AutoTradeStatusMessage {
                                                msg_type: "auto_trade_status".to_string(),
                                                active: false,
                                                entries: vec![],
                                                grand_profit,
                                                trade_count,
                                                message: format!("Auto-trade completed! P/L: ${:.2}, Wins: {}", grand_profit, win_count),
                                            }));
                                            break;
                                        }
                                    }
                                }

                                // Handle errors
                                if let Some(error) = json.get("error") {
                                    println!("❌ AutoTrade API Error: {}",
                                        error.get("message").unwrap_or(&serde_json::json!("Unknown")));
                                }
                            }
                        } else {
                            // WebSocket disconnected from Deriv — try reconnect
                            println!("⚠️ AutoTrade: Deriv WebSocket disconnected");
                            break;
                        }
                    }
                }
            }

            println!(
                "🤖 AutoTrade: Session ended. Grand P/L: ${:.2}, Trades: {}, Wins: {}",
                grand_profit, trade_count, win_count
            );
        }
        Err(e) => println!("❌ AutoTrade: Connection Failed: {}", e),
    }
}

fn parse_flexible(data: &serde_json::Value) -> Result<Candle, ()> {
    let to_f64 = |v: Option<&serde_json::Value>| -> Option<f64> {
        let val = v?;
        val.as_f64().or_else(|| val.as_str()?.parse().ok())
    };

    let to_u64 = |v: Option<&serde_json::Value>| -> Option<u64> {
        let val = v?;
        val.as_u64().or_else(|| val.as_str()?.parse().ok())
    };

    let epoch = data.get("epoch").and_then(|v| v.as_u64()).ok_or(())?;
    let open_time = to_u64(data.get("open_time")).unwrap_or(epoch / 60 * 60);

    Ok(Candle {
        symbol: String::new(),
        time: epoch,
        open_time,
        open: to_f64(data.get("open")).ok_or(())?,
        high: to_f64(data.get("high")).ok_or(())?,
        low: to_f64(data.get("low")).ok_or(())?,
        close: to_f64(data.get("close")).ok_or(())?,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveTickHistoryPayload {
    pub folder_name: String,
    pub filename: String,
    pub data: String,
}

async fn save_tick_history_handler(
    axum::Json(payload): axum::Json<SaveTickHistoryPayload>,
) -> Response {
    let path_str = format!("tickhistory/{}", payload.folder_name);
    let path = Path::new(&path_str);

    if let Err(e) = std::fs::create_dir_all(path) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to create dir: {}", e),
        )
            .into_response();
    }

    let file_path = path.join(format!("{}.json", payload.filename));
    match std::fs::write(&file_path, payload.data) {
        Ok(_) => Response::builder()
            .status(200)
            .body("Saved".into())
            .unwrap(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to write: {}", e),
        )
            .into_response(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanAssetData {
    pub symbol: String,
    pub price: f64,
    pub ci: f64,
    pub adx: f64,
    pub score: f64,
    pub is_bullish: bool,
    pub recent_candles: String,
    #[serde(default)]
    pub rank: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveScanPayload {
    pub scan_time: String,
    pub timeframe: String,
    pub period: String,
    pub assets: Vec<ScanAssetData>,
}

async fn save_scan_handler(
    State(state): State<Arc<AppState>>,
    axum::Json(payload): axum::Json<SaveScanPayload>,
) -> Response {
    println!(
        "📊 Received scan data: {} assets at {}",
        payload.assets.len(),
        payload.scan_time
    );

    let firestore = state.firestore.lock().await;
    let mut saved_count = 0;
    let mut errors = Vec::new();

    for asset in &payload.assets {
        let record = ScanRecord {
            scan_time: payload.scan_time.clone(),
            timeframe: payload.timeframe.clone(),
            period: payload.period.clone(),
            symbol: asset.symbol.clone(),
            price: asset.price,
            ci: asset.ci,
            adx: asset.adx,
            score: asset.score,
            is_bullish: asset.is_bullish,
            recent_candles: asset.recent_candles.clone(),
            rank: asset.rank.unwrap_or(0),
        };

        match firestore.save_scan(&record).await {
            Ok(_) => saved_count += 1,
            Err(e) => errors.push(format!("{}: {}", asset.symbol, e)),
        }
    }

    if errors.is_empty() {
        Response::builder()
            .status(200)
            .header("Content-Type", "application/json")
            .body(format!("{{\"success\": true, \"saved\": {}}}", saved_count).into())
            .unwrap()
    } else {
        Response::builder()
            .status(207)
            .header("Content-Type", "application/json")
            .body(
                format!(
                    "{{\"success\": true, \"saved\": {}, \"errors\": {:?}}}",
                    saved_count, errors
                )
                .into(),
            )
            .unwrap()
    }
}

// ==================== Scanner API Handlers ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScannerStartPayload {
    pub interval_seconds: u64,
    pub candle_timeframe: u64,
    pub indicator_period: usize,
    pub stop_time: Option<String>,
    #[serde(default = "default_true")]
    pub save_to_firestore: bool,
    pub assets: Vec<AssetConfigPayload>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetConfigPayload {
    pub symbol: String,
    pub name: String,
}

async fn scanner_start_handler(
    State(state): State<Arc<AppState>>,
    axum::Json(payload): axum::Json<ScannerStartPayload>,
) -> Response {
    println!(
        "📊 Scanner start request: {} assets, interval {}s, save_db: {}",
        payload.assets.len(),
        payload.interval_seconds,
        payload.save_to_firestore
    );

    let config = ScanConfig {
        interval_seconds: payload.interval_seconds,
        candle_timeframe: payload.candle_timeframe,
        indicator_period: payload.indicator_period,
        stop_time: payload.stop_time,
        save_to_firestore: payload.save_to_firestore,
        assets: payload
            .assets
            .into_iter()
            .map(|a| AssetConfig {
                symbol: a.symbol,
                name: a.name,
            })
            .collect(),
    };

    let scanner_lock = state.scanner.read().await;
    if let Some(ref scanner) = *scanner_lock {
        match scanner.start(config).await {
            Ok(_) => Response::builder()
                .status(200)
                .header("Content-Type", "application/json")
                .body("{\"success\": true, \"message\": \"Scanner started\"}".into())
                .unwrap(),
            Err(e) => Response::builder()
                .status(400)
                .header("Content-Type", "application/json")
                .body(format!("{{\"success\": false, \"error\": \"{}\"}}", e).into())
                .unwrap(),
        }
    } else {
        Response::builder()
            .status(500)
            .header("Content-Type", "application/json")
            .body("{\"success\": false, \"error\": \"Scanner not initialized\"}".into())
            .unwrap()
    }
}

async fn scanner_stop_handler(State(state): State<Arc<AppState>>) -> Response {
    println!("📊 Scanner stop request");

    let scanner_lock = state.scanner.read().await;
    if let Some(ref scanner) = *scanner_lock {
        match scanner.stop().await {
            Ok(_) => Response::builder()
                .status(200)
                .header("Content-Type", "application/json")
                .body("{\"success\": true, \"message\": \"Scanner stopped\"}".into())
                .unwrap(),
            Err(e) => Response::builder()
                .status(400)
                .header("Content-Type", "application/json")
                .body(format!("{{\"success\": false, \"error\": \"{}\"}}", e).into())
                .unwrap(),
        }
    } else {
        Response::builder()
            .status(500)
            .header("Content-Type", "application/json")
            .body("{\"success\": false, \"error\": \"Scanner not initialized\"}".into())
            .unwrap()
    }
}

async fn scanner_status_handler(State(state): State<Arc<AppState>>) -> Response {
    let scanner_lock = state.scanner.read().await;
    if let Some(ref scanner) = *scanner_lock {
        let status = scanner.get_status().await;
        match serde_json::to_string(&status) {
            Ok(json) => Response::builder()
                .status(200)
                .header("Content-Type", "application/json")
                .body(json.into())
                .unwrap(),
            Err(e) => Response::builder()
                .status(500)
                .header("Content-Type", "application/json")
                .body(format!("{{\"error\": \"{}\"}}", e).into())
                .unwrap(),
        }
    } else {
        Response::builder()
            .status(500)
            .header("Content-Type", "application/json")
            .body("{\"error\": \"Scanner not initialized\"}".into())
            .unwrap()
    }
}

async fn system_resources_handler() -> Response {
    let mut sys = System::new_all();
    sys.refresh_all();
    let pid = Pid::from_u32(std::process::id());

    let memory_used_mb = if let Some(process) = sys.process(pid) {
        process.memory() / 1024 / 1024
    } else {
        0
    };

    let resources = SystemResources {
        memory_used_mb,
        total_memory_mb: sys.total_memory() / 1024 / 1024,
        cpu_usage: sys.global_cpu_usage(),
        uptime_secs: System::uptime(),
    };

    match serde_json::to_string(&resources) {
        Ok(json) => Response::builder()
            .status(200)
            .header("Content-Type", "application/json")
            .body(json.into())
            .unwrap(),
        Err(e) => Response::builder()
            .status(500)
            .header("Content-Type", "application/json")
            .body(format!("{{\"error\": \"{}\"}}", e).into())
            .unwrap(),
    }
}

// ==================== Trading Config API Handlers ====================

const TRADING_CONFIG_FILE: &str = "public/config.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingThreshold {
    pub asset: String,
    pub macd12: f64,
    pub macd23: f64,
    #[serde(rename = "slopeValue")]
    pub slope_value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingConfigPayload {
    pub username: String,
    #[serde(rename = "assetList")]
    pub asset_list: Vec<AssetItem>,
    #[serde(rename = "defaultAsset")]
    pub default_asset: String,
    #[serde(rename = "startMoneyTrade")]
    pub start_money_trade: f64,
    #[serde(rename = "moneyMartinGale")]
    pub money_martin_gale: Vec<f64>,
    #[serde(rename = "tradeTypes")]
    pub trade_types: Vec<String>,
    #[serde(rename = "selectedTradeType")]
    pub selected_trade_type: String,
    #[serde(rename = "targetMoney")]
    pub target_money: f64,
    // EMA Settings
    #[serde(rename = "emaShortType")]
    pub ema_short_type: String,
    #[serde(rename = "emaShortPeriod")]
    pub ema_short_period: usize,
    #[serde(rename = "emaMediumType")]
    pub ema_medium_type: String,
    #[serde(rename = "emaMediumPeriod")]
    pub ema_medium_period: usize,
    #[serde(rename = "emaLongType")]
    pub ema_long_type: String,
    #[serde(rename = "emaLongPeriod")]
    pub ema_long_period: usize,
    // Thresholds
    pub thresholds: Vec<TradingThreshold>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetItem {
    pub symbol: String,
    pub name: String,
}

fn load_trading_config() -> Option<TradingConfigPayload> {
    match fs::read_to_string(TRADING_CONFIG_FILE) {
        Ok(content) => match serde_json::from_str::<TradingConfigPayload>(&content) {
            Ok(config) => Some(config),
            Err(e) => {
                println!("⚠️ Trading config parse error: {}", e);
                None
            }
        },
        Err(_) => {
            println!("📁 No trading config file found, will create on first save");
            None
        }
    }
}

fn save_trading_config(config: &TradingConfigPayload) -> Result<(), String> {
    match serde_json::to_string_pretty(config) {
        Ok(json_str) => {
            if let Err(e) = fs::write(TRADING_CONFIG_FILE, json_str) {
                Err(format!("Failed to write config file: {}", e))
            } else {
                println!(
                    "💾 Trading config saved successfully for user: {}",
                    config.username
                );
                Ok(())
            }
        }
        Err(e) => Err(format!("Failed to serialize config: {}", e)),
    }
}

fn default_trading_config() -> TradingConfigPayload {
    TradingConfigPayload {
        username: "default".to_string(),
        asset_list: vec![
            AssetItem {
                symbol: "R_10".to_string(),
                name: "Volatility 10 Index".to_string(),
            },
            AssetItem {
                symbol: "R_25".to_string(),
                name: "Volatility 25 Index".to_string(),
            },
            AssetItem {
                symbol: "R_50".to_string(),
                name: "Volatility 50 Index".to_string(),
            },
            AssetItem {
                symbol: "R_75".to_string(),
                name: "Volatility 75 Index".to_string(),
            },
            AssetItem {
                symbol: "R_100".to_string(),
                name: "Volatility 100 Index".to_string(),
            },
        ],
        default_asset: "R_10".to_string(),
        start_money_trade: 100.0,
        money_martin_gale: vec![1.0, 2.0, 6.0, 8.0, 16.0, 54.0, 162.0],
        trade_types: vec!["FixTrade".to_string(), "MartinGaleTrade".to_string()],
        selected_trade_type: "FixTrade".to_string(),
        target_money: 1000.0,
        ema_short_type: "ema".to_string(),
        ema_short_period: 3,
        ema_medium_type: "ema".to_string(),
        ema_medium_period: 8,
        ema_long_type: "ema".to_string(),
        ema_long_period: 21,
        thresholds: vec![],
    }
}

async fn get_trading_config_handler() -> Response {
    match load_trading_config() {
        Some(config) => match serde_json::to_string(&config) {
            Ok(json) => Response::builder()
                .status(200)
                .header("Content-Type", "application/json")
                .body(json.into())
                .unwrap(),
            Err(e) => Response::builder()
                .status(500)
                .header("Content-Type", "application/json")
                .body(format!("{{\"error\": \"{}\"}}", e).into())
                .unwrap(),
        },
        None => {
            // Return default config
            let default_config = default_trading_config();
            match serde_json::to_string(&default_config) {
                Ok(json) => Response::builder()
                    .status(200)
                    .header("Content-Type", "application/json")
                    .body(json.into())
                    .unwrap(),
                Err(e) => Response::builder()
                    .status(500)
                    .header("Content-Type", "application/json")
                    .body(format!("{{\"error\": \"{}\"}}", e).into())
                    .unwrap(),
            }
        }
    }
}

async fn save_trading_config_handler(
    axum::Json(payload): axum::Json<TradingConfigPayload>,
) -> Response {
    println!("📊 Saving trading config for user: {}", payload.username);

    match save_trading_config(&payload) {
        Ok(_) => Response::builder()
            .status(200)
            .header("Content-Type", "application/json")
            .body("{\"success\": true, \"message\": \"Config saved successfully\"}".into())
            .unwrap(),
        Err(e) => Response::builder()
            .status(500)
            .header("Content-Type", "application/json")
            .body(format!("{{\"success\": false, \"error\": \"{}\"}}", e).into())
            .unwrap(),
    }
}

// ==================== SAVE TRADE HANDLER ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveTradePayload {
    pub contract_id: String,
    pub symbol: String,
    pub trade_type: String,
    pub buy_price: f64,
    pub payout: f64,
    pub profit_loss: f64,
    pub status: String,
    pub trade_date: String,
    pub created_at: String,
    #[serde(default)]
    pub buy_time: u64,
    #[serde(default)]
    pub expiry_time: u64,
    #[serde(default)]
    pub entry_spot: f64,
    #[serde(default)]
    pub exit_spot: f64,
}

async fn save_trade_handler(
    State(state): State<Arc<AppState>>,
    axum::Json(payload): axum::Json<SaveTradePayload>,
) -> Response {
    println!(
        "💾 Save trade request: {} {} {} profit: {}",
        payload.contract_id, payload.symbol, payload.status, payload.profit_loss
    );

    // Create TradeRecord from payload
    let trade_record = TradeRecord {
        order_no: 0,
        contract_id: payload.contract_id.clone(),
        symbol: payload.symbol.clone(),
        trade_type: payload.trade_type.clone(),
        buy_price: payload.buy_price,
        payout: payload.payout,
        profit_loss: payload.profit_loss,
        buy_time: payload.buy_time,
        expiry_time: payload.expiry_time,
        time_remaining: 0,
        min_profit: 0.0,
        max_profit: 0.0,
        status: payload.status.clone(),
        entry_spot: payload.entry_spot,
        exit_spot: payload.exit_spot,
        lot_no: 0,
        trade_no_in_lot: 0,
        trade_date: payload.trade_date.clone(),
        created_at: payload.created_at.clone(),
    };

    // Save to Firestore
    let firestore = state.firestore.lock().await;
    match firestore.save_trade(&trade_record).await {
        Ok(doc_id) => {
            println!("✅ Trade saved to Firestore: {}", doc_id);
            Response::builder()
                .status(200)
                .header("Content-Type", "application/json")
                .body(format!("{{\"success\": true, \"doc_id\": \"{}\"}}", doc_id).into())
                .unwrap()
        }
        Err(e) => {
            println!("❌ Failed to save trade: {}", e);
            Response::builder()
                .status(500)
                .header("Content-Type", "application/json")
                .body(format!("{{\"success\": false, \"error\": \"{}\"}}", e).into())
                .unwrap()
        }
    }
}

// ==================== NEW DAY TRADE API ====================
pub async fn get_today_trade_history_handler() -> impl IntoResponse {
    let today = get_daily_folder_name();
    let path = format!("tradeHistory/{}/trade.json", today);

    match std::fs::read_to_string(&path) {
        Ok(contents) => Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "application/json")
            .body(axum::body::Body::from(contents))
            .unwrap(),
        Err(_) => {
            // If file doesn't exist, return empty JSON instead of error
            let empty = serde_json::json!({});
            axum::Json(empty).into_response()
        }
    }
}
