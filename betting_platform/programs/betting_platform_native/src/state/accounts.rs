//! Core account structures
//!
//! Primary account types for the betting platform

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    pubkey::Pubkey,
    clock::Clock,
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack, Sealed},
    sysvar::Sysvar,
};

use crate::account_validation::DISCRIMINATOR_SIZE;
use crate::state::chain_accounts::ChainPosition;

/// Discriminators for account types (first 8 bytes)
pub mod discriminators {
    pub const GLOBAL_CONFIG: [u8; 8] = [159, 213, 171, 84, 129, 36, 178, 94];
    pub const VERSE_PDA: [u8; 8] = [231, 118, 64, 193, 87, 12, 45, 203];
    pub const PROPOSAL_PDA: [u8; 8] = [112, 201, 89, 167, 34, 78, 211, 156];
    pub const POSITION: [u8; 8] = [189, 45, 122, 98, 201, 167, 43, 90];
    pub const USER_MAP: [u8; 8] = [67, 89, 234, 112, 178, 23, 198, 45];
    pub const USER_STATS: [u8; 8] = [99, 101, 110, 115, 116, 97, 116, 115];
    pub const METRICS_DASHBOARD: [u8; 8] = [109, 101, 116, 114, 105, 99, 115, 100];
    pub const CU_METRICS_TRACKER: [u8; 8] = [99, 117, 109, 101, 116, 114, 105, 99];
    pub const ORACLE_METRICS_TRACKER: [u8; 8] = [111, 114, 99, 108, 109, 101, 116, 114];
    pub const LIQUIDATION_METRICS_TRACKER: [u8; 8] = [76, 73, 81, 77, 69, 84, 82, 67]; // "LIQMETRC"
    pub const MMT_METRICS_TRACKER: [u8; 8] = [77, 77, 84, 77, 69, 84, 82, 67]; // "MMTMETRC"
    pub const L2_DISTRIBUTION: [u8; 8] = [76, 50, 68, 73, 83, 84, 82, 66]; // "L2DISTRB"
    pub const REENTRANCY_GUARD: [u8; 8] = [82, 69, 69, 78, 71, 85, 65, 82]; // "REENGUAR"
    pub const ACCESS_CONTROL: [u8; 8] = [65, 67, 67, 69, 83, 83, 67, 84]; // "ACCESSCT"
    pub const RATE_LIMITER: [u8; 8] = [82, 65, 84, 69, 76, 73, 77, 84]; // "RATELIMT"
    pub const GLOBAL_RATE_LIMITER: [u8; 8] = [71, 76, 79, 66, 82, 65, 84, 69]; // "GLOBRATE"
    pub const NONCE_MANAGER: [u8; 8] = [78, 79, 78, 67, 69, 77, 71, 82]; // "NONCEMGR"
    pub const SECURITY_MONITOR: [u8; 8] = [83, 69, 67, 77, 79, 78, 73, 84]; // "SECMONIT"
    pub const INVARIANT_CHECKER: [u8; 8] = [73, 78, 86, 65, 82, 67, 72, 75]; // "INVARCHK"
    pub const EMERGENCY_PAUSE: [u8; 8] = [69, 77, 69, 82, 80, 65, 85, 83]; // "EMERPAUS"
    pub const BLOCK_TRADE: [u8; 8] = [66, 76, 79, 67, 75, 84, 82, 68]; // "BLOCKTRD"
    pub const BACKTEST_STATE: [u8; 8] = [66, 65, 67, 75, 84, 69, 83, 84]; // "BACKTEST"
    pub const DEMO_ACCOUNT: [u8; 8] = [68, 69, 77, 79, 65, 67, 67, 84]; // "DEMOACCT"
    pub const RISK_QUIZ_STATE: [u8; 8] = [82, 73, 83, 75, 81, 85, 73, 90]; // "RISKQUIZ"
    pub const SUSTAINABILITY_MODEL: [u8; 8] = [83, 85, 83, 84, 65, 73, 78, 77]; // "SUSTAINM"
    
