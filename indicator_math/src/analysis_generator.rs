// ============================================================
// analysis_generator.rs — Full Analysis Generator
// Rust implementation of clsAnalysisGenerator.js
// ============================================================

use crate::indicators::{adx, atr, bollinger_bands, choppiness_index, rsi, BollingerBands};
use crate::{calculate_ma, Candle, MaType, ValueAtTime};
use std::collections::HashMap;

// ============================================================
// Code Candle Master — StatusDesc to SeriesCode mapping
// ============================================================

/// Embedded mapping from StatusDesc to SeriesCode
/// Based on CodeCandleMaster.json
fn build_status_code_map() -> HashMap<&'static str, u32> {
    let mut map = HashMap::new();
    map.insert("L-DD-E-D", 1);
    map.insert("L-DD-G-C", 2);
    map.insert("L-DD-G-D", 3);
    map.insert("L-DD-G-N", 4);
    map.insert("L-DD-R-C", 5);
    map.insert("L-DD-R-D", 6);
    map.insert("L-DD-R-N", 7);
    map.insert("L-DF-G-C", 8);
    map.insert("L-DF-G-D", 9);
    map.insert("L-DF-G-N", 10);
    map.insert("L-DF-R-C", 11);
    map.insert("L-DF-R-D", 12);
    map.insert("L-DF-R-N", 13);
    map.insert("L-DU-G-C", 14);
    map.insert("L-DU-G-D", 15);
    map.insert("L-DU-G-N", 16);
    map.insert("L-DU-R-C", 17);
    map.insert("L-DU-R-D", 18);
    map.insert("L-DU-R-N", 19);
    map.insert("L-FD-G-C", 20);
    map.insert("L-FD-G-N", 21);
    map.insert("L-FD-R-C", 22);
    map.insert("L-FD-R-N", 23);
    map.insert("L-FF-G-C", 24);
    map.insert("L-FF-G-N", 25);
    map.insert("L-FF-R-N", 26);
    map.insert("L-FU-G-C", 27);
    map.insert("L-FU-G-D", 28);
    map.insert("L-FU-G-N", 29);
    map.insert("L-FU-R-D", 30);
    map.insert("L-FU-R-N", 31);
    map.insert("L-UD-G-C", 32);
    map.insert("L-UD-G-N", 33);
    map.insert("L-UD-R-C", 34);
    map.insert("L-UD-R-N", 35);
    map.insert("L-UF-G-C", 36);
    map.insert("L-UF-G-N", 37);
    map.insert("L-UU-G-C", 38);
    map.insert("L-UU-G-D", 39);
    map.insert("L-UU-G-N", 40);
    map.insert("L-UU-R-D", 41);
    map.insert("L-UU-R-N", 42);
    map.insert("M-DD-G-C", 43);
    map.insert("M-DD-G-D", 44);
    map.insert("M-DD-G-N", 45);
    map.insert("M-DD-R-C", 46);
    map.insert("M-DD-R-D", 47);
    map.insert("M-DD-R-N", 48);
    map.insert("M-DF-G-C", 49);
    map.insert("M-DF-G-N", 50);
    map.insert("M-DF-R-C", 51);
    map.insert("M-DF-R-N", 52);
    map.insert("M-DU-G-C", 53);
    map.insert("M-DU-G-N", 54);
    map.insert("M-DU-R-C", 55);
    map.insert("M-DU-R-N", 56);
    map.insert("M-FD-G-C", 57);
    map.insert("M-FD-G-D", 58);
    map.insert("M-FD-G-N", 59);
    map.insert("M-FD-R-D", 60);
    map.insert("M-FD-R-N", 61);
    map.insert("M-FU-G-C", 62);
    map.insert("M-FU-G-N", 63);
    map.insert("M-FU-R-C", 64);
    map.insert("M-FU-R-N", 65);
    map.insert("M-UD-E-C", 66);
    map.insert("M-UD-G-C", 67);
    map.insert("M-UD-G-D", 68);
    map.insert("M-UD-G-N", 69);
    map.insert("M-UD-R-C", 70);
    map.insert("M-UD-R-D", 71);
    map.insert("M-UD-R-N", 72);
    map.insert("M-UF-G-C", 73);
    map.insert("M-UF-G-D", 74);
    map.insert("M-UF-G-N", 75);
    map.insert("M-UF-R-D", 76);
    map.insert("M-UU-E-D", 77);
    map.insert("M-UU-E-N", 78);
    map.insert("M-UU-G-C", 79);
    map.insert("M-UU-G-D", 80);
    map.insert("M-UU-G-N", 81);
    map.insert("M-UU-R-C", 82);
    map.insert("M-UU-R-D", 83);
    map.insert("M-UU-R-N", 84);
    map
}

