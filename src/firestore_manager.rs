use firestore::*;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Custom error types for Firestore operations
#[derive(Error, Debug)]
pub enum FirestoreError {
    #[error("Firestore connection error: {0}")]
    ConnectionError(String),

    #[error("Operation failed: {0}")]
    OperationFailed(String),
}

/// Trade Record structure for saving to Firestore
/// Matches the Active Trades table columns in dashboard_old
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeRecord {
    /// ลำดับ (Order/Index)
    pub order_no: u32,
    /// Contract ID
    pub contract_id: String,
    /// Symbol (e.g., R_100, frxEURUSD)
    pub symbol: String,
    /// ประเภท (Type: CALL/PUT)
    pub trade_type: String,
    /// ราคาซื้อ (Buy Price)
    pub buy_price: f64,
    /// Payout
    pub payout: f64,
    /// กำไร/ขาดทุน (Profit/Loss)
    pub profit_loss: f64,
    /// เวลาซื้อ (Buy Time) - Unix timestamp
    pub buy_time: u64,
    /// เวลาหมดอายุ (Expiry Time) - Unix timestamp
    pub expiry_time: u64,
    /// เวลาที่เหลือ (Time Remaining in seconds)
    pub time_remaining: i64,
    /// Min Profit recorded during trade
    pub min_profit: f64,
    /// Max Profit recorded during trade
    pub max_profit: f64,
    /// Trade Result (win/loss/sold)
    pub status: String,
    /// Entry Spot Price
    pub entry_spot: f64,
    /// Exit/Current Spot Price
    pub exit_spot: f64,
    /// Lot Number
    pub lot_no: u32,
    /// Trade Number in Lot
    pub trade_no_in_lot: u32,
    /// Date of trade (YYYY-MM-DD)
    pub trade_date: String,
    /// Time of record creation (ISO format)
    pub created_at: String,
}

/// Scan Record structure for saving market scan data to Firestore
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanRecord {
    /// Scan timestamp (ISO format)
    pub scan_time: String,
    /// Candle timeframe in seconds
    pub timeframe: String,
    /// Indicator period
    pub period: String,
    /// Asset symbol
    pub symbol: String,
    /// Current price
    pub price: f64,
    /// Choppiness Index
    pub ci: f64,
    /// ADX value
    pub adx: f64,
    /// Trend Score (ADX + (100 - CI))
    pub score: f64,
    /// Is current candle bullish
    pub is_bullish: bool,
    /// Last 10 candle colors (comma separated: up,down,up...)
    pub recent_candles: String,
    /// Rank (1 = best, higher = worse)
    pub rank: u32,
}

/// Main Firestore Manager struct
pub struct FirestoreManager {
    db: FirestoreDb,
}

impl FirestoreManager {
    /// Create a new Firestore Manager instance
    ///
    /// # Arguments
    /// * `project_id` - Your Google Cloud Project ID
    pub async fn new(project_id: &str) -> Result<Self, FirestoreError> {
        let db = FirestoreDb::new(project_id)
            .await
            .map_err(|e| FirestoreError::ConnectionError(e.to_string()))?;

        Ok(Self { db })
    }

    /// Add a new trade record to the collection
    /// If the collection doesn't exist, Firestore will create it automatically
    ///
    /// # Arguments
    /// * `collection_name` - Name of the collection (e.g., "trade_records")
    /// * `record` - The TradeRecord to save
    ///
    /// # Returns
    /// The document ID (auto-generated)
    pub async fn save_trade_record(
        &self,
        collection_name: &str,
        record: &TradeRecord,
    ) -> Result<String, FirestoreError> {
        // Use contract_id as document_id for easy lookup
        let doc_id = format!("{}_{}", record.trade_date, record.contract_id);

        self.db
            .fluent()
            .insert()
            .into(collection_name)
            .document_id(&doc_id)
            .object(record)
            .execute::<()>()
            .await
            .map_err(|e| FirestoreError::OperationFailed(e.to_string()))?;

        println!("🔥 Trade record saved to Firestore: {}", doc_id);
        Ok(doc_id)
    }

    /// Check if Firestore connection is healthy
    pub async fn health_check(&self) -> bool {
        // Try to list collections as a health check
        match self
            .db
            .list_collection_ids(FirestoreListCollectionIdsParams::new())
            .await
        {
            Ok(_) => true,
            Err(_) => false,
        }
    }

    /// Save a scan record to the collection
    /// If the collection doesn't exist, Firestore will create it automatically
    pub async fn save_scan_record(
        &self,
        collection_name: &str,
        record: &ScanRecord,
    ) -> Result<String, FirestoreError> {
        // Generate unique doc ID with timestamp and symbol
        let doc_id = format!(
            "{}_{}",
            record.scan_time.replace(":", "-").replace(".", "-"),
            record.symbol
        );

        self.db
            .fluent()
            .insert()
            .into(collection_name)
            .document_id(&doc_id)
            .object(record)
            .execute::<()>()
            .await
            .map_err(|e| FirestoreError::OperationFailed(e.to_string()))?;

        println!("🔥 Scan record saved to Firestore: {}", doc_id);
        Ok(doc_id)
    }
}

/// Global Firestore instance wrapper
pub struct GlobalFirestore {
    manager: Option<FirestoreManager>,
}

impl GlobalFirestore {
    pub fn new() -> Self {
        Self { manager: None }
    }

    pub async fn initialize(&mut self, project_id: &str) -> Result<(), FirestoreError> {
        match FirestoreManager::new(project_id).await {
            Ok(manager) => {
                println!("✅ Firestore connected successfully");
                self.manager = Some(manager);
                Ok(())
            }
            Err(e) => {
                println!("⚠️ Firestore connection failed: {}", e);
                Err(e)
            }
        }
    }

    pub async fn save_trade(&self, record: &TradeRecord) -> Result<String, FirestoreError> {
        match &self.manager {
            Some(manager) => manager.save_trade_record("trade_records", record).await,
            None => Err(FirestoreError::OperationFailed(
                "Firestore not initialized".to_string(),
            )),
        }
    }

    pub async fn save_scan(&self, record: &ScanRecord) -> Result<String, FirestoreError> {
        match &self.manager {
            Some(manager) => manager.save_scan_record("market_scans", record).await,
            None => Err(FirestoreError::OperationFailed(
                "Firestore not initialized".to_string(),
            )),
        }
    }
}
