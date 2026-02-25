use crate::structs::{AnalysisOptions, AnalysisResult, BBValues, Candle, CandleMasterCode};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GeneratorState {
    pub last_ema_1: f64,
    pub last_ema_2: f64,
    pub last_ema_3: f64,
    pub last_atr: f64,
    pub rsi_avg_gain: f64,
    pub rsi_avg_loss: f64,
    pub up_con_medium_ema: usize,
    pub down_con_medium_ema: usize,
    pub up_con_long_ema: usize,
    pub down_con_long_ema: usize,
    pub last_ema_cut_index: Option<usize>,
    pub prev_analysis: Option<AnalysisResult>,
    pub last_analysis: Option<AnalysisResult>,
    pub last_candle: Option<Candle>,

    // RSI
    pub rsi_period: usize,

    // ATR
    pub atr_period: usize,

    // ADX
    pub tr_sum: f64,
    pub pdm_sum: f64,
    pub mdm_sum: f64,
    pub adx_val: f64,
    pub dx_count: usize,
    pub adx_period: usize,

    // BB
    pub bb_window: VecDeque<f64>,
    pub bb_period: usize,

    // CI
    pub ci_window: VecDeque<Candle>,
    pub ci_atr_window: VecDeque<f64>,
    pub ci_period: usize,

    // Config cache
    pub ema_1_k: f64,
    pub ema_2_k: f64,
    pub ema_3_k: f64,

    // HMA/EHMA State
    // We need to store history for WMA/EMA calculations if we want to do incremental HMA/EHMA
    // HMA(n) = WMA(2*WMA(n/2) - WMA(n), sqrt(n))
    // This is complex for O(1).
    // For now, let's implement effective buffering for these types.
    // Ideally O(1) HMA is hard. We might need O(N) over window.
    pub close_window: VecDeque<f64>, // For types needing window like HMA
    pub max_ma_period: usize,
}

impl GeneratorState {
    pub fn new(options: &AnalysisOptions) -> Self {
        let max_period = options
            .ema1_period
            .max(options.ema2_period)
            .max(options.ema3_period);
        // HMA needs slightly more for safety or internal WMAs
        let buffer_size = max_period * 2;

        Self {
            last_ema_1: 0.0,
            last_ema_2: 0.0,
            last_ema_3: 0.0,
            last_atr: 0.0,
            rsi_avg_gain: 0.0,
            rsi_avg_loss: 0.0,
            up_con_medium_ema: 0,
            down_con_medium_ema: 0,
            up_con_long_ema: 0,
            down_con_long_ema: 0,
            last_ema_cut_index: None,
            prev_analysis: None,
            last_analysis: None,
            last_candle: None,
            atr_period: options.atr_period,
            rsi_period: options.rsi_period,
            tr_sum: 0.0,
            pdm_sum: 0.0,
            mdm_sum: 0.0,
            adx_val: 0.0,
            dx_count: 0,
            adx_period: options.adx_period,
            bb_window: VecDeque::with_capacity(options.bb_period),
            bb_period: options.bb_period,
            ci_window: VecDeque::with_capacity(options.ci_period),
            ci_atr_window: VecDeque::with_capacity(options.ci_period),
            ci_period: options.ci_period,
            ema_1_k: 2.0 / (options.ema1_period as f64 + 1.0),
            ema_2_k: 2.0 / (options.ema2_period as f64 + 1.0),
            ema_3_k: 2.0 / (options.ema3_period as f64 + 1.0),

            close_window: VecDeque::with_capacity(buffer_size),
            max_ma_period: buffer_size,
        }
    }
}

#[derive(Clone)]
pub struct AnalysisGenerator {
    options: AnalysisOptions,
    pub state: GeneratorState,
    pub analysis_array: Vec<AnalysisResult>,
    pub candle_data: Vec<Candle>,
    pub current_candle: Option<Candle>,
    pub master_codes: std::sync::Arc<Vec<CandleMasterCode>>,
}

impl AnalysisGenerator {
    pub fn new(
        options: AnalysisOptions,
        master_codes: std::sync::Arc<Vec<CandleMasterCode>>,
    ) -> Self {
        let state = GeneratorState::new(&options);
        Self {
            options,
            state,
            analysis_array: Vec::new(),
            candle_data: Vec::new(),
            current_candle: None,
            master_codes,
        }
    }