    // Error handling discriminators
    pub const CHAIN_TRANSACTION: [u8; 8] = [67, 72, 65, 73, 78, 84, 88, 78]; // "CHAINTXN"
    pub const PENDING_TRANSACTION: [u8; 8] = [80, 69, 78, 68, 84, 88, 78, 83]; // "PENDTXNS"
    pub const USER_PENDING_QUEUE: [u8; 8] = [85, 83, 82, 80, 78, 68, 81, 85]; // "USRPNDQU"
    pub const SLOT_REVERT_TRACKER: [u8; 8] = [83, 76, 79, 84, 82, 86, 82, 84]; // "SLOTRVRT"
    pub const RECOVERY_MANAGER: [u8; 8] = [82, 69, 67, 79, 86, 77, 71, 82]; // "RECOVMGR"
}

// Type alias for backwards compatibility
pub type Proposal = ProposalPDA;

/// Global configuration account
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct GlobalConfigPDA {
    /// Account discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    
    /// Account version
    pub version: u32,
    
    /// Migration state
    pub migration_state: crate::state::versioned_accounts::MigrationState,
    
    /// Current epoch number
    pub epoch: u64,
    
    /// Current season number
    pub season: u64,
    
    /// Total vault balance
    pub vault: u128,
    
    /// Total open interest
    pub total_oi: u128,
    
    /// Coverage ratio (vault / total_oi)
    pub coverage: u128,
    
    /// Base fee in basis points
    pub fee_base: u32,
    
    /// Fee slope for dynamic pricing
    pub fee_slope: u32,
    
    /// System halt flag
    pub halt_flag: bool,
    
    /// Genesis slot
    pub genesis_slot: u64,
    
    /// Season start slot
    pub season_start_slot: u64,
    
    /// Season end slot
    pub season_end_slot: u64,
    
    /// MMT total supply
    pub mmt_total_supply: u64,
    
    /// MMT allocation for current season
    pub mmt_current_season: u64,
    
    /// MMT emission rate per slot
    pub mmt_emission_rate: u64,
    
    /// Leverage tiers
    pub leverage_tiers: Vec<LeverageTier>,
    
    /// Minimum order size
    pub min_order_size: u64,
    
    /// Maximum order size
    pub max_order_size: u64,
    
    /// Update authority
    pub update_authority: Pubkey,
    
    /// Primary market ID for system-wide events
    pub primary_market_id: [u8; 32],
    
    /// Fused leverage migration flags (optional field for backward compat)
    pub fused_migration_flags: Option<super::fused_migration::FusedMigrationFlags>,
}

impl GlobalConfigPDA {
    pub fn new() -> Self {
        Self {
            discriminator: discriminators::GLOBAL_CONFIG,
            version: crate::state::versioned_accounts::CURRENT_VERSION,
            migration_state: crate::state::versioned_accounts::MigrationState::Current,
            epoch: 1,
            season: 1,
            vault: 0,
            total_oi: 0,
            coverage: u128::MAX, // Infinite coverage initially
            fee_base: 300,       // 3bp
            fee_slope: 2500,     // 25bp
            halt_flag: false,
            genesis_slot: 0,
            season_start_slot: 0,
            season_end_slot: 0,
            mmt_total_supply: 100_000_000 * 10u64.pow(9),
            mmt_current_season: 10_000_000 * 10u64.pow(9),
            mmt_emission_rate: 0,
            leverage_tiers: vec![
                LeverageTier { n: 1, max: 100 },
                LeverageTier { n: 2, max: 70 },
                LeverageTier { n: 4, max: 25 },
                LeverageTier { n: 8, max: 15 },
                LeverageTier { n: 16, max: 12 },
                LeverageTier { n: 64, max: 10 },
                LeverageTier { n: u32::MAX, max: 5 },
            ],
            min_order_size: 1_000_000, // 1 USDC minimum
            max_order_size: 1_000_000_000_000, // 1M USDC maximum
            update_authority: Pubkey::default(),
            primary_market_id: [0u8; 32], // Set during initialization
            fused_migration_flags: None, // Will be initialized when migration starts
        }
    }
    
    pub fn validate(&self) -> Result<(), ProgramError> {
        if self.discriminator != discriminators::GLOBAL_CONFIG {
            return Err(ProgramError::InvalidAccountData);
        }
        
        if self.leverage_tiers.is_empty() {
            return Err(ProgramError::InvalidAccountData);
        }
        
        Ok(())
    }
    
    /// Try to deserialize from slice
    pub fn try_from_slice(data: &[u8]) -> Result<Self, ProgramError> {
        Self::deserialize(&mut &data[..])
            .map_err(|_| ProgramError::InvalidAccountData)
    }
}