lazy_static::lazy_static! {
    static ref STATUS_CODE_MAP: HashMap<&'static str, u32> = build_status_code_map();
}

/// Lookup SeriesCode from StatusDesc
pub fn lookup_series_code(status_desc: &str) -> Option<u32> {
    STATUS_CODE_MAP.get(status_desc).copied()
}

// ============================================================
// Bollinger Band Position
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum BbPosition {
    NearUpper,
    Middle,
    NearLower,
    Unknown,
}

impl BbPosition {
    pub fn as_str(&self) -> &'static str {
        match self {
            BbPosition::NearUpper => "NearUpper",
            BbPosition::Middle => "Middle",
            BbPosition::NearLower => "NearLower",
            BbPosition::Unknown => "Unknown",
        }
    }
}

// ============================================================
// Full Analysis Result Struct
// ============================================================

#[derive(Debug, Clone)]
pub struct FullAnalysis {
    // Basic Info
    pub index: usize,
    pub candle_time: u64,
    pub candle_time_display: String,

    // OHLC
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,

    // Candle Colors
    pub color: String,
    pub next_color: Option<String>,
    pub pip_size: f64,

    // EMA Short
    pub ema_short_value: Option<f64>,
    pub ema_short_direction: String,
    pub ema_short_turn_type: String,

    // EMA Medium
    pub ema_medium_value: Option<f64>,
    pub ema_medium_direction: String,

    // EMA Long
    pub ema_long_value: Option<f64>,
    pub ema_long_direction: String,

    // EMA Relationships
    pub ema_above: Option<String>,      // ShortAbove, MediumAbove
    pub ema_long_above: Option<String>, // MediumAbove, LongAbove

    // MACD-like values
    pub macd_12: Option<f64>,
    pub macd_23: Option<f64>,

    // Previous EMA values
    pub previous_ema_short_value: Option<f64>,
    pub previous_ema_medium_value: Option<f64>,
    pub previous_ema_long_value: Option<f64>,
    pub previous_macd_12: Option<f64>,
    pub previous_macd_23: Option<f64>,

    // Convergence/Divergence
    pub ema_convergence_type: Option<String>,
    pub ema_long_convergence_type: Option<String>,

    // Additional Indicators
    pub choppy_indicator: Option<f64>,
    pub adx_value: Option<f64>,
    pub rsi_value: Option<f64>,

    // Bollinger Bands
    pub bb_upper: Option<f64>,
    pub bb_middle: Option<f64>,
    pub bb_lower: Option<f64>,
    pub bb_position: BbPosition,

    // ATR
    pub atr: Option<f64>,
    pub is_abnormal_candle: bool,
    pub is_abnormal_atr: bool,

    // Wick and Body
    pub upper_wick: f64,
    pub upper_wick_percent: f64,
    pub body: f64,
    pub body_percent: f64,
    pub lower_wick: f64,
    pub lower_wick_percent: f64,

    // EMA Cut Position
    pub ema_cut_position: Option<String>,
    pub ema_cut_long_type: Option<String>,
    pub candles_since_ema_cut: Option<usize>,

    // Consecutive EMA direction tracking
    pub up_con_medium_ema: usize,
    pub down_con_medium_ema: usize,
    pub up_con_long_ema: usize,
    pub down_con_long_ema: usize,

