use crate::structs::Candle;
use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use url::Url;

pub async fn fetch_candles(ws_url: &str, symbol: &str, count: usize) -> Result<Vec<Candle>> {
    let url = Url::parse(ws_url)?;
    let (ws_stream, _) = connect_async(url).await?;
    let (mut write, mut read) = ws_stream.split();

    let req = json!({
        "ticks_history": symbol.to_uppercase(),
        "adjust_start_time": 1,
        "count": count,
        "end": "latest",
        "start": 1,
        "style": "candles",
        "granularity": 60 
    });

    write.send(Message::Text(req.to_string())).await?;

    // Simple robust loop: look for response matching symbol or just first candles response
    while let Some(msg) = read.next().await {
        let msg = msg?;
        match msg {
            Message::Text(text) => {
                let v: Value = serde_json::from_str(&text)?;
                
                if let Some(error) = v.get("error") {
                    return Err(anyhow::anyhow!("Deriv API Error: {:?}", error));
                }

                if let Some(msg_type) = v.get("msg_type") {
                    if msg_type == "candles" {
                        if let Some(candles) = v.get("candles") {
                            // Map manually if structure differs slightly or use serde
                            // Deriv candle: { epoch, open, high, low, close }
                            // Our Candle: { time, open, high, low, close }
                            // We need to map 'epoch' to 'time'
                            
                            if let Some(list) = candles.as_array() {
                                let mut result = Vec::new();
                                for c in list {
                                    let time = c.get("epoch").and_then(|v| v.as_u64()).unwrap_or(0);
                                    let open = c.get("open").and_then(|v| v.as_f64()).unwrap_or(0.0);
                                    let high = c.get("high").and_then(|v| v.as_f64()).unwrap_or(0.0);
                                    let low = c.get("low").and_then(|v| v.as_f64()).unwrap_or(0.0);
                                    let close = c.get("close").and_then(|v| v.as_f64()).unwrap_or(0.0);
                                    
                                    result.push(Candle { time, open, high, low, close });
                                }
                                return Ok(result);
                            }
                        }
                    }
                }
            },
            _ => {}
        }
    }
    
    Err(anyhow::anyhow!("Connection closed without data"))
}
