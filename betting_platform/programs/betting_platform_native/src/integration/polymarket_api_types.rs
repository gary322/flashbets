//! Polymarket API types for integration
//!
//! Defines the types used for Polymarket API integration

use borsh::{BorshDeserialize, BorshSerialize};

/// Internal market data structure
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct InternalMarketData {
    pub market_id: [u8; 32],
    pub title: String,
    pub description: String,
    pub outcomes: Vec<String>,
    pub prices: Vec<u64>,
    pub volume: u64,
    pub liquidity: u64,
    pub resolution_time: i64,
}

/// Polymarket market response
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct PolymarketMarketResponse {
    pub id: String,
    pub title: String,
    pub description: String,
    pub outcomes: Vec<String>,
    pub end_date: i64,
    pub volume: u64,
    pub liquidity: u64,
}

/// Paginated response wrapper
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct PaginatedResponse<T> {
    pub data: Vec<T>,
    pub next_cursor: Option<String>,
    pub has_more: bool,
}

/// API response parser trait
pub trait ApiResponseParser {
    fn parse_market_response(&self, data: &[u8]) -> Result<PolymarketMarketResponse, std::io::Error>;
}

/// Error code mapper trait
pub trait ErrorCodeMapper {
    fn map_error_code(&self, code: u32) -> String;
}

/// Dispute information
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct DisputeInfo {
    pub dispute_id: [u8; 32],
    pub market_id: [u8; 32],
    pub disputer: [u8; 32],
    pub reason: String,
    pub evidence: DisputeEvidence,
    pub status: DisputeStatus,
    pub created_at: i64,
}

/// Dispute status
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum DisputeStatus {
    Pending,
    UnderReview,
    Resolved,
    Rejected,
}

/// Dispute evidence
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct DisputeEvidence {
    pub evidence_type: String,
    pub evidence_url: String,
    pub evidence_hash: [u8; 32],
    pub submitted_at: i64,
}

/// Dispute votes
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct DisputeVotes {
    pub for_votes: u64,
    pub against_votes: u64,
    pub total_weight: u64,
}