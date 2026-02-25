// ============================================================
// lib.rs — Pure Rust Technical Indicators Library
// ============================================================

// ------------------------------------------------------------
// Modules
// ------------------------------------------------------------

pub mod analysis_generator;
pub mod indicators;

// Re-exports for convenience
pub use analysis_generator::{
    lookup_series_code, AnalysisGenerator, AnalysisOptions, AnalysisSummary, BbPosition,
    FullAnalysis,
};
pub use indicators::{
    adx, atr, bollinger_bands, bollinger_bands_with_multiplier, choppiness_index, rsi, AdxResult,
    BollingerBands,
};

// ------------------------------------------------------------
// Structs
// ------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub struct Candle {
    pub time: u64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct ValueAtTime {
    pub time: u64,
    pub value: f64,
}

#[derive(Debug, Clone)]
pub struct EmaAnalysis {
    pub time_candle: u64,
    pub index: usize,
    pub color_candle: String,
    pub next_color_candle: String,

    // Short EMA
    pub ema_short_value: f64,
    pub ema_short_slope_value: f64,
    pub ema_short_slope_direction: String,
    pub is_ema_short_turn_type: String, // TurnUp, TurnDown
    pub ema_short_cut_position: String, // 1, 2, B1, B2, B3, 3, 4

    // Medium EMA
    pub ema_medium_value: f64,
    pub ema_medium_slope_direction: String,

    // Long EMA
    pub ema_long_value: f64,
    pub ema_long_slope_direction: String,

    // Relationships
    pub ema_above: String,      // Short vs Medium (ShortAbove, MediumAbove)
    pub ema_long_above: String, // Medium vs Long (MediumAbove, LongAbove)

    // MACD-like Diffs
    pub macd_12: f64, // abs(short - medium)
    pub macd_23: f64, // abs(medium - long)

    // Previous Values
    pub previous_ema_short_value: f64,
    pub previous_ema_medium_value: f64,
    pub previous_ema_long_value: f64,
    pub previous_macd_12: f64,
    pub previous_macd_23: f64,

    // Convergence/Divergence
    pub ema_convergence_type: String, // divergence, convergence, neutral (Short vs Medium)
    pub ema_long_convergence_type: String, // divergence, convergence, neutral (Medium vs Long)

    // Trends / Crossovers (Short vs Medium)
    pub ema_cut_short_type: String, // UpTrend (Short crosses Medium Up), DownTrend (Short crosses Medium Down), None
    pub candles_since_short_cut: usize,

    // Trends / Crossovers (Medium vs Long)
    pub ema_cut_long_type: String, // UpTrend (Golden Cross), DownTrend (Death Cross), None
    pub candles_since_ema_cut: usize,

    // Extra context
    pub previous_color_back1: String,
    pub previous_color_back3: String,
}

// ------------------------------------------------------------
// MA Type Enum
// ------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub enum MaType {
    EMA,
    HMA,
    WMA,
    SMA,
    EHMA,
}

#[derive(Debug, Clone, Copy)]
pub enum CutStrategy {
    ShortCut, // Use ema_cut_short_type + ema_short_slope_direction
    LongCut,  // Use ema_cut_long_type + ema_medium_slope_direction
}

// ============================================================
// Helper Functions
// ============================================================

fn extract_close(candles: &[Candle]) -> Vec<f64> {
    candles.iter().map(|c| c.close).collect()
}

fn wrap_output(candles: &[Candle], values: Vec<f64>) -> Vec<ValueAtTime> {
    candles
        .iter()
        .zip(values.iter())
        .map(|(c, v)| ValueAtTime {
            time: c.time,
            value: *v,
        })
        .collect()
}

// ============================================================
// Indicators (SMA, EMA, WMA, HMA, EHMA)
// ============================================================