// Implement Pack trait for GlobalConfigPDA
impl Pack for GlobalConfigPDA {
    const LEN: usize = 8 + // discriminator
        4 + 4 + // version, migration_state
        8 + 8 + 16 + 16 + 16 + // epoch, season, vault, total_oi, coverage
        4 + 4 + 1 + // fee_base, fee_slope, halt_flag
        8 + 8 + 8 + // genesis_slot, season_start_slot, season_end_slot
        8 + 8 + 8 + // mmt_total_supply, mmt_current_season, mmt_emission_rate
        (7 * (4 + 1)) + 4 + // leverage_tiers (7 tiers with n:u32 + max:u8) + vec length
        8 + 8 + // min_order_size, max_order_size
        32 + // update_authority
        32 + // primary_market_id
        1 + 200; // fused_migration_flags (Option + reserved space)
    
    fn pack_into_slice(&self, dst: &mut [u8]) {
        let data = self.try_to_vec().unwrap();
        dst[..data.len()].copy_from_slice(&data);
    }
    
    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        Self::deserialize(&mut &src[..])
            .map_err(|_| ProgramError::InvalidAccountData)
    }
}

impl Sealed for GlobalConfigPDA {}

impl IsInitialized for GlobalConfigPDA {
    fn is_initialized(&self) -> bool {
        self.discriminator == discriminators::GLOBAL_CONFIG
    }
}

impl crate::state::versioned_accounts::Versioned for GlobalConfigPDA {
    fn get_version(&self) -> u32 {
        self.version
    }
    
    fn set_version(&mut self, version: u32) {
        self.version = version;
    }
}

/// Leverage tier configuration
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct LeverageTier {
    /// Number of positions at this leverage
    pub n: u32,
    
    /// Maximum leverage allowed
    pub max: u8,
}

/// Quantum state for entangled verses
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct QuantumState {
    /// Entangled verse IDs (max 8 for computation limits)
    pub entangled_verses: Vec<u128>,
    
    /// Superposition weights for each entangled verse (basis points)
    pub superposition_weights: Vec<u16>,
    
    /// Collapse condition (e.g., time-based, outcome-based)
    pub collapse_condition: CollapseCondition,
    
    /// Entanglement strength (0-10000 basis points)
    pub entanglement_strength: u16,
    
    /// Is collapsed
    pub is_collapsed: bool,
    
    /// Collapse timestamp (if collapsed)
    pub collapse_timestamp: Option<i64>,
    
    /// Collapse outcome (if collapsed)
    pub collapse_outcome: Option<u8>,
}

/// Collapse condition for quantum states
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub enum CollapseCondition {
    /// Collapses at specific timestamp
    TimeBasedCollapse { timestamp: i64 },
    
    /// Collapses when any entangled verse resolves
    AnyVerseResolves,
    
    /// Collapses when all entangled verses resolve
    AllVersesResolve,
    
    /// Collapses when a specific verse reaches threshold
    ThresholdCollapse { verse_id: u128, threshold: u64 },
    
    /// Collapses based on external oracle
    OracleTriggered { oracle_id: Pubkey },
}

/// Verse account (hierarchical state management)
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct VersePDA {
    /// Account discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    
    /// Account version
    pub version: u32,
    
    /// Unique verse identifier (keccak hash of normalized title)
    pub verse_id: u128,
    
    /// Parent verse ID (None for root verses)
    pub parent_id: Option<u128>,
    
    /// Merkle root of child verses
    pub children_root: [u8; 32],
    
    /// Number of direct children
    pub child_count: u16,
    
    /// Total number of descendants
    pub total_descendants: u32,
    
    /// Verse status
    pub status: VerseStatus,
    
    /// Depth in hierarchy (max 32)
    pub depth: u8,
    
    /// Last update slot
    pub last_update_slot: u64,
    
    /// Aggregate open interest
    pub total_oi: u64,
    
    /// Weighted average probability
    pub derived_prob: crate::math::U64F64,
    
    /// Correlation factor for tail loss calculation
    pub correlation_factor: crate::math::U64F64,
    
    /// Quantum state for entangled markets
    pub quantum_state: Option<QuantumState>,
    
    /// Markets in this verse
    pub markets: Vec<Pubkey>,
    
    /// Whether cross-verse operations are enabled
    pub cross_verse_enabled: bool,
    
    /// PDA bump seed
    pub bump: u8,
}

