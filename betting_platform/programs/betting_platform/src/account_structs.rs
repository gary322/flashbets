use anchor_lang::prelude::*;
use anchor_spl::token::Token;
use crate::errors::ErrorCode;

// Fixed-point math types for precision
pub type U64F64 = u64;  // Fixed-point 64.64
pub type U128F128 = u128; // Fixed-point 128.128

// Constants
pub const PRECISION: u128 = 1_000_000_000; // 10^9 for coverage calculations
pub const PRICE_PRECISION: u64 = 1_000_000_000; // 10^9 for price calculations
pub const HEALTH_PRECISION: u64 = 10_000; // Basis points
pub const HEALTH_WARNING_THRESHOLD: u64 = 1_000; // 10% drop triggers warning
pub const MINIMUM_COVERAGE: u128 = 500_000_000; // 0.5 in fixed point
pub const VOLATILITY_PRECISION: u64 = 10_000; // Basis points
pub const MIN_LIQUIDATION_BUFFER: u64 = 200; // 2% minimum buffer

// ============= Core Account Structures =============

#[account]
pub struct VersePDA {
    pub verse_id: [u8; 32],          // 32 bytes - Keccak hash of normalized title
    pub parent_id: Option<[u8; 32]>, // 33 bytes - Single parent (trees, not DAGs)
    pub status: VerseStatus,         // 1 byte - Active, Resolved, Halted
    pub children_root: [u8; 32],     // 32 bytes - Merkle root of children
    pub depth: u8,                   // 1 byte - Max 32 levels deep
    pub child_count: u16,            // 2 bytes - Number of direct children
    pub total_oi: u64,               // 8 bytes - Aggregate open interest
    pub derived_prob: u64,           // 8 bytes - Weighted average probability (U64F64)
    pub last_update_slot: u64,       // 8 bytes - For cache invalidation
    pub correlation_factor: u64,     // 8 bytes - For tail loss calculation (U64F64)
}

impl VersePDA {
    pub const LEN: usize = 8 + // discriminator
        32 + // verse_id
        33 + // parent_id (Option<[u8; 32]>)
        1 + // status
        32 + // children_root
        1 + // depth
        2 + // child_count
        8 + // total_oi
        8 + // derived_prob
        8 + // last_update_slot
        8; // correlation_factor
        // Total: 141 bytes (aligns with 83 bytes in spec after compression)
    
    // Helper function to convert verse_id to u128 for backwards compatibility
    pub fn verse_id_as_u128(&self) -> u128 {
        // Take first 16 bytes of the 32-byte array
        let mut bytes = [0u8; 16];
        bytes.copy_from_slice(&self.verse_id[0..16]);
        u128::from_le_bytes(bytes)
    }

    pub fn validate_hierarchy(&self) -> Result<()> {
        require!(
            self.depth <= 32,
            ErrorCode::MaxDepthExceeded
        );

        if let Some(parent_id) = self.parent_id {
            require!(
                parent_id != self.verse_id,
                ErrorCode::CircularHierarchy
            );
        }

        Ok(())
    }

    pub fn can_trade(&self) -> Result<()> {
        require!(
            self.status == VerseStatus::Active,
            ErrorCode::VerseNotActive
        );

        Ok(())
    }
    