pub fn sma(candles: &[Candle], period: usize) -> Vec<ValueAtTime> {
    let prices = extract_close(candles);
    let mut out = vec![f64::NAN; prices.len()];

    if period == 0 || prices.len() < period {
        return wrap_output(candles, out);
    }

    for i in period - 1..prices.len() {
        let sum: f64 = prices[i - period + 1..=i].iter().sum();
        out[i] = sum / period as f64;
    }

    wrap_output(candles, out)
}

pub fn ema(candles: &[Candle], period: usize) -> Vec<ValueAtTime> {
    let prices = extract_close(candles);
    let mut out = vec![f64::NAN; prices.len()];

    if period == 0 || prices.is_empty() {
        return wrap_output(candles, out);
    }

    let k = 2.0 / (period as f64 + 1.0);
    // Determine the first point to start calculating (simple SMA seed or just first price)
    // Matching typical EMA: if < period, usually NAN or seeded by SMA.
    // The previous implementation used SMA at period-1.
    // Let's stick to standard behavior: first 'period' points are NAN/buildup,
    // BUT common web chart libs often start earlier or use simple price.
    // Re-using previous logic:

    // Existing logic was:
    // if i < period - 1 => NAN
    // i == period - 1 => SMA
    // i > period - 1 => EMA

    let mut prev = 0.0;

    for i in 0..prices.len() {
        if i < period - 1 {
            out[i] = f64::NAN;
        } else if i == period - 1 {
            let sma_val: f64 = prices[0..period].iter().sum::<f64>() / period as f64;
            out[i] = sma_val;
            prev = sma_val;
        } else {
            prev = prices[i] * k + prev * (1.0 - k);
            out[i] = prev;
        }
    }

    wrap_output(candles, out)
}

pub fn wma(candles: &[Candle], period: usize) -> Vec<ValueAtTime> {
    let prices = extract_close(candles);
    let mut out = vec![f64::NAN; prices.len()];

    if period == 0 || prices.len() < period {
        return wrap_output(candles, out);
    }

    let denom = (period * (period + 1) / 2) as f64;

    for i in period - 1..prices.len() {
        let mut sum = 0.0;
        for j in 0..period {
            sum += prices[i - j] * (period - j) as f64;
        }
        out[i] = sum / denom;
    }

    wrap_output(candles, out)
}

fn wma_values(values: &[f64], period: usize) -> Vec<f64> {
    let mut out = vec![f64::NAN; values.len()];
    if period == 0 || values.len() < period {
        return out;
    }

    let denom = (period * (period + 1) / 2) as f64;

    for i in period - 1..values.len() {
        let mut sum = 0.0;
        for j in 0..period {
            sum += values[i - j] * (period - j) as f64;
        }
        out[i] = sum / denom;
    }

    out
}

pub fn hma(candles: &[Candle], period: usize) -> Vec<ValueAtTime> {
    if period < 2 {
        return wrap_output(candles, vec![f64::NAN; candles.len()]);
    }

    let prices = extract_close(candles);
    let half = period / 2;
    let sqrt_n = (period as f64).sqrt().round() as usize;

    let w1 = wma_values(&prices, half);
    let w2 = wma_values(&prices, period);

    // 2 * WMA(n/2) - WMA(n)
    let diff: Vec<f64> = w1.iter().zip(w2.iter()).map(|(a, b)| 2.0 * a - b).collect();
    let h = wma_values(&diff, sqrt_n);

    wrap_output(candles, h)
}

pub fn ehma(candles: &[Candle], period: usize) -> Vec<ValueAtTime> {
    // Note: Previous implementation used EMA(period) vs EMA(period)... wait.
    // Standard EHMA or similar might be 2*EMA(n/2) - EMA(n).
    // Let's check `indicator.js`:
    // calculateEHMA uses: 2 * emaHalf - emaFull, then ema(sqrt) of that.

    // Re-implementing correctly based on JS logic:
    let ema_full = ema(candles, period);
    let ema_half = ema(candles, period / 2);

    let raw: Vec<Candle> = candles
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let val_full = ema_full[i].value;
            let val_half = ema_half[i].value;
            // If either is NAN, result is NAN
            let res = if val_full.is_nan() || val_half.is_nan() {
                f64::NAN
            } else {
                2.0 * val_half - val_full
            };
            Candle {
                time: c.time,
                open: c.open,
                high: c.high,
                low: c.low,
                close: res,
            }
        })
        .collect();

    let sqrt_n = (period as f64).sqrt().round() as usize;
    ema(&raw, sqrt_n)
}

