# Indicator Math

A comprehensive Rust library for technical analysis indicators and automated market analysis. Perfect for trading systems, backtesting frameworks, and financial analysis applications.

[![Crates.io](https://img.shields.io/crates/v/indicator_math.svg)](https://crates.io/crates/indicator_math)
[![Documentation](https://docs.rs/indicator_math/badge.svg)](https://docs.rs/indicator_math)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

## Features

- **Moving Averages**: SMA, EMA, WMA, HMA (Hull), EHMA (Exponential Hull)
- **Momentum Indicators**: RSI (Relative Strength Index)
- **Volatility Indicators**: ATR (Average True Range), Bollinger Bands
- **Trend Indicators**: ADX (Average Directional Index), Choppiness Index
- **Full Analysis Generator**: Complete market analysis with SeriesCode lookup

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
indicator_math = "0.8.0"
```

## Quick Start

### Basic Indicators

```rust
use indicator_math::{Candle, ema, rsi, atr, bollinger_bands, choppiness_index, adx};

// Create candle data
let candles = vec![
    Candle { time: 1, open: 100.0, high: 105.0, low: 99.0, close: 104.0 },
    Candle { time: 2, open: 104.0, high: 108.0, low: 103.0, close: 107.0 },
    // ... more candles
];

// Calculate indicators
let ema_values = ema(&candles, 20);
let rsi_values = rsi(&candles, 14);
let atr_values = atr(&candles, 14);
let bb = bollinger_bands(&candles, 20);
let ci = choppiness_index(&candles, 14);
let adx_result = adx(&candles, 14);

// Access values
println!("EMA: {}", ema_values.last().unwrap().value);
println!("RSI: {}", rsi_values.last().unwrap().value);
println!("BB Upper: {}", bb.upper.last().unwrap().value);
```

### Full Analysis Generator (AnalysisGenerator)

The `AnalysisGenerator` provides comprehensive market analysis similar to the JavaScript `clsAnalysisGenerator.js`:

```rust
use indicator_math::{
    Candle, AnalysisGenerator, AnalysisOptions, MaType, lookup_series_code
};

// Create candles
let candles = vec![/* ... candle data ... */];

// Configure options
let options = AnalysisOptions {
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
};

// Create generator and analyze
let mut generator = AnalysisGenerator::new(candles, options);
let analysis = generator.generate();

// Access analysis results
for item in analysis {
    println!("Time: {}", item.candle_time);
    println!("Color: {}", item.color);
    println!("StatusDesc: {}", item.status_desc);
    println!("SeriesCode: {:?}", item.series_code);
    println!("RSI: {:?}", item.rsi_value);
    println!("CI: {:?}", item.choppy_indicator);
    println!("ADX: {:?}", item.adx_value);
}

// Get summary statistics
if let Some(summary) = generator.get_summary() {
    println!("Total Candles: {}", summary.total_candles);
    println!("Green: {}, Red: {}", summary.green_count, summary.red_count);
    println!("Abnormal Candles: {}", summary.abnormal_count);
}
```

### SeriesCode Lookup

The library includes a built-in lookup table for `StatusDesc` to `SeriesCode` mapping:

```rust
use indicator_math::lookup_series_code;

// Lookup SeriesCode from StatusDesc
let code = lookup_series_code("L-DD-G-C");
assert_eq!(code, Some(2));

let code = lookup_series_code("M-UU-G-N");
assert_eq!(code, Some(81));
```

### Legacy Analysis (EmaAnalysis)

For simpler EMA-based analysis:

```rust
use indicator_math::{Candle, generate_analysis_data, MaType, get_action_by_simple, CutStrategy, get_action_by_cut_type};

let candles = vec![/* ... */];

// Generate analysis
let analysis = generate_analysis_data(
    &candles, 
    10, 20, 50,  // short, medium, long periods
    MaType::EMA, MaType::EMA, MaType::EMA
);

// Get trading signals
let last_idx = analysis.len() - 1;
let simple_action = get_action_by_simple(&analysis, last_idx);
let cut_action = get_action_by_cut_type(&analysis, last_idx, CutStrategy::ShortCut);

println!("Simple Action: {}", simple_action);  // "call", "put", or "hold"
println!("Cut Action: {}", cut_action);
```

## Available Indicators

### Moving Averages

| Function | Description |
|----------|-------------|
| `sma(candles, period)` | Simple Moving Average |
| `ema(candles, period)` | Exponential Moving Average |
| `wma(candles, period)` | Weighted Moving Average |
| `hma(candles, period)` | Hull Moving Average |
| `ehma(candles, period)` | Exponential Hull Moving Average |

### Momentum & Volatility

| Function | Description |
|----------|-------------|
| `rsi(candles, period)` | Relative Strength Index (0-100) |
| `atr(candles, period)` | Average True Range |
| `bollinger_bands(candles, period)` | Bollinger Bands (Upper, Middle, Lower) |

### Trend Indicators

| Function | Description |
|----------|-------------|
| `choppiness_index(candles, period)` | Choppiness Index (0-100) |
| `adx(candles, period)` | Average Directional Index with +DI and -DI |

## FullAnalysis Fields

The `FullAnalysis` struct provides comprehensive market data:

| Category | Fields |
|----------|--------|
| **Basic** | `index`, `candle_time`, `open`, `high`, `low`, `close`, `color`, `pip_size` |
| **Short EMA** | `ema_short_value`, `ema_short_direction`, `ema_short_turn_type` |
| **Medium EMA** | `ema_medium_value`, `ema_medium_direction` |
| **Long EMA** | `ema_long_value`, `ema_long_direction` |
| **Relationships** | `ema_above`, `ema_long_above`, `macd_12`, `macd_23` |
| **Convergence** | `ema_convergence_type`, `ema_long_convergence_type` |
| **Indicators** | `choppy_indicator`, `adx_value`, `rsi_value`, `atr` |
| **Bollinger** | `bb_upper`, `bb_middle`, `bb_lower`, `bb_position` |
| **Abnormal** | `is_abnormal_candle`, `is_abnormal_atr` |
| **Candle Body** | `upper_wick`, `body`, `lower_wick`, `*_percent` |
| **EMA Position** | `ema_cut_position`, `ema_cut_long_type`, `candles_since_ema_cut` |
| **Consecutive** | `up_con_medium_ema`, `down_con_medium_ema`, `up_con_long_ema`, `down_con_long_ema` |
| **Status** | `status_desc`, `series_code`, `is_mark`, `hint_status` |

## StatusDesc Format

The `status_desc` field follows this format: `{EmaLongAbove}-{MediumDir}{LongDir}-{Color}-{ConvergenceType}`

Examples:
- `L-DD-G-C` = LongAbove, Down-Down directions, Green candle, Convergence
- `M-UU-R-D` = MediumAbove, Up-Up directions, Red candle, Divergence

## License

MIT License - see [LICENSE](LICENSE) for details.