impl VersePDA {
    pub fn new(verse_id: u128, parent_id: Option<u128>, bump: u8) -> Self {
        Self {
            discriminator: discriminators::VERSE_PDA,
            version: crate::state::versioned_accounts::CURRENT_VERSION,
            verse_id,
            parent_id,
            children_root: [0u8; 32],
            child_count: 0,
            total_descendants: 0,
            status: VerseStatus::Active,
            depth: 0,
            last_update_slot: 0,
            total_oi: 0,
            derived_prob: crate::math::U64F64::from_num(0),
            correlation_factor: crate::math::U64F64::from_num(0),
            quantum_state: None,
            markets: Vec::new(),
            cross_verse_enabled: false,
            bump,
        }
    }
    
    pub fn validate(&self) -> Result<(), ProgramError> {
        if self.discriminator != discriminators::VERSE_PDA {
            return Err(ProgramError::InvalidAccountData);
        }
        
        if self.depth > 32 {
            return Err(ProgramError::InvalidAccountData);
        }
        
        // Validate quantum state if present
        if let Some(ref quantum) = self.quantum_state {
            if quantum.entangled_verses.len() > 8 {
                return Err(ProgramError::InvalidAccountData);
            }
            
            if quantum.entangled_verses.len() != quantum.superposition_weights.len() {
                return Err(ProgramError::InvalidAccountData);
            }
            
            // Weights should sum to 10000 (100%)
            let weight_sum: u16 = quantum.superposition_weights.iter().sum();
            if weight_sum != 10000 {
                return Err(ProgramError::InvalidAccountData);
            }
        }
        
        Ok(())
    }
    
    /// Create quantum entanglement with other verses
    pub fn create_quantum_entanglement(
        &mut self,
        entangled_verses: Vec<u128>,
        weights: Vec<u16>,
        condition: CollapseCondition,
        strength: u16,
    ) -> Result<(), ProgramError> {
        if entangled_verses.len() > 8 || entangled_verses.len() != weights.len() {
            return Err(ProgramError::InvalidArgument);
        }
        
        let weight_sum: u16 = weights.iter().sum();
        if weight_sum != 10000 {
            return Err(ProgramError::InvalidArgument);
        }
        
        self.quantum_state = Some(QuantumState {
            entangled_verses,
            superposition_weights: weights,
            collapse_condition: condition,
            entanglement_strength: strength,
            is_collapsed: false,
            collapse_timestamp: None,
            collapse_outcome: None,
        });
        
        Ok(())
    }
    
    /// Collapse quantum state
    pub fn collapse_quantum_state(&mut self, outcome: u8, timestamp: i64) -> Result<(), ProgramError> {
        match self.quantum_state.as_mut() {
            Some(quantum) => {
                if quantum.is_collapsed {
                    return Err(ProgramError::InvalidAccountData);
                }
                
                quantum.is_collapsed = true;
                quantum.collapse_timestamp = Some(timestamp);
                quantum.collapse_outcome = Some(outcome);
                
                Ok(())
            }
            None => Err(ProgramError::InvalidAccountData),
        }
    }
    
    /// Calculate quantum-adjusted probability
    pub fn get_quantum_probability(&self) -> crate::math::U64F64 {
        match &self.quantum_state {
            Some(quantum) if !quantum.is_collapsed => {
                // Apply quantum interference based on entanglement strength
                let base_prob = self.derived_prob;
                let strength_factor = crate::math::U64F64::from_num(quantum.entanglement_strength as u64) 
                    / crate::math::U64F64::from_num(10000u64);
                
                // Quantum adjustment: prob * (1 + strength * interference)
                // Use fixed point representation for 0.1 (1/10)
                base_prob * (crate::math::U64F64::from_num(1u64) + strength_factor / crate::math::U64F64::from_num(10u64))
            }
            _ => self.derived_prob,
        }
    }
}

impl crate::state::versioned_accounts::Versioned for VersePDA {
    fn get_version(&self) -> u32 {
        self.version
    }
    
    fn set_version(&mut self, version: u32) {
        self.version = version;
    }
}

/// Verse status
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum VerseStatus {
    Active,
    Halted,     // Coverage < 0.5 or circuit breaker triggered
    Resolved,   // All children resolved
    Migrating,  // During version migration
}