// ============================================================
// Logic Helpers
// ============================================================

// Removed unused slope function

fn slope_direction(v: f64) -> String {
    if v > 0.0001 {
        "Up".to_string()
    } else if v < -0.0001 {
        "Down".to_string()
    } else {
        "Flat".to_string()
    }
}

fn turn_type(prev_diff: f64, curr_diff: f64) -> String {
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
        "None".to_string()
    }
}

fn get_ema_cut_position(c: &Candle, v: f64) -> String {
    if v.is_nan() {
        return "Unknown".to_string();
    }

    let body_top = c.close.max(c.open);
    let body_bottom = c.close.min(c.open);

    if v > c.high {
        return "1".to_string(); // Above Upper Wick
    }
    if v >= body_top {
        return "2".to_string(); // Upper Wick Area
    }
    if v >= body_bottom {
        // Inside Body
        let height = body_top - body_bottom;
        if height == 0.0 {
            return "B2".to_string();
        } // Doji

        let ratio = (v - body_bottom) / height;
        if ratio >= 0.66 {
            return "B1".to_string();
        } else if ratio >= 0.33 {
            return "B2".to_string();
        } else {
            return "B3".to_string();
        }
    }
    if v >= c.low {
        return "3".to_string(); // Lower Wick aArea
    }

    "4".to_string() // Below Low
}

fn get_color(open: f64, close: f64) -> String {
    if close > open {
        "Green".to_string()
    } else if close < open {
        "Red".to_string()
    } else {
        "Equal".to_string()
    }
}

fn calculate_ma(candles: &[Candle], period: usize, ma_type: MaType) -> Vec<ValueAtTime> {
    match ma_type {
        MaType::EMA => ema(candles, period),
        MaType::HMA => hma(candles, period),
        MaType::WMA => wma(candles, period),
        MaType::SMA => sma(candles, period),
        MaType::EHMA => ehma(candles, period),
    }
}

// ============================================================
// Main Analysis Function
// ============================================================

