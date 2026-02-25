// ============================================================
// indicators.rs — Additional Technical Indicators
// RSI, ATR, Bollinger Bands, Choppiness Index (CI), ADX
// ============================================================

use crate::{Candle, ValueAtTime};

// ============================================================
// RSI (Relative Strength Index)
// ============================================================

/// Calculate RSI (Relative Strength Index)
/// Returns a vector of ValueAtTime with RSI values (0-100)
pub fn rsi(candles: &[Candle], period: usize) -> Vec<ValueAtTime> {
    if candles.len() < period + 1 || period == 0 {
        return candles
            .iter()
            .map(|c| ValueAtTime {
                time: c.time,
                value: f64::NAN,
            })
            .collect();
    }

    let mut result = vec![
        ValueAtTime {
            time: 0,
            value: f64::NAN
        };
        candles.len()
    ];

    // Calculate gains and losses
    let mut gains = Vec::with_capacity(candles.len() - 1);
    let mut losses = Vec::with_capacity(candles.len() - 1);

    for i in 1..candles.len() {
        let change = candles[i].close - candles[i - 1].close;
        gains.push(if change > 0.0 { change } else { 0.0 });
        losses.push(if change < 0.0 { change.abs() } else { 0.0 });
    }

    // Calculate first average gain and loss
    let mut avg_gain: f64 = gains[..period].iter().sum::<f64>() / period as f64;
    let mut avg_loss: f64 = losses[..period].iter().sum::<f64>() / period as f64;

    // First RSI value
    let rs = if avg_loss == 0.0 {
        100.0
    } else {
        avg_gain / avg_loss
    };
    let rsi_val = 100.0 - (100.0 / (1.0 + rs));

    result[period] = ValueAtTime {
        time: candles[period].time,
        value: rsi_val,
    };

    // Calculate RSI using smoothed averages (Wilder's smoothing)
    for i in period..gains.len() {
        avg_gain = ((avg_gain * (period as f64 - 1.0)) + gains[i]) / period as f64;
        avg_loss = ((avg_loss * (period as f64 - 1.0)) + losses[i]) / period as f64;

        let rs = if avg_loss == 0.0 {
            100.0
        } else {
            avg_gain / avg_loss
        };
        let rsi_val = 100.0 - (100.0 / (1.0 + rs));

        result[i + 1] = ValueAtTime {
            time: candles[i + 1].time,
            value: rsi_val,
        };
    }

    result
}

// ============================================================
// ATR (Average True Range)
// ============================================================

/// Calculate True Range for a single candle
fn true_range(current: &Candle, previous: Option<&Candle>) -> f64 {
    match previous {
        Some(prev) => {
            let hl = current.high - current.low;
            let hc = (current.high - prev.close).abs();
            let lc = (current.low - prev.close).abs();
            hl.max(hc).max(lc)
        }
        None => current.high - current.low,
    }
}

/// Calculate ATR (Average True Range)
/// Uses Wilder's smoothing method
pub fn atr(candles: &[Candle], period: usize) -> Vec<ValueAtTime> {
    if candles.is_empty() || period == 0 {
        return vec![];
    }

    let mut result = Vec::with_capacity(candles.len());
    let mut current_atr = 0.0;

    for i in 0..candles.len() {
        let tr = true_range(
            &candles[i],
            if i > 0 { Some(&candles[i - 1]) } else { None },
        );

        if i < period {
            // Build up phase - use simple average
            current_atr = ((current_atr * i as f64) + tr) / (i + 1) as f64;
        } else {
            // Wilder's smoothing
            current_atr = ((current_atr * (period as f64 - 1.0)) + tr) / period as f64;
        }

        result.push(ValueAtTime {
            time: candles[i].time,
            value: current_atr,
        });
    }

    result
}

/// Calculate ATR as raw values (no time wrapping)
pub fn atr_values(candles: &[Candle], period: usize) -> Vec<f64> {
    atr(candles, period).iter().map(|v| v.value).collect()
}

// ============================================================
// Bollinger Bands
// ============================================================

#[derive(Debug, Clone)]
pub struct BollingerBands {
    pub upper: Vec<ValueAtTime>,
    pub middle: Vec<ValueAtTime>,
    pub lower: Vec<ValueAtTime>,
}