/// Proposal account (prediction market)
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct ProposalPDA {
    /// Account discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    
    /// Account version
    pub version: u32,
    
    /// Unique proposal identifier
    pub proposal_id: [u8; 32],
    
    /// Parent verse ID
    pub verse_id: [u8; 32],
    
    /// Polymarket market ID
    pub market_id: [u8; 32],
    
    /// AMM type for this market
    pub amm_type: AMMType,
    
    /// Number of outcomes (2 for binary, up to 64 for multi)
    pub outcomes: u8,
    
    /// Current prices for each outcome
    pub prices: Vec<u64>,
    
    /// 7-day volumes for each outcome
    pub volumes: Vec<u64>,
    
    /// Liquidity depth
    pub liquidity_depth: u64,
    
    /// Proposal state
    pub state: ProposalState,
    
    /// Settlement slot
    pub settle_slot: u64,
    
    /// Resolution data
    pub resolution: Option<Resolution>,
    
    /// Partial liquidation accumulator
    pub partial_liq_accumulator: u64,
    
    /// Active chain positions using this proposal
    pub chain_positions: Vec<ChainPosition>,
    
    /// Balance for each outcome (used in AMM calculations)
    pub outcome_balances: Vec<u64>,
    
    /// B value parameter for LMSR AMM (scaled by 1_000_000)
    pub b_value: u64,
    
    /// Total liquidity in the market
    pub total_liquidity: u64,
    
    /// Total volume traded
    pub total_volume: u64,
    
    /// Status of the proposal (alias for state)
    pub status: ProposalState,
    
    /// Settlement timestamp
    pub settled_at: Option<i64>,
    
    /// Funding rate state for perpetual markets
    pub funding_state: crate::trading::funding_rate::FundingRateState,
}

impl ProposalPDA {
    pub fn new(proposal_id: [u8; 32], verse_id: [u8; 32], outcomes: u8) -> Self {
        Self {
            discriminator: discriminators::PROPOSAL_PDA,
            version: crate::state::versioned_accounts::CURRENT_VERSION,
            proposal_id,
            verse_id,
            market_id: [0u8; 32],
            amm_type: AMMType::LMSR,
            outcomes,
            prices: vec![500_000; outcomes as usize], // 0.5 initial price
            volumes: vec![0; outcomes as usize],
            liquidity_depth: 0,
            state: ProposalState::Active,
            settle_slot: 0,
            resolution: None,
            partial_liq_accumulator: 0,
            chain_positions: Vec::new(),
            outcome_balances: vec![0; outcomes as usize],
            b_value: 1_000_000, // Default b value of 1.0 (scaled)
            total_liquidity: 0,
            total_volume: 0,
            status: ProposalState::Active,
            settled_at: None,
            funding_state: crate::trading::funding_rate::FundingRateState::new(0),
        }
    }
    
    pub fn validate(&self) -> Result<(), ProgramError> {
        if self.discriminator != discriminators::PROPOSAL_PDA {
            return Err(ProgramError::InvalidAccountData);
        }
        
        if self.outcomes < 2 || self.outcomes > 64 {
            return Err(ProgramError::InvalidAccountData);
        }
        
        if self.prices.len() != self.outcomes as usize {
            return Err(ProgramError::InvalidAccountData);
        }
        
        Ok(())
    }

    pub fn is_active(&self) -> bool {
        self.state == ProposalState::Active
    }
}

// Add type aliases for backward compatibility
impl ProposalPDA {
    pub fn num_outcomes(&self) -> u8 {
        self.outcomes
    }
    
    pub fn current_prices(&self) -> &Vec<u64> {
        &self.prices
    }
}

impl crate::state::versioned_accounts::Versioned for ProposalPDA {
    fn get_version(&self) -> u32 {
        self.version
    }
    
    fn set_version(&mut self, version: u32) {
        self.version = version;
    }
}

/// AMM type
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum AMMType {
    LMSR,    // Binary markets
    PMAMM,   // Multi-outcome (2-64)
    L2AMM,   // Continuous distributions
    Hybrid,  // Hybrid AMM
}

/// Proposal state
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum ProposalState {
    Active,
    Paused,
    Resolved,
}