/// Generate Analysis Data
/// Mimics the logic of generateAnalysisData() in indicator.js
pub fn generate_analysis_data(
    candles: &[Candle],
    short_p: usize,
    medium_p: usize,
    long_p: usize,
    short_type: MaType,
    medium_type: MaType,
    long_type: MaType,
) -> Vec<EmaAnalysis> {
    let ma_short = calculate_ma(candles, short_p, short_type);
    let ma_medium = calculate_ma(candles, medium_p, medium_type);
    let ma_long = calculate_ma(candles, long_p, long_type);

    let mut out = Vec::new();

    let mut last_ema_cut_short_index: Option<usize> = None;
    let mut last_ema_cut_long_index: Option<usize> = None;

    for i in 0..candles.len() {
        let c = &candles[i];
        let next_c = if i < candles.len() - 1 {
            Some(&candles[i + 1])
        } else {
            None
        };

        // Basic Candle Info
        let color_candle = get_color(c.open, c.close);
        let next_color_candle = if let Some(nc) = next_c {
            get_color(nc.open, nc.close)
        } else {
            "Unknown".to_string()
        };

        // Values
        let short_val = ma_short[i].value;
        let medium_val = ma_medium[i].value;
        let long_val = ma_long[i].value;

        // Previous Values (Index i-1)
        let (prev_short, prev_medium, prev_long) = if i > 0 {
            (
                ma_short[i - 1].value,
                ma_medium[i - 1].value,
                ma_long[i - 1].value,
            )
        } else {
            (f64::NAN, f64::NAN, f64::NAN)
        };

        // Slopes & Directions (Short)
        let short_diff = if !short_val.is_nan() && !prev_short.is_nan() {
            short_val - prev_short
        } else {
            0.0
        };
        let short_slope_dir = slope_direction(short_diff);

        // Turn Type (Short)
        let mut short_turn = "None".to_string();
        if i >= 2 {
            let val_i2 = ma_short[i - 2].value; // i-2
            let val_i1 = prev_short; // i-1
                                     // if we have valid history
            if !val_i2.is_nan() && !val_i1.is_nan() && !short_val.is_nan() {
                let prev_diff = val_i1 - val_i2;
                let curr_diff = short_val - val_i1;
                short_turn = turn_type(prev_diff, curr_diff);
            }
        }

        // Medium Direction
        let medium_diff = if !medium_val.is_nan() && !prev_medium.is_nan() {
            medium_val - prev_medium
        } else {
            0.0
        };
        let medium_slope_dir = slope_direction(medium_diff);

        // Long Direction
        let long_diff = if !long_val.is_nan() && !prev_long.is_nan() {
            long_val - prev_long
        } else {
            0.0
        };
        let long_slope_dir = slope_direction(long_diff);

        // EMA Cuts / Relationships
        let ema_above = if !short_val.is_nan() && !medium_val.is_nan() {
            if short_val > medium_val {
                "ShortAbove".to_string()
            } else {
                "MediumAbove".to_string()
            }
        } else {
            "Unknown".to_string()
        };

        let ema_long_above = if !medium_val.is_nan() && !long_val.is_nan() {
            if medium_val > long_val {
                "MediumAbove".to_string()
            } else {
                "LongAbove".to_string()
            }
        } else {
            "Unknown".to_string()
        };

        // MACD Values
        let macd_12 = if !short_val.is_nan() && !medium_val.is_nan() {
            (short_val - medium_val).abs()
        } else {
            f64::NAN
        };
        let macd_23 = if !medium_val.is_nan() && !long_val.is_nan() {
            (medium_val - long_val).abs()
        } else {
            f64::NAN
        };

        // Previous MACD
        let prev_macd_12 = if !prev_short.is_nan() && !prev_medium.is_nan() {
            (prev_short - prev_medium).abs()
        } else {
            f64::NAN
        };
        let prev_macd_23 = if !prev_medium.is_nan() && !prev_long.is_nan() {
            (prev_medium - prev_long).abs()
        } else {
            f64::NAN
        };

        // Convergence Types
        let mut ema_convergence_type = "Neutral".to_string();
        if !macd_12.is_nan() && !prev_macd_12.is_nan() {
            if macd_12 > prev_macd_12 {
                ema_convergence_type = "divergence".to_string();
            } else if macd_12 < prev_macd_12 {
                ema_convergence_type = "convergence".to_string();
            }
        }

        let mut ema_long_convergence_type = "Neutral".to_string();
        if !macd_23.is_nan() && !prev_macd_23.is_nan() {
            if macd_23 > prev_macd_23 {
                ema_long_convergence_type = "divergence".to_string();
            } else if macd_23 < prev_macd_23 {
                ema_long_convergence_type = "convergence".to_string();
            }
        }

        // EMA Cut Short Type (Short vs Medium Cross)
        let mut ema_cut_short_type = "None".to_string();
        if i > 0
            && !short_val.is_nan()
            && !medium_val.is_nan()
            && !prev_short.is_nan()
            && !prev_medium.is_nan()
        {
            let curr_short_above = short_val > medium_val;
            let prev_short_above = prev_short > prev_medium;

            if curr_short_above != prev_short_above {
                if curr_short_above {
                    ema_cut_short_type = "UpTrend".to_string();
                } else {
                    ema_cut_short_type = "DownTrend".to_string();
                }
            }
        }

        if ema_cut_short_type != "None" {
            last_ema_cut_short_index = Some(i);
        }

        let candles_since_short_cut = if let Some(idx) = last_ema_cut_short_index {
            i - idx
        } else {
            0
        };

        // EMA Cut Long Type (Medium vs Long Cross Analysis)
        let mut ema_cut_long_type = "None".to_string();
        // Needs history
        if i > 0
            && !medium_val.is_nan()
            && !long_val.is_nan()
            && !prev_medium.is_nan()
            && !prev_long.is_nan()
        {
            let curr_medium_above = medium_val > long_val;
            let prev_medium_above = prev_medium > prev_long;

            if curr_medium_above != prev_medium_above {
                if curr_medium_above {
                    ema_cut_long_type = "UpTrend".to_string(); // Golden Cross
                } else {
                    ema_cut_long_type = "DownTrend".to_string(); // Death Cross
                }
            }
        }

        if ema_cut_long_type != "None" {
            last_ema_cut_long_index = Some(i);
        }

        let candles_since_ema_cut = if let Some(idx) = last_ema_cut_long_index {
            i - idx
        } else {
            0
        };

        // Cut Position
        let cut_pos = get_ema_cut_position(c, short_val);

        // History Colors
        let prev_color_1 = if i >= 1 {
            get_color(candles[i - 1].open, candles[i - 1].close)
        } else {
            "Unknown".to_string()
        };
        let prev_color_3 = if i >= 3 {
            get_color(candles[i - 3].open, candles[i - 3].close)
        } else {
            "Unknown".to_string()
        };

        out.push(EmaAnalysis {
            time_candle: c.time,
            index: i,
            color_candle,
            next_color_candle,

            ema_short_value: short_val,
            ema_short_slope_value: short_diff,
            ema_short_slope_direction: short_slope_dir,
            is_ema_short_turn_type: short_turn,
            ema_short_cut_position: cut_pos,

            ema_medium_value: medium_val,
            ema_medium_slope_direction: medium_slope_dir,

            ema_long_value: long_val,
            ema_long_slope_direction: long_slope_dir,

            ema_above,
            ema_long_above,

            macd_12,
            macd_23,

            previous_ema_short_value: prev_short,
            previous_ema_medium_value: prev_medium,
            previous_ema_long_value: prev_long,
            previous_macd_12: prev_macd_12,
            previous_macd_23: prev_macd_23,

            ema_convergence_type,
            ema_long_convergence_type,

            ema_cut_short_type,
            candles_since_short_cut,

            ema_cut_long_type,
            candles_since_ema_cut,

            previous_color_back1: prev_color_1,
            previous_color_back3: prev_color_3,
        });
    }

    out
}