    // Status fields
    pub is_mark: String,
    pub status_code: Option<u32>,
    pub series_code: Option<u32>,
    pub status_desc: String,
    pub status_desc0: String,
    pub hint_status: String,
    pub suggest_color: String,
    pub win_status: String,
    pub win_con: i32,
    pub loss_con: i32,
}

// ============================================================
// Analysis Options
// ============================================================

#[derive(Debug, Clone)]
pub struct AnalysisOptions {
    pub ema1_period: usize,
    pub ema1_type: MaType,
    pub ema2_period: usize,
    pub ema2_type: MaType,
    pub ema3_period: usize,
    pub ema3_type: MaType,
    pub atr_period: usize,
    pub atr_multiplier: f64,
    pub bb_period: usize,
    pub ci_period: usize,
    pub adx_period: usize,
    pub rsi_period: usize,
    pub flat_threshold: f64,
    pub macd_narrow: f64,
}

impl Default for AnalysisOptions {
    fn default() -> Self {
        Self {
            ema1_period: 20,
            ema1_type: MaType::EMA,
            ema2_period: 50,
            ema2_type: MaType::EMA,
            ema3_period: 200,
            ema3_type: MaType::EMA,
            atr_period: 14,
            atr_multiplier: 2.0,
            bb_period: 20,
            ci_period: 14,
            adx_period: 14,
            rsi_period: 14,
            flat_threshold: 0.2,
            macd_narrow: 0.15,
        }
    }
}

// ============================================================
// Analysis Generator
// ============================================================

pub struct AnalysisGenerator {
    candles: Vec<Candle>,
    options: AnalysisOptions,

    // Calculated indicator data
    ema1_data: Vec<ValueAtTime>,
    ema2_data: Vec<ValueAtTime>,
    ema3_data: Vec<ValueAtTime>,
    atr_data: Vec<ValueAtTime>,
    ci_data: Vec<ValueAtTime>,
    adx_data: Vec<ValueAtTime>,
    rsi_data: Vec<ValueAtTime>,
    bb_data: BollingerBands,

    // Analysis result
    analysis_array: Vec<FullAnalysis>,
}

impl AnalysisGenerator {
    pub fn new(candles: Vec<Candle>, options: AnalysisOptions) -> Self {
        Self {
            candles,
            options,
            ema1_data: Vec::new(),
            ema2_data: Vec::new(),
            ema3_data: Vec::new(),
            atr_data: Vec::new(),
            ci_data: Vec::new(),
            adx_data: Vec::new(),
            rsi_data: Vec::new(),
            bb_data: BollingerBands {
                upper: Vec::new(),
                middle: Vec::new(),
                lower: Vec::new(),
            },
            analysis_array: Vec::new(),
        }
    }

    pub fn with_default_options(candles: Vec<Candle>) -> Self {
        Self::new(candles, AnalysisOptions::default())
    }

    /// Get EMA Direction based on previous and current values
    fn get_ema_direction(&self, prev: f64, curr: f64) -> String {
        let diff = prev - curr;
        if diff.abs() <= self.options.flat_threshold {
            "Flat".to_string()
        } else if prev < curr {
            "Up".to_string()
        } else {
            "Down".to_string()
        }
    }

    /// Get MACD Convergence Type
    fn get_macd_convergence(&self, prev_macd: f64, curr_macd: f64) -> Option<String> {
        if curr_macd.is_nan() || prev_macd.is_nan() {
            return None;
        }

        if curr_macd <= self.options.macd_narrow {
            return Some("N".to_string()); // Narrow
        }

        if curr_macd > prev_macd {
            Some("D".to_string()) // Divergence
        } else if curr_macd < prev_macd {
            Some("C".to_string()) // Convergence
        } else {
            None
        }
    }

    /// Get color from open/close
    fn get_color(open: f64, close: f64) -> String {
        if close > open {
            "Green".to_string()
        } else if close < open {
            "Red".to_string()
        } else {
            "Equal".to_string()
        }
    }

