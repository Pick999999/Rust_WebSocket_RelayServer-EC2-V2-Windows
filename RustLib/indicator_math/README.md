# Indicator Math

A high-performance Rust library for financial technical analysis, designed to replicate the logic of `clsAnalysisGenerator.js` with parallel processing capabilities.

## Features

- **Parallel Asset Processing**: Efficiently handles multiple assets concurrently using async tasks.
- **Incremental Updates**: Optimized for real-time tick data processing (O(1) update per tick).
- **Deriv Integration**: Built-in support for fetching OHLC history via Deriv WebSocket/API.
- **Comprehensive Indicators**: Includes EMA, RSI, ATR, Bollinger Bands, ADX, Choppiness Index, and MACD.
- **Status Code Matching**: Automatically maps analysis results to `CandleMasterCode` status descriptions.

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
indicator_math = { path = "../RustLib/indicator_math" } # Or version from crates.io
tokio = { version = "1.0", features = ["full"] }
```

### Basic Example

```rust
use indicator_math::{AnalysisManager, AnalysisOptions, CandleMasterCode};
use std::sync::Arc;

#[tokio::main]
async fn main() {
    // 1. Configure Options
    let options = AnalysisOptions::default();
    
    // 2. Define Master Codes (Status Definitions)
    let master_codes = vec![
        CandleMasterCode { status_code: "1".to_string(), status_desc: "L-DD-E-D".to_string() },
        // ... add more codes
    ];

    // 3. Initialize Manager
    let manager = AnalysisManager::new(options, master_codes);
    
    // 4. Fetch History and Initialize Assets
    let assets = vec!["R_100".to_string(), "R_50".to_string()];
    let ws_url = "wss://ws.binaryws.com/websockets/v3?app_id=1089";
    
    println!("Initializing assets...");
    let results = manager.initialize(ws_url, assets).await;
    
    for (asset, res) in results {
        match res {
            Ok(analysis) => println!("Initialized {}: Close={}", asset, analysis.close),
            Err(e) => eprintln!("Error {}: {}", asset, e),
        }
    }

    // 5. Process Ticks (Real-time)
    // Simulate a tick update
    if let Some((asset, result)) = manager.process_tick("R_100", 123.45, 1700000060) {
        println!("New Candle Closed for {}: Status={}", asset, result.status_code);
    }
    
    // 6. Get All Statuses
    let all_status = manager.get_all_status();
    println!("Current Statuses: {:?}", all_status.len());
}
```

## Publishing to Crates.io

1. Ensure you have an account on [crates.io](https://crates.io/).
2. Login via cargo: `cargo login <token>`.
3. Publish: `cargo publish`.

## License

MIT