// ============================================================
// Action Logic
// ============================================================

pub fn get_action_by_simple(results: &[EmaAnalysis], index: usize) -> &'static str {
    if let Some(analysis) = results.get(index) {
        match analysis.ema_above.as_str() {
            "ShortAbove" => "call",
            "MediumAbove" => "put", // Adjusted: if Medium > Short, usually PUT in this simple logic
            _ => "hold",
        }
    } else {
        "none"
    }
}

pub fn get_action_by_cut_type(
    results: &[EmaAnalysis],
    index: usize,
    use_cut_type: CutStrategy,
) -> &'static str {
    if let Some(analysis) = results.get(index) {
        match use_cut_type {
            CutStrategy::ShortCut => {
                // Use ema_cut_short_type + ema_short_slope_direction
                let trend = analysis.ema_cut_short_type.as_str();
                let slope = analysis.ema_short_slope_direction.as_str();

                if trend == "UpTrend" && slope == "Up" {
                    return "call";
                }
                if trend == "DownTrend" && slope == "Down" {
                    return "put";
                }
                "hold"
            }
            CutStrategy::LongCut => {
                // Use ema_cut_long_type + ema_medium_slope_direction
                let trend = analysis.ema_cut_long_type.as_str();
                let slope = analysis.ema_medium_slope_direction.as_str();

                if trend == "UpTrend" && slope == "Up" {
                    return "call";
                }
                if trend == "DownTrend" && slope == "Down" {
                    return "put";
                }
                "hold"
            }
        }
    } else {
        "none"
    }
}