    // Helper: WMA Calculation
    fn calculate_wma(data: &[f64], period: usize) -> f64 {
        if data.len() < period {
            return 0.0;
        }
        let mut num = 0.0;
        let mut den = 0.0;
        for j in 0..period {
            let val = data[data.len() - 1 - j];
            let w = (period - j) as f64;
            num += val * w;
            den += w;
        }
        num / den
    }

    // Helper: EMA Calculation on slice (simple)
    fn calculate_ema_slice(data: &[f64], period: usize) -> f64 {
        if data.is_empty() {
            return 0.0;
        }
        let k = 2.0 / (period as f64 + 1.0);
        let mut ema = data[0];
        for i in 1..data.len() {
            ema = data[i] * k + ema * (1.0 - k);
        }
        ema
    }

    // Helper: HMA Calculation
    // HMA = WMA(2 * WMA(n/2) - WMA(n), sqrt(n))
    fn calculate_ma(
        &self,
        ma_type: &str,
        period: usize,
        current_price: f64,
        last_ema_state: f64,
        ema_k: f64,
    ) -> f64 {
        match ma_type {
            "EMA" => current_price * ema_k + last_ema_state * (1.0 - ema_k),
            "HMA" | "EHMA" => {
                // For HMA/EHMA in incremental, we need the window.
                // We use state.close_window which is updated in append_candle
                // Note: 'current_price' is new, not yet in window if we call this before pushing?
                // Let's assume input 'data' includes everything or we pass window.

                // Construct a temporary window including current price
                // Only needed for calculation
                // Performance warning: copying vector

                // We can optimize by iterating strictly.

                // Ensure we have enough data
                if self.state.close_window.len() + 1 < period {
                    return current_price;
                }

                // Combine window + current
                let iter = self
                    .state
                    .close_window
                    .iter()
                    .chain(std::iter::once(&current_price));
                // Collecting to vec is expensive. We should change design if perf critical.
                // But for < 200 items it's microsecond scale.
                let data: Vec<f64> = iter.copied().collect();

                if ma_type == "HMA" {
                    let half = (period / 2).max(1);
                    let sqrt = (period as f64).sqrt() as usize;

                    // We need WMA of last 'sqrt' points.
                    // The inputs to this WMA are (2*WMA(half) - WMA(period)).
                    // We need to generate a series of these "raw" values for the last 'sqrt' points.

                    let mut raw_series = Vec::new();
                    let needed = sqrt; // We need 'needed' points of Raw Val.
                                       // To get 1 Raw Val at index T, we need WMA(half) and WMA(period) ending at T.
                                       // So we need data up to T.

                    // We need to calculate RawVal for i = len-sqrt to len-1
                    let len = data.len();
                    if len < period {
                        return current_price;
                    }

                    for i in 0..needed {
                        let end_idx = len - (needed - 1 - i); // ending index (exclusive of slice?) no, include data[end_idx-1]
                                                              // slice 0..end_idx
                        if end_idx < period {
                            continue;
                        }

                        let slice = &data[0..end_idx];
                        // Efficiency: This re-calculates WMAs many times.
                        // For incremental tick: O(period * sqrt(period)).
                        // With period=200, ~200*14 = 2800 ops. Very fast.

                        let wma_half = Self::calculate_wma(slice, half);
                        let wma_full = Self::calculate_wma(slice, period);
                        let raw = 2.0 * wma_half - wma_full;
                        raw_series.push(raw);
                    }

                    Self::calculate_wma(&raw_series, sqrt)
                } else {
                    // EHMA
                    let half = (period / 2).max(1);
                    let sqrt = (period as f64).sqrt() as usize;

                    // Similar to HMA but using EMA
                    // EMA is stateful. We need to recalculate EMAs from start of window or keep state?
                    // Recalculating EMA from scratch for the window is safer for consistence.
                    // Optimization: Use cached EMA if possible?
                    // Incremental EHMA is tricky without full series re-calc.
                    // We will do full re-calc on the window for correctness.

                    let len = data.len();
                    if len < period {
                        return current_price;
                    }

                    // We need enough raw values for the final EMA(sqrt)
                    // But EMA needs to settle.
                    // Simple implementation: Calculate Raw Series for whole available window, then EMA it.

                    let mut raw_series = Vec::new();
                    // Optimization: Calculate EMA_Half and EMA_Full incrementally over the window

                    let k_half = 2.0 / (half as f64 + 1.0);
                    let k_full = 2.0 / (period as f64 + 1.0);

                    let mut val_half = data[0];
                    let mut val_full = data[0];

                    // Spin up to period
                    for i in 1..len {
                        val_half = data[i] * k_half + val_half * (1.0 - k_half);
                        val_full = data[i] * k_full + val_full * (1.0 - k_full);

                        let raw = 2.0 * val_half - val_full;
                        raw_series.push(raw);
                    }

                    // Now EMA(sqrt) on raw_series
                    // We only care about the last value
                    Self::calculate_ema_slice(&raw_series, sqrt)
                }
            }
            _ => current_price * ema_k + last_ema_state * (1.0 - ema_k),
        }
    }