    /// Get EMA Cut Position relative to candle
    fn get_ema_cut_position(candle: &Candle, ema_value: f64) -> Option<String> {
        if ema_value.is_nan() {
            return None;
        }

        let body_top = candle.open.max(candle.close);
        let body_bottom = candle.open.min(candle.close);

        if ema_value > candle.high {
            Some("1".to_string())
        } else if ema_value >= body_top && ema_value <= candle.high {
            Some("2".to_string())
        } else if ema_value >= body_bottom && ema_value < body_top {
            let body_range = body_top - body_bottom;
            if body_range > 0.0 {
                let position_in_body = (ema_value - body_bottom) / body_range;
                if position_in_body >= 0.66 {
                    Some("B1".to_string())
                } else if position_in_body >= 0.33 {
                    Some("B2".to_string())
                } else {
                    Some("B3".to_string())
                }
            } else {
                Some("B2".to_string())
            }
        } else if ema_value >= candle.low && ema_value < body_bottom {
            Some("3".to_string())
        } else if ema_value < candle.low {
            Some("4".to_string())
        } else {
            None
        }
    }

    /// Generate analysis data
    pub fn generate(&mut self) -> &Vec<FullAnalysis> {
        if self.candles.is_empty() {
            return &self.analysis_array;
        }

        // Calculate all indicators
        self.ema1_data = calculate_ma(
            &self.candles,
            self.options.ema1_period,
            self.options.ema1_type,
        );
        self.ema2_data = calculate_ma(
            &self.candles,
            self.options.ema2_period,
            self.options.ema2_type,
        );
        self.ema3_data = calculate_ma(
            &self.candles,
            self.options.ema3_period,
            self.options.ema3_type,
        );
        self.atr_data = atr(&self.candles, self.options.atr_period);
        self.ci_data = choppiness_index(&self.candles, self.options.ci_period);
        let adx_result = adx(&self.candles, self.options.adx_period);
        self.adx_data = adx_result.adx;
        self.rsi_data = rsi(&self.candles, self.options.rsi_period);
        self.bb_data = bollinger_bands(&self.candles, self.options.bb_period);

        // Clear previous analysis
        self.analysis_array.clear();

        let mut last_ema_cut_index: Option<usize> = None;
        let mut up_con_medium_ema = 0usize;
        let mut down_con_medium_ema = 0usize;
        let mut up_con_long_ema = 0usize;
        let mut down_con_long_ema = 0usize;

        for i in 0..self.candles.len() {
            let candle = &self.candles[i];
            let prev_candle = if i > 0 {
                Some(&self.candles[i - 1])
            } else {
                None
            };
            let next_candle = if i < self.candles.len() - 1 {
                Some(&self.candles[i + 1])
            } else {
                None
            };

            // Basic candle info
            let color = Self::get_color(candle.open, candle.close);
            let next_color = next_candle.map(|nc| Self::get_color(nc.open, nc.close));
            let pip_size = (candle.close - candle.open).abs();

            // EMA values
            let ema_short = self
                .ema1_data
                .get(i)
                .map(|v| v.value)
                .filter(|v| !v.is_nan());
            let ema_medium = self
                .ema2_data
                .get(i)
                .map(|v| v.value)
                .filter(|v| !v.is_nan());
            let ema_long = self
                .ema3_data
                .get(i)
                .map(|v| v.value)
                .filter(|v| !v.is_nan());

            // Previous EMA values
            let prev_ema_short = if i > 0 {
                self.ema1_data
                    .get(i - 1)
                    .map(|v| v.value)
                    .filter(|v| !v.is_nan())
            } else {
                None
            };
            let prev_ema_medium = if i > 0 {
                self.ema2_data
                    .get(i - 1)
                    .map(|v| v.value)
                    .filter(|v| !v.is_nan())
            } else {
                None
            };
            let prev_ema_long = if i > 0 {
                self.ema3_data
                    .get(i - 1)
                    .map(|v| v.value)
                    .filter(|v| !v.is_nan())
            } else {
                None
            };

            // EMA Directions
            let ema_short_direction = match (prev_ema_short, ema_short) {
                (Some(prev), Some(curr)) => self.get_ema_direction(prev, curr),
                _ => "Flat".to_string(),
            };

            let ema_medium_direction = match (prev_ema_medium, ema_medium) {
                (Some(prev), Some(curr)) => self.get_ema_direction(prev, curr),
                _ => "Flat".to_string(),
            };

            let ema_long_direction = match (prev_ema_long, ema_long) {
                (Some(prev), Some(curr)) => self.get_ema_direction(prev, curr),
                _ => "Flat".to_string(),
            };

            // Track consecutive EMA directions
            match ema_medium_direction.as_str() {
                "Up" => {
                    up_con_medium_ema += 1;
                    down_con_medium_ema = 0;
                }
                "Down" => {
                    down_con_medium_ema += 1;
                    up_con_medium_ema = 0;
                }
                _ => {}
            }

            match ema_long_direction.as_str() {
                "Up" => {
                    up_con_long_ema += 1;
                    down_con_long_ema = 0;
                }
                "Down" => {
                    down_con_long_ema += 1;
                    up_con_long_ema = 0;
                }
                _ => {}
            }

            // Short EMA Turn Type
            let ema_short_turn_type = if i >= 2 {
                let v_i2 = self.ema1_data.get(i - 2).map(|v| v.value);
                let v_i1 = prev_ema_short;
                let v_i0 = ema_short;

                match (v_i2, v_i1, v_i0) {
                    (Some(a), Some(b), Some(c)) if !a.is_nan() && !b.is_nan() && !c.is_nan() => {
                        let prev_diff = b - a;
                        let curr_diff = c - b;

                        let prev_dir = if prev_diff > 0.0001 {
                            "Up"
                        } else if prev_diff < -0.0001 {
                            "Down"
                        } else {
                            "Flat"
                        };
                        let curr_dir = if curr_diff > 0.0001 {
                            "Up"
                        } else if curr_diff < -0.0001 {
                            "Down"
                        } else {
                            "Flat"
                        };

                        if curr_dir == "Up" && prev_dir == "Down" {
                            "TurnUp".to_string()
                        } else if curr_dir == "Down" && prev_dir == "Up" {
                            "TurnDown".to_string()
                        } else {
                            "-".to_string()
                        }
                    }
                    _ => "-".to_string(),
                }
            } else {
                "-".to_string()
            };

            // EMA Above relationships
            let ema_above = match (ema_short, ema_medium) {
                (Some(s), Some(m)) => {
                    if s > m {
                        Some("ShortAbove".to_string())
                    } else {
                        Some("MediumAbove".to_string())
                    }
                }
                _ => None,
            };

            let ema_long_above = match (ema_medium, ema_long) {
                (Some(m), Some(l)) => {
                    if m > l {
                        Some("MediumAbove".to_string())
                    } else {
                        Some("LongAbove".to_string())
                    }
                }
                _ => None,
            };

            // MACD values
            let macd_12 = match (ema_short, ema_medium) {
                (Some(s), Some(m)) => Some((s - m).abs()),
                _ => None,
            };

            let macd_23 = match (ema_medium, ema_long) {
                (Some(m), Some(l)) => Some((m - l).abs()),
                _ => None,
            };

            // Previous MACD values
            let prev_macd_12 = match (prev_ema_short, prev_ema_medium) {
                (Some(s), Some(m)) => Some((s - m).abs()),
                _ => None,
            };

            let prev_macd_23 = match (prev_ema_medium, prev_ema_long) {
                (Some(m), Some(l)) => Some((m - l).abs()),
                _ => None,
            };

            // Convergence types
            let ema_convergence_type = match (macd_12, prev_macd_12) {
                (Some(curr), Some(prev)) => {
                    if curr > prev {
                        Some("divergence".to_string())
                    } else if curr < prev {
                        Some("convergence".to_string())
                    } else {
                        Some("neutral".to_string())
                    }
                }
                _ => None,
            };

            let ema_long_convergence_type = match (macd_23, prev_macd_23) {
                (Some(curr), Some(prev)) => self.get_macd_convergence(prev, curr),
                _ => None,
            };

            // EMA Cut Long Type
            let ema_cut_long_type = if i > 0 {
                match (ema_long, ema_medium, prev_ema_long, prev_ema_medium) {
                    (Some(curr_l), Some(curr_m), Some(prev_l), Some(prev_m)) => {
                        let curr_medium_above = curr_m > curr_l;
                        let prev_medium_above = prev_m > prev_l;

                        if curr_medium_above != prev_medium_above {
                            if curr_medium_above {
                                Some("UpTrend".to_string())
                            } else {
                                Some("DownTrend".to_string())
                            }
                        } else {
                            None
                        }
                    }
                    _ => None,
                }
            } else {
                None
            };

            if ema_cut_long_type.is_some() {
                last_ema_cut_index = Some(i);
            }

            let candles_since_ema_cut = last_ema_cut_index.map(|idx| i - idx);

            // Additional indicators
            let ci_value = self.ci_data.get(i).map(|v| v.value).filter(|v| !v.is_nan());
            let adx_value = self
                .adx_data
                .get(i)
                .map(|v| v.value)
                .filter(|v| !v.is_nan());
            let rsi_value = self
                .rsi_data
                .get(i)
                .map(|v| v.value)
                .filter(|v| !v.is_nan());

            // Bollinger Bands
            let bb_upper = self
                .bb_data
                .upper
                .get(i)
                .map(|v| v.value)
                .filter(|v| !v.is_nan());
            let bb_middle = self
                .bb_data
                .middle
                .get(i)
                .map(|v| v.value)
                .filter(|v| !v.is_nan());
            let bb_lower = self
                .bb_data
                .lower
                .get(i)
                .map(|v| v.value)
                .filter(|v| !v.is_nan());

            // BB Position
            let bb_position = match (bb_upper, bb_lower) {
                (Some(upper), Some(lower)) => {
                    let bb_range = upper - lower;
                    let upper_zone = upper - (bb_range * 0.33);
                    let lower_zone = lower + (bb_range * 0.33);

                    if candle.close >= upper_zone {
                        BbPosition::NearUpper
                    } else if candle.close <= lower_zone {
                        BbPosition::NearLower
                    } else {
                        BbPosition::Middle
                    }
                }
                _ => BbPosition::Unknown,
            };

            // ATR
            let atr_value = self
                .atr_data
                .get(i)
                .map(|v| v.value)
                .filter(|v| !v.is_nan());

            // Abnormal candle detection
            let is_abnormal_candle = match (atr_value, prev_candle) {
                (Some(atr), Some(prev)) => {
                    let true_range = (candle.high - candle.low)
                        .max((candle.high - prev.close).abs())
                        .max((candle.low - prev.close).abs());
                    true_range > (atr * self.options.atr_multiplier)
                }
                _ => false,
            };

            let is_abnormal_atr = match atr_value {
                Some(atr) if atr > 0.0 => {
                    let body_size = (candle.close - candle.open).abs();
                    let full_candle_size = candle.high - candle.low;
                    body_size > atr * self.options.atr_multiplier
                        || full_candle_size > atr * self.options.atr_multiplier * 1.5
                }
                _ => false,
            };

            // Wick and Body calculations
            let body_top = candle.open.max(candle.close);
            let body_bottom = candle.open.min(candle.close);
            let upper_wick = candle.high - body_top;
            let body = (candle.close - candle.open).abs();
            let lower_wick = body_bottom - candle.low;
            let full_candle_size = candle.high - candle.low;

            let (body_percent, upper_wick_percent, lower_wick_percent) = if full_candle_size > 0.0 {
                (
                    (body / full_candle_size) * 100.0,
                    (upper_wick / full_candle_size) * 100.0,
                    (lower_wick / full_candle_size) * 100.0,
                )
            } else {
                (0.0, 0.0, 0.0)
            };

            // EMA Cut Position
            let ema_cut_position = ema_short.and_then(|v| Self::get_ema_cut_position(candle, v));

            // Build StatusDesc: {emaLongAbove[0]}-{emaMediumDir[0]}{emaLongDir[0]}-{color[0]}-{emaLongConvergenceType}
            let series_desc = format!(
                "{}-{}{}-{}-{}",
                ema_long_above.as_ref().map(|s| &s[..1]).unwrap_or("-"),
                &ema_medium_direction[..1],
                &ema_long_direction[..1],
                &color[..1],
                ema_long_convergence_type
                    .as_ref()
                    .unwrap_or(&"-".to_string())
            );

            // Lookup SeriesCode from StatusDesc
            let series_code = lookup_series_code(&series_desc);

            // Create timestamp display (simplified - just use Unix timestamp)
            let candle_time_display = format!("{}", candle.time);

            // Build analysis object
            let analysis = FullAnalysis {
                index: i,
                candle_time: candle.time,
                candle_time_display,
                open: candle.open,
                high: candle.high,
                low: candle.low,
                close: candle.close,
                color,
                next_color,
                pip_size,
                ema_short_value: ema_short,
                ema_short_direction,
                ema_short_turn_type,
                ema_medium_value: ema_medium,
                ema_medium_direction,
                ema_long_value: ema_long,
                ema_long_direction,
                ema_above,
                ema_long_above,
                macd_12,
                macd_23,
                previous_ema_short_value: prev_ema_short,
                previous_ema_medium_value: prev_ema_medium,
                previous_ema_long_value: prev_ema_long,
                previous_macd_12: prev_macd_12,
                previous_macd_23: prev_macd_23,
                ema_convergence_type,
                ema_long_convergence_type,
                choppy_indicator: ci_value,
                adx_value,
                rsi_value,
                bb_upper,
                bb_middle,
                bb_lower,
                bb_position,
                atr: atr_value,
                is_abnormal_candle,
                is_abnormal_atr,
                upper_wick,
                upper_wick_percent,
                body,
                body_percent,
                lower_wick,
                lower_wick_percent,
                ema_cut_position,
                ema_cut_long_type,
                candles_since_ema_cut,
                up_con_medium_ema,
                down_con_medium_ema,
                up_con_long_ema,
                down_con_long_ema,
                is_mark: "n".to_string(),
                status_code: None,
                series_code,
                status_desc: series_desc.clone(),
                status_desc0: series_desc,
                hint_status: String::new(),
                suggest_color: String::new(),
                win_status: String::new(),
                win_con: 0,
                loss_con: 0,
            };

            self.analysis_array.push(analysis);
        }

        // Update next_color for all items
        for i in 0..self.analysis_array.len().saturating_sub(1) {
            let next_color = self.analysis_array.get(i + 1).map(|a| a.color.clone());
            self.analysis_array[i].next_color = next_color;
        }

        &self.analysis_array
    }