/// Resolution data
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct Resolution {
    /// Winning outcome
    pub outcome: u8,
    
    /// Resolution timestamp
    pub timestamp: i64,
    
    /// Oracle signature
    pub oracle_signature: [u8; 64],
}

/// Position account
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct Position {
    /// Account discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    
    /// Account version
    pub version: u32,
    
    /// User who owns the position
    pub user: Pubkey,
    
    /// Proposal ID  
    pub proposal_id: u128,
    
    /// Unique position ID
    pub position_id: [u8; 32],
    
    /// Outcome index
    pub outcome: u8,
    
    /// Position size
    pub size: u64,
    
    /// Notional value
    pub notional: u64,
    
    /// Leverage used
    pub leverage: u64,
    
    /// Entry price
    pub entry_price: u64,
    
    /// Liquidation price
    pub liquidation_price: u64,
    
    /// Is long position
    pub is_long: bool,
    
    /// Creation timestamp
    pub created_at: i64,
    
    /// Is position closed
    pub is_closed: bool,
    
    /// Partial liquidation accumulator
    pub partial_liq_accumulator: u64,
    
    /// Verse ID where position was created
    pub verse_id: u128,
    
    /// Margin/collateral for position
    pub margin: u64,
    
    /// Is short position (for compatibility)
    pub is_short: bool,
    
    /// Last mark price for PnL calculation
    pub last_mark_price: u64,
    
    /// Unrealized PnL in USD (signed, can be negative)
    pub unrealized_pnl: i64,
    
    /// Unrealized PnL percentage in basis points (signed, 10000 = 100%)
    pub unrealized_pnl_pct: i64,
    
    /// Cross-margin enabled for this position
    pub cross_margin_enabled: bool,
    
    /// Collateral for the position (updated with funding payments)
    pub collateral: u64,
    
    /// Entry funding index for tracking funding payments
    pub entry_funding_index: Option<crate::math::U64F64>,
}

impl Position {
    pub const LEN: usize = DISCRIMINATOR_SIZE + 4 + 32 + 16 + 32 + 1 + 8 + 8 + 8 + 8 + 8 + 1 + 8 + 1 + 8 + 16 + 8 + 1 + 8 + 8 + 8 + 1;
    
    pub fn new(
        user: Pubkey,
        proposal_id: u128,
        verse_id: u128,
        outcome: u8,
        size: u64,
        leverage: u64,
        entry_price: u64,
        is_long: bool,
        created_at: i64,
    ) -> Self {
        let liquidation_price = if is_long {
            entry_price * (leverage - 1) / leverage
        } else {
            entry_price * (leverage + 1) / leverage
        };
        
        // Generate position ID from user, proposal_id, and outcome
        let position_id = Self::generate_position_id(&user, proposal_id, outcome);
        
        Self {
            discriminator: discriminators::POSITION,
            version: crate::state::versioned_accounts::CURRENT_VERSION,
            user,
            proposal_id,
            position_id,
            outcome,
            size,
            notional: size, // Initially, notional equals size
            leverage,
            entry_price,
            liquidation_price,
            is_long,
            created_at,
            is_closed: false,
            partial_liq_accumulator: 0,
            verse_id,
            margin: size / leverage, // Calculate margin from size and leverage
            is_short: !is_long, // is_short is opposite of is_long
            last_mark_price: entry_price, // Initialize with entry price
            unrealized_pnl: 0, // No PnL at entry
            unrealized_pnl_pct: 0, // 0% PnL at entry
            cross_margin_enabled: false, // Default to isolated margin
            collateral: size / leverage, // Initial collateral equals margin
            entry_funding_index: None, // Will be set when opening position
        }
    }
    
    pub fn validate(&self) -> Result<(), ProgramError> {
        if self.discriminator != discriminators::POSITION {
            return Err(ProgramError::InvalidAccountData);
        }
        
        if self.leverage == 0 || self.leverage > 100 {
            return Err(ProgramError::InvalidAccountData);
        }
        
        Ok(())
    }
    
    pub fn generate_id(trader: &Pubkey, market_id: u128, outcome: u8) -> u128 {
        use solana_program::keccak;
        let mut data = Vec::new();
        data.extend_from_slice(trader.as_ref());
        data.extend_from_slice(&market_id.to_le_bytes());
        data.push(outcome);
        let hash = keccak::hash(&data);
        u128::from_le_bytes(hash.0[..16].try_into().unwrap())
    }
    
