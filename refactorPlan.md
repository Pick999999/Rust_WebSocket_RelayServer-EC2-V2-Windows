# 🔧 Refactor Plan: `src/main.rs`

> **สถานะปัจจุบัน:** `main.rs` มี **4,200+ บรรทัด** — เป็น monolith ที่ยากต่อการ maintain  
> **เป้าหมาย:** แยกเป็น modules ที่ชัดเจน แต่ละไฟล์ไม่เกิน 500-800 บรรทัด  
> **วันที่สร้างแผน:** 2026-02-25

---

## 📁 โครงสร้างเป้าหมาย

```
src/
├── main.rs              (~300 lines — main, routes, handle_socket dispatch)
├── models.rs            (~300 lines — all structs & enums)
├── commands.rs          (~200 lines — command parsing & dispatch)
├── config.rs            (~100 lines — config loading & caching)
├── deriv_common.rs      (~300 lines — shared Deriv API utilities)
├── deriv_single.rs      (~500 lines — connect_to_deriv)
├── auto_trade.rs        (~800 lines — auto_multi_trade)
└── firestore_manager.rs (existing — ไม่ต้องแก้)
```

---

## 📋 รายละเอียดแต่ละ Task

### Task 1: แยก Structs → `models.rs`
**Priority: 🔴 สูง | Effort: ต่ำ**

**ย้าย structs ทั้งหมดจากต้นไฟล์ `main.rs` (~line 1–640):**
- `Candle` (main.rs candle, ไม่ใช่ indicator_math)
- `ServerTime`
- `TradeOpened`, `TradeResult`, `TradeUpdate`
- `BalanceMessage`
- `EmaPoint`, `EmaData`
- `AnalysisData`
- `LotStatus`
- `TradeSignalEntry`
- `AssetSignalResult`, `MultiAnalysisMessage`
- `AutoTradeStatusMessage`
- `CompactAnalysis`, `HistoricalAnalysis`
- `BroadcastMessage` (enum)
- `ClientCommand`
- `AppState`
- `IndicatorConfig`, `IndicatorsSettings`, `TradingSettings`

**สิ่งที่ต้องทำ:**
```rust
// models.rs
pub use ต่างๆ

// main.rs
mod models;
use models::*;
```

**ผลลัพธ์:** `main.rs` ลดลง ~600 บรรทัดทันที

---

### Task 2: `LotStatus` + Default Trait
**Priority: 🔴 สูง | Effort: ต่ำ**

**ปัญหา:** เพิ่ม field ใหม่ใน `LotStatus` → ต้องแก้ 9+ จุดทั่วไฟล์

**แก้ไข:** ใช้ `Default` trait + struct update syntax:
```rust
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

impl Default for LotStatus {
    fn default() -> Self {
        Self {
            msg_type: "lot_status".to_string(),
            grand_profit: 0.0,
            win_count: 0,
            target_profit: 10.0,
            target_win: 5,
            lot_active: false,
            balance: 0.0,
        }
    }
}

// Usage — เพิ่ม field ใหม่ไม่ต้องแก้ทุกจุด:
LotStatus {
    grand_profit,
    win_count,
    lot_active,
    balance,
    ..Default::default()
}
```

**ผลลัพธ์:** เพิ่ม field ใหม่ในอนาคต แก้แค่ 1 จุด (Default impl)

---

### Task 3: แยก `deriv_common.rs` — Shared Utilities
**Priority: 🟡 กลาง | Effort: กลาง**

**โค้ดที่ซ้ำกันระหว่าง `connect_to_deriv` และ `auto_multi_trade`:**

| Function | คำอธิบาย | ซ้ำกี่ที่ |
|----------|---------|----------|
| `parse_flexible()` | Parse OHLC from Deriv JSON | 2 |
| `build_candle_master_codes()` | Load CandleMasterCode table | 2 |
| Deriv authorize flow | Send authorize + parse response | 2 |
| Subscribe ticks/candles | `{ "ticks": asset }` / `{ "ticks_history": ... }` | 2 |
| Parse candle history | Loop ผ่าน `json["candles"]` array | 2 |
| Build EmaData message | สร้าง short/medium/long EMA arrays | 2 |
| Build HistoricalAnalysis | V2AnalysisGenerator → CompactAnalysis | 2 |
| Action determination | `get_action_by_simple/cut_type` + slope fallback | 2 |
| Signal matching | Match status_code vs tradeSignal.json signals | 2 |

**สิ่งที่ย้ายไป `deriv_common.rs`:**
```rust
pub fn connect_deriv_ws(asset: &str) -> Result<WebSocket>
pub fn authorize(ws: &mut WebSocket, token: &str) -> Result<f64> // returns balance
pub fn subscribe_candles(ws: &mut WebSocket, asset: &str) -> Result<()>
pub fn parse_ohlc(json: &Value) -> Result<Candle>
pub fn parse_candle_history(json: &Value) -> Vec<IndicatorCandle>
pub fn build_ema_data(candles: &[IndicatorCandle], config: &IndicatorsSettings) -> EmaData
pub fn build_historical_analysis(candles: &[IndicatorCandle], asset: &str, signals: &[TradeSignalEntry]) -> HistoricalAnalysis
pub fn determine_action(analysis: &[AnalysisResultOld], index: usize, config: &IndicatorsSettings) -> (String, String)
pub fn match_signal(status_code: &str, entry: &TradeSignalEntry) -> String // "call"/"put"/"idle"
```

