# 🚀 Deriv Trading Relay Server (Rust)

## 1. System Architecture

### Overview

ระบบถูกออกแบบเป็น **Relay Server** ทำหน้าที่เป็นตัวกลางระหว่าง Browser (Frontend) และ Deriv WebSocket API

```
[ Browser / Dashboard ]
          │
          │ WebSocket (/ws)
          ▼
[ Rust Relay Server (Axum) ]
          │
          │ WebSocket (tokio-tungstenite)
          ▼
[ Deriv WebSocket API ]
```

---

## 2. Component Breakdown

### 2.1 Axum WebSocket Server

หน้าที่:

* รับการเชื่อมต่อจาก Browser
* ส่งข้อมูล Real-time ผ่าน Broadcast Channel
* รับคำสั่งควบคุมระบบ

Endpoint:

* `GET /ws`

---

### 2.2 Broadcast System

ใช้ `tokio::sync::broadcast`

```
Deriv Data
   │
   ▼
broadcast::Sender
   │
   ├── Browser #1
   ├── Browser #2
   └── Browser #N
```

---

### 2.3 Deriv Connector Task

ทำงานใน background task แยกจาก WebSocket Server

หน้าที่:

* เชื่อมต่อ Deriv API
* Subscribe Tick / Candle
* คำนวณ Indicator
* วิเคราะห์ Trend
* ดำเนินการ Trade

---

## 3. Data Flow

### 3.1 Connection Flow

```
Browser
  │ START_DERIV
  ▼
Relay Server
  │ spawn task
  ▼
Deriv Connector
```

หากมีการ START ใหม่:

* ปิด connection เก่า
* unsubscribe ทุก subscription

---

### 3.2 Market Data Flow

```
Deriv Tick / Candle
        │
        ▼
parse_flexible()
        │
        ├─ Candle
        ├─ EMA Calculation
        └─ EMA Analysis
        │
        ▼
Broadcast → Browser
```

---

## 4. Indicator & Analysis Engine

### 4.1 Candle Buffer

* เก็บข้อมูลแท่งเทียนสูงสุด 200 แท่ง
* Normalize เป็นระดับ 1 นาที

### 4.2 Supported Indicators

* EMA
* SMA
* WMA
* HMA
* EHMA

### 4.3 EMA Analysis

ใช้ `analyze_ema()` วิเคราะห์:

* EMA Slope
* EMA Turn
* EMA Cross
* Position (Above / Below)
* Distance Metrics

ส่งผลลัพธ์ทุก 2 วินาที

---

## 5. Trading Engine

### 5.1 Trade Trigger

* วินาทีที่ 0–2 ของแต่ละนาที
* จำกัด 1 trade ต่อ 1 นาที

### 5.2 Trade Mode

| Mode | Behavior                |
| ---- | ----------------------- |
| idle | ไม่เทรด                 |
| call | CALL ทุกนาที            |
| put  | PUT ทุกนาที             |
| auto | ใช้ Action จาก Analysis |

---

### 5.3 Money Management

#### Fix Mode

* ใช้ stake เดิมตลอด

#### Martingale Mode

* เพิ่ม stake เมื่อแพ้
* reset เมื่อชนะ

---

## 6. Command Protocol (Browser → Server)

| Command       | Description                   |
| ------------- | ----------------------------- |
| START_DERIV   | เริ่มเชื่อมต่อ Deriv          |
| UPDATE_MODE   | เปลี่ยน trade mode            |
| UPDATE_PARAMS | เปลี่ยน duration / money mode |
| SELL          | ปิด contract ทันที            |

---

## 7. Message Protocol (Server → Browser)

| Type          | Payload           |
| ------------- | ----------------- |
| candle        | OHLC Data         |
| ema_data      | EMA Lines         |
| analysis_data | Trend Analysis    |
| balance       | Account Balance   |
| trade_opened  | Trade Open Info   |
| trade_update  | Live Trade Status |
| trade_result  | Final Result      |

---

## 8. Fault Tolerance

* Detect Deriv API Error
* Auto stop connection on error
* Safe unsubscribe on reconnect

---

## 9. Design Goals

* Low Latency
* Real-time Streaming
* Decoupled Frontend / Backend
* Extensible Indicator Engine

---

## 10. Future Extension

* Multi-asset support
* AI / ML decision engine
* Backtesting module
* Persistent trade history

---

✅ **Production-ready architecture for real-time trading systems**
"# Rust_Websocket_WIndows10" 
"# Rust_WebSocket_RelayServer-EC2-V2-Windows" 