/// Calculate Bollinger Bands
/// Default multiplier is 2.0 standard deviations
pub fn bollinger_bands(candles: &[Candle], period: usize) -> BollingerBands {
    bollinger_bands_with_multiplier(candles, period, 2.0)
}

/// Calculate Bollinger Bands with custom multiplier
pub fn bollinger_bands_with_multiplier(
    candles: &[Candle],
    period: usize,
    multiplier: f64,
) -> BollingerBands {
    let len = candles.len();
    let mut upper = vec![
        ValueAtTime {
            time: 0,
            value: f64::NAN
        };
        len
    ];
    let mut middle = vec![
        ValueAtTime {
            time: 0,
            value: f64::NAN
        };
        len
    ];
    let mut lower = vec![
        ValueAtTime {
            time: 0,
            value: f64::NAN
        };
        len
    ];

    if len < period || period == 0 {
        return BollingerBands {
            upper,
            middle,
            lower,
        };
    }

    for i in period - 1..len {
        let start_idx = i.saturating_sub(period - 1);
        let slice: Vec<f64> = candles[start_idx..=i].iter().map(|c| c.close).collect();
        let avg = slice.iter().sum::<f64>() / period as f64;
        let variance = slice.iter().map(|x| (x - avg).powi(2)).sum::<f64>() / period as f64;
        let std_dev = variance.sqrt();

        upper[i] = ValueAtTime {
            time: candles[i].time,
            value: avg + (multiplier * std_dev),
        };
        middle[i] = ValueAtTime {
            time: candles[i].time,
            value: avg,
        };
        lower[i] = ValueAtTime {
            time: candles[i].time,
            value: avg - (multiplier * std_dev),
        };
    }

    BollingerBands {
        upper,
        middle,
        lower,
    }
}

// ============================================================
// Choppiness Index (CI)
// ============================================================

/// Calculate Choppiness Index (CI)
/// Returns values between 0-100
/// High values (>61.8) indicate choppy/ranging market
/// Low values (<38.2) indicate trending market
pub fn choppiness_index(candles: &[Candle], period: usize) -> Vec<ValueAtTime> {
    let len = candles.len();

    if len < period || period == 0 {
        return candles
            .iter()
            .map(|c| ValueAtTime {
                time: c.time,
                value: f64::NAN,
            })
            .collect();
    }

    let atr_vals = atr_values(candles, period);
    let mut result = vec![
        ValueAtTime {
            time: 0,
            value: f64::NAN
        };
        len
    ];

    for i in period - 1..len {
        let start_idx = i.saturating_sub(period - 1);
        let slice = &candles[start_idx..=i];
        let highest = slice.iter().map(|c| c.high).fold(f64::MIN, f64::max);
        let lowest = slice.iter().map(|c| c.low).fold(f64::MAX, f64::min);

        let sum_atr: f64 = atr_vals[start_idx..=i].iter().sum();
        let range = highest - lowest;

        let ci = if range > 0.0 {
            100.0 * (sum_atr / range).log10() / (period as f64).log10()
        } else {
            0.0
        };

        result[i] = ValueAtTime {
            time: candles[i].time,
            value: ci,
        };
    }

    result
}

// ============================================================
// ADX (Average Directional Index)
// ============================================================

#[derive(Debug, Clone)]
pub struct AdxResult {
    pub adx: Vec<ValueAtTime>,
    pub plus_di: Vec<ValueAtTime>,
    pub minus_di: Vec<ValueAtTime>,
}

