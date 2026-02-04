//! Optimized response types for high-throughput endpoints

use serde::Serialize;
use crate::types::Market;

/// Optimized markets response structure
#[derive(Serialize)]
pub struct MarketsResponse {
    pub markets: Vec<Market>,
    pub count: usize,
    pub total: usize,
    pub limit: usize,
    pub offset: usize,
    pub source: &'static str,
    pub cached: bool,
}

impl MarketsResponse {
    pub fn new(
        markets: Vec<Market>, 
        total: usize, 
        limit: usize, 
        offset: usize,
        source: &'static str
    ) -> Self {
        let count = markets.len();
        Self {
            markets,
            count,
            total,
            limit,
            offset,
            source,
            cached: false,
        }
    }
}