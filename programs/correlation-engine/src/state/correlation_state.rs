use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    program_error::ProgramError,
    pubkey::Pubkey,
};

/// Global correlation engine state
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct CorrelationEngine {
    pub is_initialized: bool,
    pub authority: Pubkey,
    pub update_frequency: u64,          // Slots between correlation updates
    pub min_sample_size: u32,           // Minimum data points for correlation
    pub correlation_threshold: u64,     // Threshold for significant correlation
    pub last_update_slot: u64,
    pub total_verses_tracked: u32,
    pub total_correlations_calculated: u64,
    pub bump: u8,
}

impl CorrelationEngine {
    pub const LEN: usize = 1 + 32 + 8 + 4 + 8 + 8 + 4 + 8 + 1;
    
    pub fn new(authority: Pubkey, bump: u8) -> Self {
        Self {
            is_initialized: true,
            authority,
            update_frequency: 21_600,      // ~6 hours in slots
            min_sample_size: 7,            // 7 days of data
            correlation_threshold: 100_000, // 0.1 in fixed point
            last_update_slot: 0,
            total_verses_tracked: 0,
            total_correlations_calculated: 0,
            bump,
        }
    }
}

/// Configuration for correlation calculations
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct CorrelationConfig {
    pub enable_clustering: bool,
    pub clustering_threshold: u64,      // Correlation threshold for clustering
    pub max_cluster_size: u16,
    pub recalculation_interval: u64,   // Slots between full recalculations
}

impl Default for CorrelationConfig {
    fn default() -> Self {
        Self {
            enable_clustering: true,
            clustering_threshold: 700_000,  // 0.7 correlation
            max_cluster_size: 20,
            recalculation_interval: 216_000, // 1 day
        }
    }
}

/// Market weight for correlation calculations
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct MarketWeight {
    pub market_id: [u8; 16],
    pub weight: u64,        // Fixed point weight
    pub volume_7d: u64,
    pub liquidity: u64,
    pub last_updated: i64,
}

/// Verse tracking for correlation engine
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct VerseTracking {
    pub is_initialized: bool,
    pub verse_id: [u8; 16],
    pub market_weights: Vec<MarketWeight>,
    pub correlation_matrix_pda: Pubkey,
    pub tail_loss_pda: Pubkey,
    pub last_correlation_update: i64,
    pub correlation_version: u32,
    pub bump: u8,
}

impl VerseTracking {
    pub const BASE_LEN: usize = 1 + 16 + 4 + 32 + 32 + 8 + 4 + 1;
    
    pub fn new(verse_id: [u8; 16], bump: u8) -> Self {
        Self {
            is_initialized: true,
            verse_id,
            market_weights: Vec::new(),
            correlation_matrix_pda: Pubkey::default(),
            tail_loss_pda: Pubkey::default(),
            last_correlation_update: 0,
            correlation_version: 0,
            bump,
        }
    }
    
    pub fn add_market(&mut self, weight: MarketWeight) -> Result<(), ProgramError> {
        // Check if market already exists
        if self.market_weights.iter().any(|w| w.market_id == weight.market_id) {
            return Err(ProgramError::AccountAlreadyInitialized);
        }
        
        self.market_weights.push(weight);
        Ok(())
    }
    
    pub fn update_market_weight(
        &mut self,
        market_id: [u8; 16],
        weight: u64,
        volume: u64,
        liquidity: u64,
        timestamp: i64,
    ) -> Result<(), ProgramError> {
        if let Some(market_weight) = self.market_weights.iter_mut()
            .find(|w| w.market_id == market_id) {
            market_weight.weight = weight;
            market_weight.volume_7d = volume;
            market_weight.liquidity = liquidity;
            market_weight.last_updated = timestamp;
            Ok(())
        } else {
            Err(ProgramError::InvalidAccountData)
        }
    }
    
    pub fn remove_market(&mut self, market_id: &[u8; 16]) -> Result<(), ProgramError> {
        self.market_weights.retain(|w| &w.market_id != market_id);
        Ok(())
    }
    
    pub fn calculate_size(max_markets: usize) -> usize {
        Self::BASE_LEN 
            + (max_markets * std::mem::size_of::<MarketWeight>())
            + 100 // Buffer
    }
}

/// Correlation alert for monitoring
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct CorrelationAlert {
    pub verse_id: [u8; 16],
    pub alert_type: AlertType,
    pub correlation_value: u64,
    pub affected_markets: Vec<u16>,
    pub timestamp: i64,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub enum AlertType {
    HighCorrelation,       // Correlation above threshold
    CorrelationCluster,    // Detected correlation cluster
    RapidChange,          // Rapid change in correlation
    InsufficientData,     // Not enough data for reliable calculation
}