/// Calculate ADX (Average Directional Index)
/// Returns ADX, +DI, and -DI values
pub fn adx(candles: &[Candle], period: usize) -> AdxResult {
    let len = candles.len();
    let nan_val = || ValueAtTime {
        time: 0,
        value: f64::NAN,
    };

    if len < period * 2 || period == 0 {
        return AdxResult {
            adx: candles
                .iter()
                .map(|c| ValueAtTime {
                    time: c.time,
                    value: 0.0,
                })
                .collect(),
            plus_di: candles
                .iter()
                .map(|c| ValueAtTime {
                    time: c.time,
                    value: 0.0,
                })
                .collect(),
            minus_di: candles
                .iter()
                .map(|c| ValueAtTime {
                    time: c.time,
                    value: 0.0,
                })
                .collect(),
        };
    }

    let mut adx_result = vec![nan_val(); len];
    let mut plus_di_result = vec![nan_val(); len];
    let mut minus_di_result = vec![nan_val(); len];

    let mut tr_sum = 0.0;
    let mut pdm_sum = 0.0;
    let mut mdm_sum = 0.0;
    let mut dx_values: Vec<(u64, f64)> = Vec::new();

    for i in 1..len {
        let up_move = candles[i].high - candles[i - 1].high;
        let down_move = candles[i - 1].low - candles[i].low;

        let pdm = if up_move > down_move && up_move > 0.0 {
            up_move
        } else {
            0.0
        };
        let mdm = if down_move > up_move && down_move > 0.0 {
            down_move
        } else {
            0.0
        };

        let tr = true_range(&candles[i], Some(&candles[i - 1]));

        if i <= period {
            tr_sum += tr;
            pdm_sum += pdm;
            mdm_sum += mdm;
        } else {
            tr_sum = tr_sum - (tr_sum / period as f64) + tr;
            pdm_sum = pdm_sum - (pdm_sum / period as f64) + pdm;
            mdm_sum = mdm_sum - (mdm_sum / period as f64) + mdm;
        }

        if i >= period {
            let di_plus = if tr_sum != 0.0 {
                (pdm_sum / tr_sum) * 100.0
            } else {
                0.0
            };
            let di_minus = if tr_sum != 0.0 {
                (mdm_sum / tr_sum) * 100.0
            } else {
                0.0
            };

            plus_di_result[i] = ValueAtTime {
                time: candles[i].time,
                value: di_plus,
            };
            minus_di_result[i] = ValueAtTime {
                time: candles[i].time,
                value: di_minus,
            };

            let dx = if di_plus + di_minus != 0.0 {
                ((di_plus - di_minus).abs() / (di_plus + di_minus)) * 100.0
            } else {
                0.0
            };

            dx_values.push((candles[i].time, dx));
        }
    }

    // Calculate ADX from DX values
    let mut adx_val = 0.0;
    for (j, (time, dx)) in dx_values.iter().enumerate() {
        if j < period {
            adx_val += dx / period as f64;
        } else {
            adx_val = ((adx_val * (period as f64 - 1.0)) + dx) / period as f64;
        }

        if j >= period {
            let candle_idx = period + j;
            if candle_idx < len {
                adx_result[candle_idx] = ValueAtTime {
                    time: *time,
                    value: adx_val,
                };
            }
        }
    }

    AdxResult {
        adx: adx_result,
        plus_di: plus_di_result,
        minus_di: minus_di_result,
    }
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
            Candle {
                time: 11,
                open: 112.0,
                high: 114.0,
                low: 109.0,
                close: 110.0,
            },
            Candle {
                time: 12,
                open: 110.0,
                high: 113.0,
                low: 108.0,
                close: 111.0,
            },
            Candle {
                time: 13,
                open: 111.0,
                high: 115.0,
                low: 110.0,
                close: 114.0,
            },
            Candle {
                time: 14,
                open: 114.0,
                high: 118.0,
                low: 113.0,
                close: 117.0,
            },
            Candle {
                time: 15,
                open: 117.0,
                high: 120.0,
                low: 116.0,
                close: 119.0,
            },
        ]
    }

    #[test]
    fn test_rsi() {
        let candles = sample_candles();
        let result = rsi(&candles, 5);
        assert!(!result[6].value.is_nan());
    }

    #[test]
    fn test_atr() {
        let candles = sample_candles();
        let result = atr(&candles, 5);
        assert!(!result[4].value.is_nan());
    }

    #[test]
    fn test_bollinger_bands() {
        let candles = sample_candles();
        let bb = bollinger_bands(&candles, 5);
        assert!(!bb.middle[4].value.is_nan());
        assert!(bb.upper[4].value > bb.middle[4].value);
        assert!(bb.lower[4].value < bb.middle[4].value);
    }

    #[test]
    fn test_choppiness_index() {
        let candles = sample_candles();
        let ci = choppiness_index(&candles, 5);
        assert!(!ci[4].value.is_nan());
    }

    #[test]
    fn test_adx() {
        let candles = sample_candles();
        let result = adx(&candles, 5);
        // ADX needs 2*period data points to start producing values
        assert!(result.adx.len() == candles.len());
    }
}