**ผลลัพธ์:** ลดโค้ดซ้ำ ~40-50%, แก้ bug ที่เดียวแก้ได้ทั้งสองทาง

---

### Task 4: แยก `auto_trade.rs` — Auto Multi-Trade Logic
**Priority: 🟡 กลาง | Effort: สูง**

**ย้าย function `auto_multi_trade` (~line 2200–3700) ทั้งหมด:**
- รวม helper functions ที่ใช้เฉพาะ auto_trade
- Trade execution logic (buy/sell via Deriv API)
- Lot management (profit tracking, win counting, stop conditions)
- Martingale logic
- `proposal_open_contract` polling
- Trade result processing

**Signature:**
```rust
pub async fn auto_multi_trade(
    assets: Vec<String>,
    config: AutoTradeConfig,  // new struct รวม trade params
    tx: broadcast::Sender<BroadcastMessage>,
    mut cmd_rx: mpsc::Receiver<String>,
    firestore: Arc<tokio::sync::Mutex<GlobalFirestore>>,
)
```

**ผลลัพธ์:** `main.rs` ลดลง ~1,500 บรรทัด

---

### Task 5: แยก `deriv_single.rs` — Single Asset Viewer
**Priority: 🟡 กลาง | Effort: กลาง**

**ย้าย function `connect_to_deriv` (~line 1089–2100):**
- Single asset chart data provider
- Real-time tick/candle streaming
- EMA calculation & broadcast
- Historical analysis generation
- Single-trade execution (when in non-idle mode)

**Signature:**
```rust
pub async fn connect_to_deriv(
    config: DerivConfig,  // asset, token, trade_mode, etc.
    tx: broadcast::Sender<BroadcastMessage>,
    mut cmd_rx: mpsc::Receiver<String>,
    firestore: Arc<tokio::sync::Mutex<GlobalFirestore>>,
)
```

**ผลลัพธ์:** `main.rs` ลดลง ~1,000 บรรทัด

---

### Task 6: แยก `commands.rs` — Command Handling
**Priority: 🟢 ต่ำ | Effort: ต่ำ**

**แปลง `handle_socket` จาก if/else chain เป็น match + functions:**
```rust
// commands.rs
pub async fn handle_command(
    req: ClientCommand,
    state: Arc<AppState>,
) -> Result<()> {
    match req.command.as_str() {
        "START_DERIV" => handle_start_deriv(req, state).await,
        "START_AUTO_TRADE" => handle_start_auto_trade(req, state).await,
        "UPDATE_MODE" => handle_update_mode(req, state).await,
        "UPDATE_PARAMS" => handle_update_params(req, state).await,
        "SYNC_STATUS" => handle_sync_status(state).await,
        "STOP_AUTO_TRADE" => handle_stop_auto_trade(state).await,
        "SELL" => handle_sell(req, state).await,
        _ => { println!("❓ Unknown command: {}", req.command); Ok(()) }
    }
}
```

---

### Task 7: แยก `config.rs` — Config Loading
**Priority: 🟢 ต่ำ | Effort: ต่ำ**

**ปัญหา:** `tradeSignal.json` และ `indicatorConfig.json` ถูก load ซ้ำหลายครั้ง

**แก้ไข:**
```rust
// config.rs
pub struct AppConfig {
    pub indicator: IndicatorConfig,
    pub signals: Vec<TradeSignalEntry>,
}

impl AppConfig {
    pub fn load() -> Self { ... }
    pub fn reload(&mut self) { ... } // Hot reload
}

// เก็บใน AppState:
struct AppState {
    tx: broadcast::Sender<BroadcastMessage>,
    config: Arc<RwLock<AppConfig>>,  // Shared & hot-reloadable
    // ...
}
```

---

## ⚡ ลำดับการทำงาน (แนะนำ)

| Step | Task | เหตุผล |
|------|------|--------|
| 1 | Task 1: `models.rs` | ง่ายสุด เห็นผลทันที ไม่กระทบ logic |
| 2 | Task 2: `LotStatus` Default | แก้ปัญหา field เพิ่มยาก |
| 3 | Task 3: `deriv_common.rs` | ต้องทำก่อน Task 4-5 เพราะเป็น shared code |
| 4 | Task 5: `deriv_single.rs` | เล็กกว่า auto_trade ทำก่อนได้ |
| 5 | Task 4: `auto_trade.rs` | ใหญ่สุด ทำหลังสุด |
| 6 | Task 6-7: commands + config | Polish ขั้นสุดท้าย |

---

## ⚠️ ข้อควรระวัง

1. **ทำทีละ Task** — อย่า refactor หลาย Task พร้อมกัน เพราะจะ merge conflict
2. **Test หลังทุก Task** — `cargo check` + `cargo run` + ทดสอบ browser ทุกครั้ง
3. **ไม่เปลี่ยน behavior** — Refactor คือเปลี่ยนโครงสร้าง ไม่เปลี่ยน logic
4. **`pub(crate)` vs `pub`** — ใช้ `pub(crate)` สำหรับ internal modules
5. **Circular dependencies** — ระวัง models ที่ reference กัน ให้อยู่ไฟล์เดียวกัน