    /// Get analysis results
    pub fn get_analysis(&self) -> &Vec<FullAnalysis> {
        &self.analysis_array
    }

    /// Get summary statistics
    pub fn get_summary(&self) -> Option<AnalysisSummary> {
        if self.analysis_array.is_empty() {
            return None;
        }

        let total = self.analysis_array.len();
        let green_count = self
            .analysis_array
            .iter()
            .filter(|a| a.color == "Green")
            .count();
        let red_count = self
            .analysis_array
            .iter()
            .filter(|a| a.color == "Red")
            .count();
        let abnormal_count = self
            .analysis_array
            .iter()
            .filter(|a| a.is_abnormal_candle)
            .count();
        let abnormal_atr_count = self
            .analysis_array
            .iter()
            .filter(|a| a.is_abnormal_atr)
            .count();
        let ema_crossover_count = self
            .analysis_array
            .iter()
            .filter(|a| a.ema_cut_long_type.is_some())
            .count();
        let uptrend_count = self
            .analysis_array
            .iter()
            .filter(|a| a.ema_cut_long_type.as_deref() == Some("UpTrend"))
            .count();
        let downtrend_count = self
            .analysis_array
            .iter()
            .filter(|a| a.ema_cut_long_type.as_deref() == Some("DownTrend"))
            .count();

        let latest = self.analysis_array.last().unwrap();

        Some(AnalysisSummary {
            total_candles: total,
            green_count,
            red_count,
            abnormal_count,
            abnormal_atr_count,
            ema_crossover_count,
            uptrend_count,
            downtrend_count,
            latest_ci: latest.choppy_indicator,
            latest_adx: latest.adx_value,
            latest_ema_short_direction: latest.ema_short_direction.clone(),
            latest_ema_medium_direction: latest.ema_medium_direction.clone(),
            latest_ema_long_direction: latest.ema_long_direction.clone(),
            latest_up_con_medium_ema: latest.up_con_medium_ema,
            latest_down_con_medium_ema: latest.down_con_medium_ema,
            latest_up_con_long_ema: latest.up_con_long_ema,
            latest_down_con_long_ema: latest.down_con_long_ema,
        })
    }
}