    pub fn num_outcomes(&self) -> u8 {
        // For now, return a default. This should be stored in the VersePDA
        // or calculated based on the verse structure
        2
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum VerseStatus {
    Active,
    Halted,      // Coverage < 0.5 or circuit breaker triggered
    Resolved,    // All children resolved
    Migrating,   // During version migration
    Disputed,    // Resolution is being disputed
}

#[account]
pub struct ProposalPDA {
    pub proposal_id: [u8; 32],       // 32 bytes - Unique proposal ID
    pub verse_id: [u8; 32],          // 32 bytes - Parent verse
    pub market_id: [u8; 32],         // 32 bytes - Polymarket market ID
    pub amm_type: AMMType,           // 1 byte - LMSR, PM-AMM, or L2
    pub outcomes: Vec<Outcome>,      // Variable - Binary or multi-outcome
    pub prices: Vec<u64>,            // Variable - Current prices from Polymarket (U64F64)
    pub volumes: Vec<u64>,           // Variable - 7-day volumes for weighting (U64F64)
    pub liquidity_depth: u64,        // 8 bytes - For routing decisions
    pub state: ProposalState,        // 1 byte - Active, Paused, Resolved
    pub settle_slot: u64,            // 8 bytes - Resolution time
    pub resolution: Option<Resolution>, // Variable - Resolution data
    pub chain_positions: Vec<ChainPosition>, // Variable - Active chains
    pub partial_liq_accumulator: u64, // 8 bytes - Tracks partial liquidations
}

impl ProposalPDA {
    pub const BASE_LEN: usize = 8 + // discriminator
        32 + // proposal_id
        32 + // verse_id
        32 + // market_id
        1 + // amm_type
        1 + // state
        8 + // liquidity_depth
        8 + // settle_slot
        8 + // partial_liq_accumulator
        1; // Option discriminator for resolution

    pub fn space(outcomes: usize, chains: usize) -> usize {
        Self::BASE_LEN +
        4 + (outcomes * 1) +  // outcomes vec
        4 + (outcomes * 8) +  // prices vec
        4 + (outcomes * 8) +  // volumes vec
        64 + // resolution struct if Some
        4 + (chains * 112)    // chain_positions vec (32+8+8+8 per position)
    }
    
    // Helper function to convert proposal_id to u128 for backwards compatibility
    pub fn id(&self) -> u128 {
        // Take first 16 bytes of the 32-byte array
        let mut bytes = [0u8; 16];
        bytes.copy_from_slice(&self.proposal_id[0..16]);
        u128::from_le_bytes(bytes)
    }
    
    // Helper function to get outcome count
    pub fn outcome_count(&self) -> u8 {
        self.outcomes.len() as u8
    }
    
    // Helper function to get q_values for LMSR compatibility
    pub fn q_values(&self) -> Vec<i64> {
        // Convert prices to q_values for LMSR
        // In LMSR: price = exp(q/b) / sum(exp(q_i/b))
        // For simplicity, use log of prices as q_values
        self.prices.iter().map(|&p| p as i64).collect()
    }
    
    // Helper function to get liquidity parameter
    pub fn liquidity_parameter(&self) -> u64 {
        // Use liquidity_depth as the liquidity parameter
        self.liquidity_depth
    }
    
    // Helper function to get created_at timestamp
    pub fn created_at(&self) -> i64 {
        // Use current timestamp - derived from settle_slot
        (self.settle_slot as i64 - 432_000) * 400 / 1000 // Approx conversion from slots to unix timestamp
    }
    
    // Helper function to get expires_at timestamp
    pub fn expires_at(&self) -> i64 {
        // Use settle_slot converted to timestamp
        self.settle_slot as i64 * 400 / 1000 // 0.4s per slot
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AMMType {
    LMSR,        // Binary markets
    PMAMM,       // Multi-outcome (2-64)
    L2Norm,      // Continuous distributions
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProposalState {
    Active,
    Paused,
    Resolved,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Yes,
    No,
    Index(u8), // For multi-outcome markets
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct Resolution {
    pub winning_outcome: Outcome,
    pub resolution_slot: u64,
    pub resolver: Pubkey,
    pub evidence_hash: [u8; 32],
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct ChainPosition {
    pub chain_id: [u8; 32],
    pub position_size: u64,
    pub entry_price: u64,
    pub leverage: u64,
}

#[account]
pub struct MapEntryPDA {
    pub user: Pubkey,                // 32 bytes - User pubkey
    pub verse_id: u128,              // 16 bytes - Verse ID
    pub positions: Vec<Position>,    // Variable - User positions
    pub total_collateral: u64,       // 8 bytes - Total locked collateral
    pub total_borrowed: u64,         // 8 bytes - Total borrowed in chains
    pub last_update: i64,            // 8 bytes - Last update timestamp
    pub realized_pnl: i64,           // 8 bytes - Realized P&L
    pub unrealized_pnl: i64,         // 8 bytes - Unrealized P&L
    pub health_factor: u64,          // 8 bytes - Position health (fixed-point)
}

impl MapEntryPDA {
    pub const BASE_LEN: usize = 8 + 32 + 16 + 8 + 8 + 8 + 8 + 8 + 8;

    pub fn space(max_positions: usize) -> usize {
        Self::BASE_LEN + 4 + (max_positions * Position::LEN)
    }

    pub fn calculate_health(&self, current_prices: &[u64]) -> u64 {
        if self.total_collateral == 0 {
            return 0;
        }

        let mut total_value = self.total_collateral;
        let mut total_risk = 0u64;

        for (i, position) in self.positions.iter().enumerate() {
            let current_price = current_prices.get(i).unwrap_or(&position.entry_price);
            let price_delta = if *current_price > position.entry_price {
                current_price - position.entry_price
            } else {
                position.entry_price - current_price
            };

            let position_risk = (price_delta * position.size * position.leverage)
                / PRICE_PRECISION;

            total_risk = total_risk.saturating_add(position_risk);
        }

        if total_risk == 0 {
            u64::MAX
        } else {
            (total_value * HEALTH_PRECISION) / total_risk
        }
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct Position {
    pub proposal_id: u128,           // 16 bytes
    pub outcome: u8,                 // 1 byte
    pub size: u64,                   // 8 bytes
    pub leverage: u64,               // 8 bytes
    pub entry_price: u64,            // 8 bytes
    pub liquidation_price: u64,      // 8 bytes
    pub is_long: bool,               // 1 byte
    pub created_at: i64,             // 8 bytes
}

impl Position {
    pub const LEN: usize = 16 + 1 + 8 + 8 + 8 + 8 + 1 + 8;
}

// ============= Additional PDAs =============

#[account]
pub struct PriceCachePDA {
    pub verse_id: u128,
    pub proposal_id: u128,
    pub last_price: u64,
    pub last_update_slot: u64,
    pub price_history: Vec<PricePoint>,
    pub volatility: u64,
    pub bump: u8,
}

impl PriceCachePDA {
    pub const BASE_LEN: usize = 8 + 16 + 16 + 8 + 8 + 8 + 1;
    
    pub fn space(history_size: usize) -> usize {
        Self::BASE_LEN + 4 + (history_size * PricePoint::LEN)
    }
    
    pub fn is_stale(&self, current_slot: u64) -> bool {
        current_slot > self.last_update_slot + 10 // Stale after 10 slots
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct PricePoint {
    pub price: u64,
    pub slot: u64,
    pub volume: u64,
}

impl PricePoint {
    pub const LEN: usize = 8 + 8 + 8;
}

#[account]
pub struct PriceHistory {
    pub movements: Vec<i64>, // Recent price movements for circuit breaker checks
}

// Keep the existing GlobalConfigPDA but add missing fields
#[account]
pub struct GlobalConfigPDA {
    pub epoch: u64,
    pub coverage: u128,
    pub vault: u64,
    pub total_oi: u64,
    pub halt_flag: bool,
    pub halt_until: u64,              // Added: halt duration
    pub fee_base: u64,                // 3bp in basis points
    pub fee_slope: u64,               // 25bp
    pub season: u64,
    pub genesis_slot: u64,
    pub season_start_slot: u64,
    pub season_end_slot: u64,
    pub mmt_total_supply: u64,
    pub mmt_current_season: u64,
    pub mmt_emission_rate: u64,
    pub mmt_reward_pool: u64,         // Added: MMT reward pool
    pub leverage_tiers: Vec<LeverageTier>,
    pub keeper_reward_bps: u16,       // Keeper reward percentage in basis points
    pub insurance_fund_bps: u16,      // Insurance fund percentage in basis points
    pub bump: u8,                     // Added: PDA bump
}

impl GlobalConfigPDA {
    pub const LEN: usize = 8 + // discriminator
        8 + // epoch
        16 + // coverage
        8 + // vault
        8 + // total_oi
        1 + // halt_flag
        8 + // halt_until
        8 + // fee_base
        8 + // fee_slope
        8 + // season
        8 + // genesis_slot
        8 + // season_start_slot
        8 + // season_end_slot
        8 + // mmt_total_supply
        8 + // mmt_current_season
        8 + // mmt_emission_rate
        8 + // mmt_reward_pool
        4 + (7 * 12) + // leverage_tiers (vec length + 7 tiers * (4 + 8) bytes each)
        2 + // keeper_reward_bps
        2 + // insurance_fund_bps
        1; // bump
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy)]
pub struct LeverageTier {
    pub n: u32,
    pub max: u64,
}

// ============= Account Context Structs =============

#[derive(Accounts)]
pub struct OpenPosition<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    
    #[account(mut)]
    pub global_config: Account<'info, GlobalConfigPDA>,
    
    #[account(mut)]
    pub verse: Account<'info, VersePDA>,
    
    #[account(mut)]
    pub proposal: Account<'info, ProposalPDA>,
    
    #[account(
        init_if_needed,
        payer = user,
        space = MapEntryPDA::space(50), // Max 50 positions per user
        seeds = [b"map_entry", verse.verse_id_as_u128().to_le_bytes().as_ref(), user.key().as_ref()],
        bump
    )]
    pub user_map: Account<'info, MapEntryPDA>,
    
    #[account(mut)]
    pub price_cache: Account<'info, PriceCachePDA>,
    
    /// CHECK: User's token account for collateral
    #[account(mut)]
    pub user_token_account: AccountInfo<'info>,
    
    /// CHECK: Vault token account
    #[account(mut)]
    pub vault_token_account: AccountInfo<'info>,
    
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct PartialLiquidate<'info> {
    #[account(mut)]
    pub keeper: Signer<'info>,
    
    #[account(mut)]
    pub global_config: Account<'info, GlobalConfigPDA>,
    
    #[account(mut)]
    pub user_map: Account<'info, MapEntryPDA>,
    
    #[account(mut)]
    pub price_cache: Account<'info, PriceCachePDA>,
    
    pub price_history: Account<'info, PriceHistory>,
    
    /// CHECK: User whose position is being liquidated
    pub user: AccountInfo<'info>,
    
    /// CHECK: Vault token account
    #[account(mut)]
    pub vault_token_account: AccountInfo<'info>,
    
    /// CHECK: Keeper's token account for rewards
    #[account(mut)]
    pub keeper_token_account: AccountInfo<'info>,
    
    /// CHECK: Vault authority PDA
    pub vault_authority: AccountInfo<'info>,
    
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct DistributeFees<'info> {
    #[account(mut)]
    pub global_config: Account<'info, GlobalConfigPDA>,
    
    /// CHECK: USDC mint
    pub usdc_mint: AccountInfo<'info>,
    
    /// CHECK: Vault token account
    #[account(mut)]
    pub vault_token_account: AccountInfo<'info>,
    
    /// CHECK: Vault authority PDA
    pub vault_authority: AccountInfo<'info>,
    
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct MonitorHealth<'info> {
    #[account(mut)]
    pub user_map: Account<'info, MapEntryPDA>,
    
    pub price_cache: Account<'info, PriceCachePDA>,
}

// ============= Parameter Structs =============

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct OpenPositionParams {
    pub amount: u64,
    pub leverage: u64,
    pub outcome: u8,
    pub is_long: bool,
}