    pub fn generate_position_id(trader: &Pubkey, market_id: u128, outcome: u8) -> [u8; 32] {
        use solana_program::keccak;
        let mut data = Vec::new();
        data.extend_from_slice(trader.as_ref());
        data.extend_from_slice(&market_id.to_le_bytes());
        data.push(outcome);
        data.extend_from_slice(&Clock::get().unwrap_or_default().slot.to_le_bytes());
        keccak::hash(&data).to_bytes()
    }
    
    /// Calculate unrealized PnL based on current mark price
    pub fn calculate_unrealized_pnl(&mut self, current_price: u64) -> Result<(), ProgramError> {
        self.last_mark_price = current_price;
        
        // Calculate PnL differently for long vs short
        let price_diff = if self.is_long {
            // Long: profit when price goes up
            current_price as i64 - self.entry_price as i64
        } else {
            // Short: profit when price goes down
            self.entry_price as i64 - current_price as i64
        };
        
        // PnL = price_diff * size / entry_price (normalized to position size)
        self.unrealized_pnl = (price_diff * self.size as i64) / self.entry_price as i64;
        
        // Calculate PnL percentage in basis points
        // pnl_pct = (price_diff / entry_price) * 10000
        self.unrealized_pnl_pct = (price_diff * 10000) / self.entry_price as i64;
        
        Ok(())
    }
    
    /// Get effective leverage adjusted for unrealized PnL
    pub fn get_effective_leverage(&self) -> Result<u64, ProgramError> {
        // effective_leverage = position_leverage × (1 - unrealized_pnl_pct)
        // where unrealized_pnl_pct is in basis points (10000 = 100%)
        
        // Calculate (1 - unrealized_pnl_pct) in basis points
        let adjustment_factor = 10000i64 - self.unrealized_pnl_pct;
        
        // Ensure adjustment factor doesn't go below 10% (minimum 0.1x multiplier)
        let safe_adjustment = adjustment_factor.max(1000);
        
        // Calculate effective leverage
        let effective = (self.leverage as i64 * safe_adjustment) / 10000;
        
        // Ensure minimum leverage of 1x
        Ok(effective.max(1) as u64)
    }
    
    /// Update liquidation price based on current effective leverage
    pub fn update_liquidation_price(&mut self) -> Result<(), ProgramError> {
        let effective_leverage = self.get_effective_leverage()?;
        
        // Recalculate liquidation price with new effective leverage
        self.liquidation_price = if self.is_long {
            // Long positions: liquidate when price drops
            self.entry_price * (effective_leverage - 1) / effective_leverage
        } else {
            // Short positions: liquidate when price rises
            self.entry_price * (effective_leverage + 1) / effective_leverage
        };
        
        Ok(())
    }
    
    /// Check if position should be liquidated at current price
    pub fn should_liquidate(&self, current_price: u64) -> bool {
        if self.is_long {
            current_price <= self.liquidation_price
        } else {
            current_price >= self.liquidation_price
        }
    }
    
    /// Update position with new price (recalculates PnL and liquidation price)
    pub fn update_with_price(&mut self, current_price: u64) -> Result<(), ProgramError> {
        // First calculate new PnL
        self.calculate_unrealized_pnl(current_price)?;
        
        // Then update liquidation price based on new effective leverage
        self.update_liquidation_price()?;
        
        Ok(())
    }
    
    /// Get margin ratio at current price
    pub fn get_margin_ratio(&self, current_price: u64) -> Result<crate::math::U64F64, ProgramError> {
        use crate::math::U64F64;
        
        // Calculate current notional value
        let notional = (self.size as u128 * current_price as u128 / 1_000_000) as u64;
        
        if notional == 0 {
            return Err(ProgramError::InvalidAccountData);
        }
        
        // Margin ratio = margin / notional
        Ok(U64F64::from_num(self.margin) / U64F64::from_num(notional))
    }
}

impl crate::state::versioned_accounts::Versioned for Position {
    fn get_version(&self) -> u32 {
        self.version
    }
    
    fn set_version(&mut self, version: u32) {
        self.version = version;
    }
}