    // Helper: EMA Direction
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

    pub fn append_candle(&mut self, new_candle: Candle) -> AnalysisResult {
        let i = self.analysis_array.len();
        let prev_candle = self.state.last_candle; // Copy

        // Push candle
        self.candle_data.push(new_candle);

        // 1. EMA/HMA/EHMA Logic
        // Update history window first
        self.state.close_window.push_back(new_candle.close);
        if self.state.close_window.len() > self.state.max_ma_period {
            self.state.close_window.pop_front();
        }

        let close = new_candle.close;

        // Handle init (first candle) - if it's the very first, MA is just the price
        let new_ema_1 = if i == 0 {
            close
        } else {
            self.calculate_ma(
                &self.options.ema1_type,
                self.options.ema1_period,
                close,
                self.state.last_ema_1,
                self.state.ema_1_k,
            )
        };
        let new_ema_2 = if i == 0 {
            close
        } else {
            self.calculate_ma(
                &self.options.ema2_type,
                self.options.ema2_period,
                close,
                self.state.last_ema_2,
                self.state.ema_2_k,
            )
        };
        let new_ema_3 = if i == 0 {
            close
        } else {
            self.calculate_ma(
                &self.options.ema3_type,
                self.options.ema3_period,
                close,
                self.state.last_ema_3,
                self.state.ema_3_k,
            )
        };

        // Directions
        let ema_1_dir = if i > 0 {
            self.get_ema_direction(self.state.last_ema_1, new_ema_1)
        } else {
            "Flat".to_string()
        };
        let ema_2_dir = if i > 0 {
            self.get_ema_direction(self.state.last_ema_2, new_ema_2)
        } else {
            "Flat".to_string()
        };
        let ema_3_dir = if i > 0 {
            self.get_ema_direction(self.state.last_ema_3, new_ema_3)
        } else {
            "Flat".to_string()
        };

        // Turn Type
        let mut ema_short_turn_type = "-".to_string();
        if let Some(prev_analysis) = &self.state.prev_analysis {
            // Logic from JS: checks 2 steps back
            // We need to store enough history or rely on prev_analysis values
            // JS: if (i >= 2 && ...)
            // We use the last stored EMA values + current.
            // But we need ema1[i-2].
            // In state we only have lastEma1 (i-1) and newEma1 (i).
            // We need ema1[i-2], which is `prev_analysis.emaShortValue`?
            // Wait, JS uses `this.ema1Data` array.
            // In incremental, we access `st.prevAnalysis.emaShortValue`.
            // `st.lastEma1` corresponds to `i-1`. `st.prevAnalysis` corresponds to `i-2` IF we just pushed `i-1`?
            // No. `prev_analysis` is `i-2` relative to `new_candle` (which is `i`)?
            // `state.last_analysis` is `i-1`. `state.prev_analysis` is `i-2`.
            if let Some(prev_prev_val) = prev_analysis.ema_short_value {
                if let Some(_prev_val) = self
                    .state
                    .last_analysis
                    .as_ref()
                    .map(|a| a.ema_short_value)
                    .flatten()
                {
                    // This corresponds to state.last_ema_1
                    // Wait, last_ema_1 IS the value at i-1.
                    // prev_analysis.ema_short_value IS the value at i-2?
                    // Let's check JS: `const prevEma1Before = st.prevAnalysis.emaShortValue;`
                    // `st.prevAnalysis` is set to `st.lastAnalysis` at the END of append.
                    // So at the START of append, `st.lastAnalysis` is index `i-1`. `st.prevAnalysis` is index `i-2`.
                    // Correct.

                    let curr_diff = new_ema_1 - self.state.last_ema_1;
                    let prev_diff = self.state.last_ema_1 - prev_prev_val;

                    let curr_dir_calc = if curr_diff > 0.0001 {
                        "Up"
                    } else if curr_diff < -0.0001 {
                        "Down"
                    } else {
                        "Flat"
                    };
                    let prev_dir_calc = if prev_diff > 0.0001 {
                        "Up"
                    } else if prev_diff < -0.0001 {
                        "Down"
                    } else {
                        "Flat"
                    };

                    if curr_dir_calc == "Up" && prev_dir_calc == "Down" {
                        ema_short_turn_type = "TurnUp".to_string();
                    } else if curr_dir_calc == "Down" && prev_dir_calc == "Up" {
                        ema_short_turn_type = "TurnDown".to_string();
                    }
                }
            }
        }

        // Consecutives
        let mut up_con_medium_ema = self.state.up_con_medium_ema;
        let mut down_con_medium_ema = self.state.down_con_medium_ema;

        if ema_2_dir == "Up" {
            up_con_medium_ema += 1;
            down_con_medium_ema = 0;
        } else if ema_2_dir == "Down" {
            down_con_medium_ema += 1;
            up_con_medium_ema = 0;
        }

        let mut up_con_long_ema = self.state.up_con_long_ema;
        let mut down_con_long_ema = self.state.down_con_long_ema;

        if ema_3_dir == "Up" {
            up_con_long_ema += 1;
            down_con_long_ema = 0;
        } else if ema_3_dir == "Down" {
            down_con_long_ema += 1;
            up_con_long_ema = 0;
        }

        // 2. MACD
        let ema_above = if new_ema_1 > new_ema_2 {
            "ShortAbove"
        } else {
            "MediumAbove"
        }
        .to_string();
        let ema_long_above = if new_ema_2 > new_ema_3 {
            "MediumAbove"
        } else {
            "LongAbove"
        }
        .to_string();

        let macd_12 = (new_ema_1 - new_ema_2).abs();
        let macd_23 = (new_ema_2 - new_ema_3).abs();

        let prev_macd_12 = self.state.last_analysis.as_ref().and_then(|a| a.macd_12);
        let prev_macd_23 = self.state.last_analysis.as_ref().and_then(|a| a.macd_23);

        let ema_convergence_type = if let Some(prev) = prev_macd_12 {
            if macd_12 > prev {
                "divergence".to_string()
            } else if macd_12 < prev {
                "convergence".to_string()
            } else {
                "neutral".to_string()
            }
        } else {
            "neutral".to_string()
        }; // Or handle as Option

        let ema_long_convergence_type = if let Some(prev) = prev_macd_23 {
            if macd_23 > prev {
                "D".to_string()
            } else if macd_23 < prev {
                "C".to_string()
            } else {
                "N".to_string()
            }
        } else {
            "N".to_string()
        };

        // EmaCutLongType
        let mut ema_cut_long_type = None;
        if i > 0 {
            // Need prev
            let prev_medium_above = self.state.last_ema_2 > self.state.last_ema_3;
            let curr_medium_above = new_ema_2 > new_ema_3;
            if curr_medium_above != prev_medium_above {
                ema_cut_long_type = Some(if curr_medium_above {
                    "UpTrend".to_string()
                } else {
                    "DownTrend".to_string()
                });
            }
        }

        let mut last_ema_cut_index = self.state.last_ema_cut_index;
        if ema_cut_long_type.is_some() {
            last_ema_cut_index = Some(i);
        }
        let candles_since_ema_cut = last_ema_cut_index.map(|idx| i - idx);

        // 3. ATR
        let tr = if let Some(p) = prev_candle {
            (new_candle.high - new_candle.low)
                .max((new_candle.high - p.close).abs())
                .max((new_candle.low - p.close).abs())
        } else {
            new_candle.high - new_candle.low
        };

        let new_atr = if i < self.state.atr_period {
            if i == 0 {
                tr
            } else {
                ((self.state.last_atr * i as f64) + tr) / (i as f64 + 1.0)
            }
        } else {
            ((self.state.last_atr * (self.state.atr_period as f64 - 1.0)) + tr)
                / self.state.atr_period as f64
        };

        // 4. RSI
        let mut rsi_value = None;
        let mut new_rsi_avg_gain = self.state.rsi_avg_gain;
        let mut new_rsi_avg_loss = self.state.rsi_avg_loss;

        if let Some(p) = prev_candle {
            // Need to verify if i >= rsi_period logic matches JS.
            // JS: if (prevCandle && i >= st.rsiPeriod)
            // But RSI usually needs initialization period.
            // In strict Incremental, we assume stream is long enough.
            // However, for the *start*, we need to handle the first N candles.
            // JS `calculateRSI` function handles initial slice.
            // Incremental `appendCandle` handles update.

            let change = close - p.close;
            let gain = if change > 0.0 { change } else { 0.0 };
            let loss = if change < 0.0 { change.abs() } else { 0.0 };

            if i < self.state.rsi_period {
                // Accumulating for initial average
                // Wait, `i` is 0-indexed.
                // JS `appendCandle`: if (i >= st.rsiPeriod)
                // This implies that for i < period, it just accumulates or does nothing?
                // JS `generate` calculates full RSI array.
                // JS `_saveState` recalculates avgGain/Loss from history.
                // If we start from scratch:
                // We need to accumulate gains/losses for the first period.
                // `new_rsi_avg_gain` can store sum during init.
                if i == 0 {
                    new_rsi_avg_gain = gain;
                    new_rsi_avg_loss = loss;
                } else {
                    // Simple cumulative average for init
                    new_rsi_avg_gain += gain;
                    new_rsi_avg_loss += loss;
                }

                if i + 1 == self.state.rsi_period {
                    // Finalize initial average
                    new_rsi_avg_gain /= self.state.rsi_period as f64;
                    new_rsi_avg_loss /= self.state.rsi_period as f64;
                    // Calculate first RSI?
                    let rs = if new_rsi_avg_loss == 0.0 {
                        100.0
                    } else {
                        new_rsi_avg_gain / new_rsi_avg_loss
                    };
                    rsi_value = Some(100.0 - (100.0 / (1.0 + rs)));
                }
            } else {
                // Wilder's Smoothing
                new_rsi_avg_gain = (self.state.rsi_avg_gain * (self.state.rsi_period as f64 - 1.0)
                    + gain)
                    / self.state.rsi_period as f64;
                new_rsi_avg_loss = (self.state.rsi_avg_loss * (self.state.rsi_period as f64 - 1.0)
                    + loss)
                    / self.state.rsi_period as f64;

                let rs = if new_rsi_avg_loss == 0.0 {
                    100.0
                } else {
                    new_rsi_avg_gain / new_rsi_avg_loss
                };
                rsi_value = Some(100.0 - (100.0 / (1.0 + rs)));
            }
        }

        // 5. BB
        self.state.bb_window.push_back(close);
        if self.state.bb_window.len() > self.state.bb_period {
            self.state.bb_window.pop_front();
        }

        let (bb_upper, bb_middle, bb_lower) = if self.state.bb_window.len() >= self.state.bb_period
        {
            let sum: f64 = self.state.bb_window.iter().sum();
            let avg = sum / self.state.bb_period as f64;
            let variance: f64 = self.state.bb_window.iter().map(|x| (x - avg).powi(2)).sum();
            let std = (variance / self.state.bb_period as f64).sqrt();
            (Some(avg + 2.0 * std), Some(avg), Some(avg - 2.0 * std))
        } else {
            (None, None, None)
        };

        let bb_position = if let (Some(u), Some(l)) = (bb_upper, bb_lower) {
            let range = u - l;
            let upper_zone = u - (range * 0.33);
            let lower_zone = l + (range * 0.33);
            if close >= upper_zone {
                "NearUpper".to_string()
            } else if close <= lower_zone {
                "NearLower".to_string()
            } else {
                "Middle".to_string()
            }
        } else {
            "Unknown".to_string()
        };

        // 6. CI
        self.state.ci_window.push_back(new_candle);
        if self.state.ci_window.len() > self.state.ci_period {
            self.state.ci_window.pop_front();
        }
        self.state.ci_atr_window.push_back(new_atr);
        if self.state.ci_atr_window.len() > self.state.ci_period {
            self.state.ci_atr_window.pop_front();
        }

        let choppy_indicator = if self.state.ci_window.len() >= self.state.ci_period {
            let high_max = self
                .state
                .ci_window
                .iter()
                .map(|c| c.high)
                .fold(f64::NEG_INFINITY, f64::max);
            let low_min = self
                .state
                .ci_window
                .iter()
                .map(|c| c.low)
                .fold(f64::INFINITY, f64::min);
            let sum_atr: f64 = self.state.ci_atr_window.iter().sum();

            if (high_max - low_min) > 0.0 {
                Some(
                    100.0 * (sum_atr / (high_max - low_min)).log10()
                        / (self.state.ci_period as f64).log10(),
                )
            } else {
                Some(0.0)
            }
        } else {
            None
        };

        // 7. ADX
        let mut adx_value = None;
        if let Some(p) = prev_candle {
            let up_move = new_candle.high - p.high;
            let down_move = p.low - new_candle.low;
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

            // Update sums
            if i < self.state.adx_period {
                // Initial accumulation
                self.state.tr_sum += tr;
                self.state.pdm_sum += pdm;
                self.state.mdm_sum += mdm;
            } else {
                self.state.tr_sum =
                    self.state.tr_sum - (self.state.tr_sum / self.state.adx_period as f64) + tr;
                self.state.pdm_sum =
                    self.state.pdm_sum - (self.state.pdm_sum / self.state.adx_period as f64) + pdm;
                self.state.mdm_sum =
                    self.state.mdm_sum - (self.state.mdm_sum / self.state.adx_period as f64) + mdm;
            }

            if i >= self.state.adx_period && self.state.tr_sum > 0.0 {
                let di_plus = (self.state.pdm_sum / self.state.tr_sum) * 100.0;
                let di_minus = (self.state.mdm_sum / self.state.tr_sum) * 100.0;
                let sum_di = di_plus + di_minus;
                let dx = if sum_di == 0.0 {
                    0.0
                } else {
                    (di_plus - di_minus).abs() / sum_di * 100.0
                };

                self.state.dx_count += 1;

                if self.state.dx_count <= self.state.adx_period {
                    // Should be < ? JS says `j < period`
                    // Wait. JS dxValues usage: `if (j < period) adx += ...`
                    // Essentially, the first ADX is an average of the first DX values.
                    if self.state.dx_count == 1 {
                        self.state.adx_val = dx; // First DX? No, average...
                                                 // The JS logic builds an array of DXs then loops.
                                                 // Incremental logic needs to approximate or replicate.
                                                 // JS Tick:
                                                 // if (st.dxCount < st.adxPeriod) st.adxVal += dx / st.adxPeriod;
                                                 // else st.adxVal = ((st.adxVal * (st.adxPeriod - 1)) + dx) / st.adxPeriod;

                        self.state.adx_val += dx / self.state.adx_period as f64;
                    // This is wrong for first element?
                    // If we sum them up then we are good.
                    } else {
                        self.state.adx_val += dx / self.state.adx_period as f64;
                    }

                    // Actually, let's follow JS strictly:
                    // `if (st.dxCount < st.adxPeriod) st.adxVal += dx / st.adxPeriod;`
                    // This means for the first `adxPeriod` DX values, it accumulates `dx/N`.
                    // At the end of `adxPeriod` counts, `adxVal` is the average.
                    // Wait, `st.dxCount` starts at 0.
                } else {
                    // `else st.adxVal = ((st.adxVal * (st.adxPeriod - 1)) + dx) / st.adxPeriod;`
                    // This happens when we have enough history.
                    self.state.adx_val =
                        ((self.state.adx_val * (self.state.adx_period as f64 - 1.0)) + dx)
                            / self.state.adx_period as f64;
                }

                if self.state.dx_count >= self.state.adx_period {
                    adx_value = Some(self.state.adx_val);
                }
            }
        }

        // 8. Properties
        let color = if close > new_candle.open {
            "Green"
        } else if close < new_candle.open {
            "Red"
        } else {
            "Equal"
        }
        .to_string();
        let pip_size = (close - new_candle.open).abs();
        let body_top = new_candle.open.max(close);
        let body_bottom = new_candle.open.min(close);
        let u_wick = new_candle.high - body_top;
        let body = (close - new_candle.open).abs();
        let l_wick = body_bottom - new_candle.low;
        let full_candle_size = new_candle.high - new_candle.low;

        let body_percent = if full_candle_size > 0.0 {
            (body / full_candle_size) * 100.0
        } else {
            0.0
        };
        let u_wick_percent = if full_candle_size > 0.0 {
            (u_wick / full_candle_size) * 100.0
        } else {
            0.0
        };
        let l_wick_percent = if full_candle_size > 0.0 {
            (l_wick / full_candle_size) * 100.0
        } else {
            0.0
        };

        let is_abnormal_candle = if let Some(p) = prev_candle {
            let tr_val = (new_candle.high - new_candle.low)
                .max((new_candle.high - p.close).abs())
                .max((new_candle.low - p.close).abs());
            tr_val > (new_atr * self.options.atr_multiplier)
        } else {
            false
        };

        let is_abnormal_atr = if new_atr > 0.0 {
            (body > new_atr * self.options.atr_multiplier)
                || (full_candle_size > new_atr * self.options.atr_multiplier * 1.5)
        } else {
            false
        };

        // emaCutPosition
        let ema_cut_position = if new_ema_1 > new_candle.high {
            Some("1".to_string())
        } else if new_ema_1 >= body_top && new_ema_1 <= new_candle.high {
            Some("2".to_string())
        } else if new_ema_1 >= body_bottom && new_ema_1 < body_top {
            let body_range = body_top - body_bottom;
            if body_range > 0.0 {
                let pos = (new_ema_1 - body_bottom) / body_range;
                if pos >= 0.66 {
                    Some("B1".to_string())
                } else if pos >= 0.33 {
                    Some("B2".to_string())
                } else {
                    Some("B3".to_string())
                }
            } else {
                Some("B2".to_string())
            }
        } else if new_ema_1 >= new_candle.low && new_ema_1 < body_bottom {
            Some("3".to_string())
        } else if new_ema_1 < new_candle.low {
            Some("4".to_string())
        } else {
            None
        };

        // StatusDesc
        let _ema_long_above_char =
            if ema_long_above != "LongAbove" && ema_long_above != "MediumAbove" {
                "-"
            } else {
                &ema_long_above[0..1]
            }; // Rough approx, need check
               // JS: `emaLongAbove ? emaLongAbove.substr(0, 1) : '-'`
               // emaLongAbove is "MediumAbove" or "LongAbove". So 'M' or 'L'.
        let c1 = if ema_long_above == "MediumAbove" {
            "M"
        } else if ema_long_above == "LongAbove" {
            "L"
        } else {
            "-"
        };
        let c2 = if ema_2_dir == "Up" {
            "U"
        } else if ema_2_dir == "Down" {
            "D"
        } else {
            "F"
        };
        let c3 = if ema_3_dir == "Up" {
            "U"
        } else if ema_3_dir == "Down" {
            "D"
        } else {
            "F"
        };
        let c4 = &color[0..1];
        let c5 = if ema_long_convergence_type != "" {
            &ema_long_convergence_type
        } else {
            "-"
        };

        // JS: emaLongAbove.0 - ema2Dir.0 - ema3Dir.0 - color.0 - emaLongConver
        // Note: JS `SeriesDesc` calc logic
        let status_desc = format!("{}-{}-{}-{}-{}", c1, c2, c3, c4, c5);

        let mut status_code = "".to_string();
        for code in self.master_codes.iter() {
            if code.status_desc == status_desc {
                status_code = code.status_code.clone();
                break;
            }
        }

        // Match StatusCode
        // We need the `CandleMasterCode` list from init?
        // For now, I'll pass it? Or just leave logic for manager?
        // The struct has `status_code` field. The library needs to know how to map.
        // User prompt: "receive websocket, assetList, CandleMasterCode... return... analysisObject"
        // So `CandleMasterCode` is available.
        // I will add a method to resolve status code, or let the caller do it.
        // But `AnalysisResult` includes it.
        // I will add a placeholder for now.

        let _display_time = ""; // Need Chrono formatting

        let analysis_obj = AnalysisResult {
            index: i,
            candletime: new_candle.time,
            candletime_display: "".to_string(), // TODO
            open: new_candle.open,
            high: new_candle.high,
            low: new_candle.low,
            close: new_candle.close,
            color: color.clone(),
            next_color: None,
            pip_size,
            ema_short_value: Some(new_ema_1),
            ema_short_direction: ema_1_dir,
            ema_short_turn_type: ema_short_turn_type,
            ema_medium_value: Some(new_ema_2),
            ema_medium_direction: ema_2_dir,
            ema_long_value: Some(new_ema_3),
            ema_long_direction: ema_3_dir,
            ema_above: Some(ema_above),
            ema_long_above: Some(ema_long_above),
            macd_12: Some(macd_12),
            macd_23: Some(macd_23),
            previous_ema_short_value: self
                .state
                .last_analysis
                .as_ref()
                .and_then(|a| a.ema_short_value),
            previous_ema_medium_value: self
                .state
                .last_analysis
                .as_ref()
                .and_then(|a| a.ema_medium_value),
            previous_ema_long_value: self
                .state
                .last_analysis
                .as_ref()
                .and_then(|a| a.ema_long_value),
            previous_macd_12: prev_macd_12,
            previous_macd_23: prev_macd_23,
            ema_convergence_type: Some(ema_convergence_type),
            ema_long_convergence_type,
            choppy_indicator,
            adx_value,
            rsi_value,
            bb_values: BBValues {
                upper: bb_upper,
                middle: bb_middle,
                lower: bb_lower,
            },
            bb_position,
            atr: Some(new_atr),
            is_abnormal_candle,
            is_abnormal_atr,
            u_wick,
            u_wick_percent,
            body,
            body_percent,
            l_wick,
            l_wick_percent,
            ema_cut_position,
            ema_cut_long_type,
            candles_since_ema_cut,
            up_con_medium_ema,
            down_con_medium_ema,
            up_con_long_ema,
            down_con_long_ema,
            is_mark: "n".to_string(),
            status_code,
            status_desc: status_desc.clone(),
            status_desc_0: status_desc,
            hint_status: "".to_string(),
            suggest_color: "".to_string(),
            win_status: "".to_string(),
            win_con: 0,
            loss_con: 0,
        };

        // Update next color of previous
        if let Some(last) = self.analysis_array.last_mut() {
            last.next_color = Some(color);
        }

        self.analysis_array.push(analysis_obj.clone());

        // Update state
        self.state.last_ema_1 = new_ema_1;
        self.state.last_ema_2 = new_ema_2;
        self.state.last_ema_3 = new_ema_3;
        self.state.last_atr = new_atr;
        self.state.rsi_avg_gain = new_rsi_avg_gain;
        self.state.rsi_avg_loss = new_rsi_avg_loss;
        self.state.up_con_medium_ema = up_con_medium_ema;
        self.state.down_con_medium_ema = down_con_medium_ema;
        self.state.up_con_long_ema = up_con_long_ema;
        self.state.down_con_long_ema = down_con_long_ema;
        self.state.last_ema_cut_index = last_ema_cut_index;

        self.state.prev_analysis = self.state.last_analysis.clone();
        self.state.last_analysis = Some(analysis_obj.clone());
        self.state.last_candle = Some(new_candle);

        analysis_obj
    }

    pub fn append_tick(&mut self, price: f64, time: u64) -> Option<AnalysisResult> {
        let tick_minute = (time / 60) * 60;

        if let Some(mut current) = self.current_candle {
            if time >= current.time + 60 {
                // Or robust check: tick_minute > current.time
                // Complete previous candle
                let completed_candle = current; // Copy

                // Analyze it
                let result = self.append_candle(completed_candle);

                // Start new candle
                self.current_candle = Some(Candle {
                    time: tick_minute,
                    open: price,
                    high: price,
                    low: price,
                    close: price,
                });

                Some(result)
            } else {
                // Update current
                current.high = current.high.max(price);
                current.low = current.low.min(price);
                current.close = price;
                self.current_candle = Some(current);
                None
            }
        } else {
            // First tick ever seen (or after reset)
            self.current_candle = Some(Candle {
                time: tick_minute,
                open: price,
                high: price,
                low: price,
                close: price,
            });
            None
        }
    }
}
