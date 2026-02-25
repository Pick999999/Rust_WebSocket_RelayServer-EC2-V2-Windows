use crate::generator::AnalysisGenerator;
use crate::structs::{AnalysisOptions, AnalysisResult, CandleMasterCode};
use crate::deriv_api::fetch_candles;
use dashmap::DashMap;
use std::sync::Arc;
use tokio::task;

pub struct AnalysisManager {
    pub generators: Arc<DashMap<String, AnalysisGenerator>>,
    pub options: AnalysisOptions,
    pub master_codes: Arc<Vec<CandleMasterCode>>,
}

impl AnalysisManager {
    pub fn new(options: AnalysisOptions, master_codes: Vec<CandleMasterCode>) -> Self {
        Self {
            generators: Arc::new(DashMap::new()),
            options,
            master_codes: Arc::new(master_codes),
        }
    }

    pub async fn initialize(&self, ws_url: &str, assets: Vec<String>) -> Vec<(String, Result<AnalysisResult, String>)> {
        let mut tasks = Vec::new();

        for asset in assets {
            let url = ws_url.to_string();
            let asset_clone = asset.clone();
            // Spawn task for each asset
            tasks.push(task::spawn(async move {
                // Fetch history
                match fetch_candles(&url, &asset_clone, 1000).await {
                    Ok(candles) => (asset_clone, Ok(candles)),
                    Err(e) => (asset_clone, Err(e.to_string())),
                }
            }));
        }

        let mut results = Vec::new();

        for task in tasks {
            if let Ok((asset, res)) = task.await {
                match res {
                    Ok(candles) => {
                        let mut gen = AnalysisGenerator::new(self.options.clone(), self.master_codes.clone());
                        let mut last_result = None;
                        
                        // Process existing history
                        for candle in candles {
                            last_result = Some(gen.append_candle(candle));
                             // Note: We might want to optimize this by batch processing if we had a batch function
                             // But append_candle updates state correctly.
                        }
                        
                        // Store generator
                        self.generators.insert(asset.clone(), gen);
                        
                        if let Some(r) = last_result {
                             results.push((asset, Ok(r)));
                        } else {
                             results.push((asset, Err("No candles".to_string())));
                        }
                    }
                    Err(e) => {
                        results.push((asset, Err(e)));
                    }
                }
            }
        }
        
        results
    }

    /// Process a new tick for a specific asset.
    /// Returns Some((Asset, AnalysisResult)) if a candle closed.
    pub fn process_tick(&self, asset: &str, price: f64, time: u64) -> Option<(String, AnalysisResult)> {
        if let Some(mut gen) = self.generators.get_mut(asset) {
            if let Some(result) = gen.append_tick(price, time) {
                // Candle closed
                return Some((asset.to_string(), result));
            }
        }
        None
    }
    
    /// Returns the current known status for an asset (based on latest analysis)
    pub fn get_latest_analysis(&self, asset: &str) -> Option<AnalysisResult> {
        if let Some(gen) = self.generators.get(asset) {
            gen.state.last_analysis.clone()
        } else {
            None
        }
    }

    pub fn get_all_status(&self) -> Vec<(String, AnalysisResult)> {
        self.generators
            .iter()
            .filter_map(|entry| {
                entry.value().state.last_analysis.clone().map(|a| (entry.key().clone(), a))
            })
            .collect()
    }
}