/// User map account (tracks user positions)
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct UserMap {
    /// Account discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    
    /// Account version
    pub version: u32,
    
    /// User public key
    pub user: Pubkey,
    
    /// Number of active positions
    pub position_count: u32,
    
    /// Position IDs (proposal_id)
    pub position_ids: Vec<u128>,
    
    /// Position pubkeys
    pub positions: Vec<Pubkey>,
    
    /// User's 7-day trading volume (for fee discounts)
    pub total_volume_7d: u64,
    
    /// Last volume update timestamp
    pub last_volume_update: i64,
}

impl UserMap {
    pub const LEN: usize = 8 + 4 + 32 + 4 + 4 + (16 * 32) + (32 * 32) + 8 + 8; // discriminator + version + user + count + vec_len + position_ids + positions + volume + timestamp
    
    pub fn new(user: Pubkey) -> Self {
        Self {
            discriminator: discriminators::USER_MAP,
            version: crate::state::versioned_accounts::CURRENT_VERSION,
            user,
            position_count: 0,
            position_ids: Vec::with_capacity(32),
            positions: Vec::with_capacity(32),
            total_volume_7d: 0,
            last_volume_update: 0,
        }
    }
    
    pub fn add_position(&mut self, proposal_id: u128) -> Result<(), ProgramError> {
        if self.position_count >= 32 {
            return Err(ProgramError::AccountDataTooSmall);
        }
        
        if !self.position_ids.contains(&proposal_id) {
            self.position_ids.push(proposal_id);
            self.position_count += 1;
        }
        
        Ok(())
    }
    
    pub fn remove_position(&mut self, proposal_id: u128) -> Result<(), ProgramError> {
        if let Some(index) = self.position_ids.iter().position(|&id| id == proposal_id) {
            self.position_ids.remove(index);
            self.position_count -= 1;
            Ok(())
        } else {
            Err(ProgramError::InvalidAccountData)
        }
    }
    
    pub fn validate(&self) -> Result<(), ProgramError> {
        if self.discriminator != discriminators::USER_MAP {
            return Err(ProgramError::InvalidAccountData);
        }
        
        if self.position_count as usize != self.position_ids.len() {
            return Err(ProgramError::InvalidAccountData);
        }
        
        Ok(())
    }
    
    pub fn active_positions(&self) -> u32 {
        self.position_count
    }
}

// Implement Pack trait for UserMap
impl solana_program::program_pack::Pack for UserMap {
    const LEN: usize = Self::LEN;
    
    fn pack_into_slice(&self, dst: &mut [u8]) {
        let data = self.try_to_vec().unwrap();
        dst[..data.len()].copy_from_slice(&data);
    }
    
    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        Self::deserialize(&mut &src[..])
            .map_err(|_| ProgramError::InvalidAccountData)
    }
}

impl solana_program::program_pack::Sealed for UserMap {}

impl solana_program::program_pack::IsInitialized for UserMap {
    fn is_initialized(&self) -> bool {
        self.discriminator == discriminators::USER_MAP
    }
}

impl crate::state::versioned_accounts::Versioned for UserMap {
    fn get_version(&self) -> u32 {
        self.version
    }
    
    fn set_version(&mut self, version: u32) {
        self.version = version;
    }
}

/// User statistics account
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct UserStatsPDA {
    /// Account discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    
    /// Account version
    pub version: u32,
    
    /// User pubkey
    pub user: Pubkey,
    
    /// Total positions opened
    pub total_positions: u64,
    
    /// Total volume traded
    pub total_volume: u64,
    
    /// Total fees paid
    pub total_fees: u64,
    
    /// Win rate (basis points)
    pub win_rate_bps: u16,
    
    /// Number of liquidations
    pub liquidation_count: u32,
    
    /// Last activity timestamp
    pub last_activity: i64,
}

impl UserStatsPDA {
    pub fn new(user: Pubkey) -> Self {
        Self {
            discriminator: discriminators::USER_STATS,
            version: crate::state::versioned_accounts::CURRENT_VERSION,
            user,
            total_positions: 0,
            total_volume: 0,
            total_fees: 0,
            win_rate_bps: 0,
            liquidation_count: 0,
            last_activity: 0,
        }
    }
    
    pub fn validate(&self) -> Result<(), ProgramError> {
        if self.discriminator != discriminators::USER_STATS {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(())
    }
}

impl crate::state::versioned_accounts::Versioned for UserStatsPDA {
    fn get_version(&self) -> u32 {
        self.version
    }
    
    fn set_version(&mut self, version: u32) {
        self.version = version;
    }
}