// ============================================================
// Analysis Summary
// ============================================================

#[derive(Debug, Clone)]
pub struct AnalysisSummary {
    pub total_candles: usize,
    pub green_count: usize,
    pub red_count: usize,
    pub abnormal_count: usize,
    pub abnormal_atr_count: usize,
    pub ema_crossover_count: usize,
    pub uptrend_count: usize,
    pub downtrend_count: usize,
    pub latest_ci: Option<f64>,
    pub latest_adx: Option<f64>,
    pub latest_ema_short_direction: String,
    pub latest_ema_medium_direction: String,
    pub latest_ema_long_direction: String,
    pub latest_up_con_medium_ema: usize,
    pub latest_down_con_medium_ema: usize,
    pub latest_up_con_long_ema: usize,
    pub latest_down_con_long_ema: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_candles() -> Vec<Candle> {
        vec![
            Candle {
                time: 1,
                open: 100.0,
                high: 105.0,
                low: 99.0,
                close: 104.0,
            },
            Candle {
                time: 2,
                open: 104.0,
                high: 108.0,
                low: 103.0,
                close: 107.0,
            },
            Candle {
                time: 3,
                open: 107.0,
                high: 110.0,
                low: 106.0,
                close: 105.0,
            },
            Candle {
                time: 4,
                open: 105.0,
                high: 107.0,
                low: 102.0,
                close: 103.0,
            },
            Candle {
                time: 5,
                open: 103.0,
                high: 106.0,
                low: 101.0,
                close: 105.0,
            },
            Candle {
                time: 6,
                open: 105.0,
                high: 109.0,
                low: 104.0,
                close: 108.0,
            },
            Candle {
                time: 7,
                open: 108.0,
                high: 112.0,
                low: 107.0,
                close: 111.0,
            },
            Candle {
                time: 8,
                open: 111.0,
                high: 115.0,
                low: 110.0,
                close: 114.0,
            },
            Candle {
                time: 9,
                open: 114.0,
                high: 116.0,
                low: 112.0,
                close: 113.0,
            },
            Candle {
                time: 10,
                open: 113.0,
                high: 115.0,
                low: 111.0,
                close: 112.0,
            },
        ]
    }

    #[test]
    fn test_analysis_generator() {
        let candles = sample_candles();
        let options = AnalysisOptions {
            ema1_period: 3,
            ema2_period: 5,
            ema3_period: 7,
            atr_period: 3,
            bb_period: 5,
            ci_period: 3,
            adx_period: 3,
            rsi_period: 5,
            ..Default::default()
        };

        let mut generator = AnalysisGenerator::new(candles, options);
        let results = generator.generate();

        assert_eq!(results.len(), 10);
        assert!(
            results[0].color == "Green" || results[0].color == "Red" || results[0].color == "Equal"
        );
    }

    #[test]
    fn test_series_code_lookup() {
        assert_eq!(lookup_series_code("L-DD-G-C"), Some(2));
        assert_eq!(lookup_series_code("M-UU-G-N"), Some(81));
        assert_eq!(lookup_series_code("INVALID"), None);
    }

    #[test]
    fn test_analysis_summary() {
        let candles = sample_candles();
        let mut generator = AnalysisGenerator::with_default_options(candles);
        generator.generate();

        let summary = generator.get_summary();
        assert!(summary.is_some());
        assert_eq!(summary.unwrap().total_candles, 10);
    }
}
