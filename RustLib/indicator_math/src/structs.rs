use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct Candle {
    pub time: u64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AnalysisOptions {
    pub ema1_period: usize,
    pub ema1_type: String, // "EMA", "HMA", "EHMA"
    pub ema2_period: usize,
    pub ema2_type: String,
    pub ema3_period: usize,
    pub ema3_type: String,
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
            ema1_type: "EMA".to_string(),
            ema2_period: 50,
            ema2_type: "EMA".to_string(),
            ema3_period: 200,
            ema3_type: "EMA".to_string(),
            atr_period: 14,
            atr_multiplier: 2.0,
            bb_period: 20,
            ci_period: 14,
            adx_period: 14,
            rsi_period: 14,
            flat_threshold: 0.2, // Adjust scaling if needed (JS uses raw values usually)
            macd_narrow: 0.15,
        }
    }
}

// Helper struct for parsing JSON array of MA configurations
#[derive(Serialize, Deserialize, Debug)]
struct MaConfigHelper {
    #[serde(alias = "type")]
    pub ma_type: String,
    pub period: usize,
}

impl AnalysisOptions {
    pub fn from_ma_config_json(
        json_str: &str,
        base_options: Option<AnalysisOptions>,
    ) -> Result<Self, serde_json::Error> {
        let configs: Vec<MaConfigHelper> = serde_json::from_str(json_str)?;
        let mut opts = base_options.unwrap_or_default();

        if configs.len() >= 1 {
            opts.ema1_type = configs[0].ma_type.clone();
            opts.ema1_period = configs[0].period;
        }
        if configs.len() >= 2 {
            opts.ema2_type = configs[1].ma_type.clone();
            opts.ema2_period = configs[1].period;
        }
        if configs.len() >= 3 {
            opts.ema3_type = configs[2].ma_type.clone();
            opts.ema3_period = configs[2].period;
        }

        Ok(opts)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AnalysisResult {
    pub index: usize,
    pub candletime: u64,
    pub candletime_display: String, // JS has this formatted string
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub color: String,              // "Green", "Red", "Equal"
    pub next_color: Option<String>, // Lookahead
    pub pip_size: f64,

    // EMA Short (1)
    pub ema_short_value: Option<f64>,
    pub ema_short_direction: String, // "Up", "Down", "Flat"
    pub ema_short_turn_type: String, // "TurnUp", "TurnDown", "-"

    // EMA Medium (2)
    pub ema_medium_value: Option<f64>,
    pub ema_medium_direction: String,

    // EMA Long (3)
    pub ema_long_value: Option<f64>,
    pub ema_long_direction: String,

    // Relations
    pub ema_above: Option<String>,      // "ShortAbove", "MediumAbove"
    pub ema_long_above: Option<String>, // "MediumAbove", "LongAbove"

    // MACD
    pub macd_12: Option<f64>, // Abs val in JS
    pub macd_23: Option<f64>,

    // Previous Values
    pub previous_ema_short_value: Option<f64>,
    pub previous_ema_medium_value: Option<f64>,
    pub previous_ema_long_value: Option<f64>,
    pub previous_macd_12: Option<f64>,
    pub previous_macd_23: Option<f64>,

    // Convergence
    pub ema_convergence_type: Option<String>, // "divergence", "convergence", "neutral"
    pub ema_long_convergence_type: String,    // "D", "C", "N"

    // Indicators
    pub choppy_indicator: Option<f64>,
    pub adx_value: Option<f64>,
    pub rsi_value: Option<f64>,

    pub bb_values: BBValues,
    pub bb_position: String, // "Unknown", "NearUpper", "NearLower", "Middle"

    pub atr: Option<f64>,
    pub is_abnormal_candle: bool,
    pub is_abnormal_atr: bool,

    // Wicks
    pub u_wick: f64,
    pub u_wick_percent: f64,
    pub body: f64,
    pub body_percent: f64,
    pub l_wick: f64,
    pub l_wick_percent: f64,

    // Cuts
    pub ema_cut_position: Option<String>, // "1", "2", "3", "4", "B1", "B2", "B3"
    pub ema_cut_long_type: Option<String>, // "UpTrend", "DownTrend"
    pub candles_since_ema_cut: Option<usize>,

    // Consecutive Counts
    pub up_con_medium_ema: usize,
    pub down_con_medium_ema: usize,
    pub up_con_long_ema: usize,
    pub down_con_long_ema: usize,

    // Status
    pub is_mark: String, // "n"
    pub status_code: String,
    pub status_desc: String,
    pub status_desc_0: String,
    pub hint_status: String,
    pub suggest_color: String,
    pub win_status: String,
    pub win_con: usize,
    pub loss_con: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BBValues {
    pub upper: Option<f64>,
    pub middle: Option<f64>,
    pub lower: Option<f64>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CandleMasterCode {
    pub status_code: String, // Using String instead of number to match flexibility, though prompt said 1, 2
    pub status_desc: String,
}
