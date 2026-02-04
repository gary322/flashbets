//! MMT Token State Accounts
//! 
//! All account structures for the MMT token distribution and staking system
//! Native Solana implementation - NO ANCHOR

use solana_program::{
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack, Sealed},
    pubkey::Pubkey,
};
use borsh::{BorshDeserialize, BorshSerialize};
use super::staking::StakingTier;
// Fixed point types are converted to/from u64/u128 for serialization

/// Alias for backward compatibility
pub type MMTState = MMTConfig;

/// MMT token configuration account
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct MMTConfig {
    /// Account discriminator
    pub discriminator: [u8; 8],
    /// Is initialized
    pub is_initialized: bool,
    /// MMT mint address
    pub mint: Pubkey,
    /// Authority for administrative operations
    pub authority: Pubkey,
    /// Total supply (100M with 6 decimals)
    pub total_supply: u64,
    /// Current circulating supply
    pub circulating_supply: u64,
    /// Tokens allocated per season (10M)
    pub season_allocation: u64,
    /// Current season number
    pub current_season: u8,
    /// Start slot of current season
    pub season_start_slot: u64,
    /// Tokens emitted in current season
    pub season_emitted: u64,
    /// Locked supply (90M reserved)
    pub locked_supply: u64,
    /// Bump seed for PDA
    pub bump: u8,
}

impl MMTConfig {
    pub const DISCRIMINATOR: [u8; 8] = [0x4D, 0x4D, 0x54, 0x5F, 0x43, 0x46, 0x47, 0x00];
    pub const LEN: usize = 8 + 1 + 32 + 32 + 8 + 8 + 8 + 1 + 8 + 8 + 8 + 1 + 32; // 178 bytes + padding
}

impl Sealed for MMTConfig {}

impl IsInitialized for MMTConfig {
    fn is_initialized(&self) -> bool {
        self.is_initialized
    }
}

impl Pack for MMTConfig {
    const LEN: usize = 256; // Padded for future expansion

    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        let mut data = src;
        let config = MMTConfig::deserialize(&mut data)
            .map_err(|_| ProgramError::InvalidAccountData)?;
        
        if config.discriminator != Self::DISCRIMINATOR {
            return Err(ProgramError::InvalidAccountData);
        }
        
        Ok(config)
    }

    fn pack_into_slice(&self, dst: &mut [u8]) {
        let data = self.try_to_vec().unwrap();
        dst[..data.len()].copy_from_slice(&data);
    }
}

/// Season emission tracking account
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct SeasonEmission {
    /// Account discriminator
    pub discriminator: [u8; 8],
    /// Is initialized
    pub is_initialized: bool,
    /// Season number
    pub season: u8,
    /// Total tokens allocated for this season
    pub total_allocation: u64,
    /// Tokens already emitted
    pub emitted_amount: u64,
    /// Tokens distributed as maker rewards
    pub maker_rewards: u64,
    /// Tokens distributed as staking rewards/rebates
    pub staking_rewards: u64,
    /// Tokens distributed as early trader bonuses
    pub early_trader_bonus: u64,
    /// Season start slot
    pub start_slot: u64,
    /// Season end slot
    pub end_slot: u64,
}

impl SeasonEmission {
    pub const DISCRIMINATOR: [u8; 8] = [0x53, 0x45, 0x41, 0x53, 0x4F, 0x4E, 0x00, 0x00];
    pub const LEN: usize = 8 + 1 + 1 + 8 + 8 + 8 + 8 + 8 + 8 + 8 + 32; // 98 bytes + padding
}

impl Sealed for SeasonEmission {}

impl IsInitialized for SeasonEmission {
    fn is_initialized(&self) -> bool {
        self.is_initialized
    }
}

impl Pack for SeasonEmission {
    const LEN: usize = 128; // Padded for future expansion

    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        let mut data = src;
        let emission = SeasonEmission::deserialize(&mut data)
            .map_err(|_| ProgramError::InvalidAccountData)?;
        
        if emission.discriminator != Self::DISCRIMINATOR {
            return Err(ProgramError::InvalidAccountData);
        }
        
        Ok(emission)
    }

    fn pack_into_slice(&self, dst: &mut [u8]) {
        let data = self.try_to_vec().unwrap();
        dst[..data.len()].copy_from_slice(&data);
    }
}

/// User stake account
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct StakeAccount {
    /// Account discriminator
    pub discriminator: [u8; 8],
    /// Is initialized
    pub is_initialized: bool,
    /// Owner pubkey
    pub owner: Pubkey,
    /// Amount of MMT staked
    pub amount_staked: u64,
    /// Timestamp when staked
    pub stake_timestamp: i64,
    /// Last slot when rewards were claimed
    pub last_claim_slot: u64,
    /// Accumulated rewards to claim
    pub accumulated_rewards: u64,
    /// Rebate percentage (based on stake share) - stored as basis points
    pub rebate_percentage: u64,  // basis points where 10000 = 100%
    /// Optional lock period end slot
    pub lock_end_slot: Option<u64>,
    /// Lock multiplier applied (10000 = 1x, 12500 = 1.25x)
    pub lock_multiplier: u16,
    /// User's staking tier based on amount staked
    pub tier: StakingTier,
    /// Alias for amount_staked (for backward compatibility)
    pub amount: u64,
    /// Whether the stake is locked
    pub is_locked: bool,
    /// Total rewards earned over time
    pub rewards_earned: u64,
}

impl StakeAccount {
    pub const DISCRIMINATOR: [u8; 8] = [0x53, 0x54, 0x41, 0x4B, 0x45, 0x00, 0x00, 0x00];
    pub const LEN: usize = 8 + 1 + 32 + 8 + 8 + 8 + 8 + 8 + 9 + 2 + 1 + 8 + 1 + 8 + 32; // 151 bytes + padding
}

impl Sealed for StakeAccount {}

impl IsInitialized for StakeAccount {
    fn is_initialized(&self) -> bool {
        self.is_initialized
    }
}

impl Pack for StakeAccount {
    const LEN: usize = 256; // Padded for future expansion

    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        let mut data = src;
        let account = StakeAccount::deserialize(&mut data)
            .map_err(|_| ProgramError::InvalidAccountData)?;
        
        if account.discriminator != Self::DISCRIMINATOR {
            return Err(ProgramError::InvalidAccountData);
        }
        
        Ok(account)
    }

    fn pack_into_slice(&self, dst: &mut [u8]) {
        let data = self.try_to_vec().unwrap();
        dst[..data.len()].copy_from_slice(&data);
    }
}

/// Global staking pool state
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct StakingPool {
    /// Account discriminator
    pub discriminator: [u8; 8],
    /// Is initialized
    pub is_initialized: bool,
    /// Total MMT staked across all users
    pub total_staked: u64,
    /// Total number of active stakers
    pub total_stakers: u32,
    /// Reward tokens per slot
    pub reward_per_slot: u64,
    /// Last slot when rewards were updated
    pub last_update_slot: u64,
    /// Accumulated rewards per share (high precision) - stored as u128
    pub accumulated_rewards_per_share: u128,
    /// Base rebate percentage (15% = 1500 basis points)
    pub rebate_percentage_base: u64,
    /// Total fees collected for distribution
    pub total_fees_collected: u64,
    /// Total rebates distributed
    pub total_rebates_distributed: u64,
}

impl StakingPool {
    pub const DISCRIMINATOR: [u8; 8] = [0x53, 0x54, 0x41, 0x4B, 0x50, 0x4F, 0x4F, 0x4C];
    pub const LEN: usize = 8 + 1 + 8 + 4 + 8 + 8 + 32 + 16 + 8 + 8 + 64; // 165 bytes + padding
}

impl Sealed for StakingPool {}

impl IsInitialized for StakingPool {
    fn is_initialized(&self) -> bool {
        self.is_initialized
    }
}

impl Pack for StakingPool {
    const LEN: usize = 256; // Padded for future expansion

    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        let mut data = src;
        let pool = StakingPool::deserialize(&mut data)
            .map_err(|_| ProgramError::InvalidAccountData)?;
        
        if pool.discriminator != Self::DISCRIMINATOR {
            return Err(ProgramError::InvalidAccountData);
        }
        
        Ok(pool)
    }

    fn pack_into_slice(&self, dst: &mut [u8]) {
        let data = self.try_to_vec().unwrap();
        dst[..data.len()].copy_from_slice(&data);
    }
}

/// Maker metrics for tracking performance
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct MakerMetrics {
    /// Total trading volume
    pub total_volume: u64,
    /// Sum of all spread improvements (in basis points)
    pub spread_improvements: u64,
    /// Number of trades executed
    pub trades_count: u32,
    /// Average spread improvement per trade in basis points
    pub average_spread_improvement_bp: u64,
    /// Last trade slot
    pub last_trade_slot: u64,
}

/// Maker account for rewards tracking
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct MakerAccount {
    /// Account discriminator
    pub discriminator: [u8; 8],
    /// Is initialized
    pub is_initialized: bool,
    /// Owner pubkey
    pub owner: Pubkey,
    /// Trading metrics
    pub metrics: MakerMetrics,
    /// Pending rewards to claim
    pub pending_rewards: u64,
    /// Total rewards already claimed
    pub total_rewards_claimed: u64,
    /// Is this an early trader (first 100)
    pub is_early_trader: bool,
}

impl MakerAccount {
    pub const DISCRIMINATOR: [u8; 8] = [0x4D, 0x41, 0x4B, 0x45, 0x52, 0x00, 0x00, 0x00];
    pub const LEN: usize = 8 + 1 + 32 + (8 + 8 + 4 + 16 + 8) + 8 + 8 + 1 + 32; // 133 bytes + padding
}

impl Sealed for MakerAccount {}

impl IsInitialized for MakerAccount {
    fn is_initialized(&self) -> bool {
        self.is_initialized
    }
}

impl Pack for MakerAccount {
    const LEN: usize = 256; // Padded for future expansion

    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        let mut data = src;
        let account = MakerAccount::deserialize(&mut data)
            .map_err(|_| ProgramError::InvalidAccountData)?;
        
        if account.discriminator != Self::DISCRIMINATOR {
            return Err(ProgramError::InvalidAccountData);
        }
        
        Ok(account)
    }

    fn pack_into_slice(&self, dst: &mut [u8]) {
        let data = self.try_to_vec().unwrap();
        dst[..data.len()].copy_from_slice(&data);
    }
}

/// Early trader registry for first 100 traders
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct EarlyTraderRegistry {
    /// Account discriminator
    pub discriminator: [u8; 8],
    /// Is initialized
    pub is_initialized: bool,
    /// Season number
    pub season: u8,
    /// Number of registered early traders
    pub count: u32,
    /// List of early trader pubkeys (max 100)
    pub traders: Vec<Pubkey>,
}

impl EarlyTraderRegistry {
    pub const DISCRIMINATOR: [u8; 8] = [0x45, 0x41, 0x52, 0x4C, 0x59, 0x00, 0x00, 0x00];
    pub const MAX_TRADERS: usize = 100;
    pub const LEN: usize = 8 + 1 + 1 + 4 + 4 + (32 * Self::MAX_TRADERS) + 64; // 3278 bytes + padding
}

impl Sealed for EarlyTraderRegistry {}

impl IsInitialized for EarlyTraderRegistry {
    fn is_initialized(&self) -> bool {
        self.is_initialized
    }
}

impl Pack for EarlyTraderRegistry {
    const LEN: usize = 4096; // Large account for 100 traders

    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        let mut data = src;
        let registry = EarlyTraderRegistry::deserialize(&mut data)
            .map_err(|_| ProgramError::InvalidAccountData)?;
        
        if registry.discriminator != Self::DISCRIMINATOR {
            return Err(ProgramError::InvalidAccountData);
        }
        
        Ok(registry)
    }

    fn pack_into_slice(&self, dst: &mut [u8]) {
        let data = self.try_to_vec().unwrap();
        dst[..data.len()].copy_from_slice(&data);
    }
}

/// Distribution record for tracking token distributions
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct DistributionRecord {
    /// Account discriminator
    pub discriminator: [u8; 8],
    /// Is initialized
    pub is_initialized: bool,
    /// Type of distribution
    pub distribution_type: DistributionType,
    /// Recipient address
    pub recipient: Pubkey,
    /// Amount distributed
    pub amount: u64,
    /// Slot when distributed
    pub slot: u64,
    /// Season number
    pub season: u8,
    /// Transaction signature
    pub transaction_signature: [u8; 64],
}

/// Types of token distributions
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub enum DistributionType {
    MakerReward,
    StakingReward,
    EarlyTraderBonus,
    VaultSeed,
    Airdrop,
    EarlyLiquidityProvider,
}

impl DistributionRecord {
    pub const DISCRIMINATOR: [u8; 8] = [0x44, 0x49, 0x53, 0x54, 0x52, 0x00, 0x00, 0x00];
    pub const LEN: usize = 8 + 1 + 1 + 32 + 8 + 8 + 1 + 64 + 32; // 155 bytes + padding
}

impl Sealed for DistributionRecord {}

impl IsInitialized for DistributionRecord {
    fn is_initialized(&self) -> bool {
        self.is_initialized
    }
}

impl Pack for DistributionRecord {
    const LEN: usize = 256; // Padded for future expansion

    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        let mut data = src;
        let record = DistributionRecord::deserialize(&mut data)
            .map_err(|_| ProgramError::InvalidAccountData)?;
        
        if record.discriminator != Self::DISCRIMINATOR {
            return Err(ProgramError::InvalidAccountData);
        }
        
        Ok(record)
    }

    fn pack_into_slice(&self, dst: &mut [u8]) {
        let data = self.try_to_vec().unwrap();
        dst[..data.len()].copy_from_slice(&data);
    }
}

/// Treasury management account
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct TreasuryAccount {
    /// Account discriminator
    pub discriminator: [u8; 8],
    /// Is initialized
    pub is_initialized: bool,
    /// Treasury vault address
    pub vault: Pubkey,
    /// Authority for treasury operations
    pub authority: Pubkey,
    /// Total tokens in treasury
    pub balance: u64,
    /// Total distributed from treasury
    pub total_distributed: u64,
    /// Bump seed for PDA
    pub bump: u8,
}

impl TreasuryAccount {
    pub const DISCRIMINATOR: [u8; 8] = [0x54, 0x52, 0x45, 0x41, 0x53, 0x00, 0x00, 0x00];
    pub const LEN: usize = 8 + 1 + 32 + 32 + 8 + 8 + 1 + 64; // 154 bytes + padding
}

impl Sealed for TreasuryAccount {}

impl IsInitialized for TreasuryAccount {
    fn is_initialized(&self) -> bool {
        self.is_initialized
    }
}

impl Pack for TreasuryAccount {
    const LEN: usize = 256; // Padded for future expansion

    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        let mut data = src;
        let treasury = TreasuryAccount::deserialize(&mut data)
            .map_err(|_| ProgramError::InvalidAccountData)?;
        
        if treasury.discriminator != Self::DISCRIMINATOR {
            return Err(ProgramError::InvalidAccountData);
        }
        
        Ok(treasury)
    }

    fn pack_into_slice(&self, dst: &mut [u8]) {
        let data = self.try_to_vec().unwrap();
        dst[..data.len()].copy_from_slice(&data);
    }
}

/// Reserved token vault (for 90M locked tokens)
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct ReservedVault {
    /// Account discriminator
    pub discriminator: [u8; 8],
    /// Is initialized
    pub is_initialized: bool,
    /// Amount locked (90M)
    pub locked_amount: u64,
    /// Authority (set to system program for permanent lock)
    pub authority: Pubkey,
    /// Lock timestamp
    pub lock_timestamp: i64,
    /// Is permanently locked
    pub is_permanently_locked: bool,
    /// Bump seed for PDA
    pub bump: u8,
}

impl ReservedVault {
    pub const DISCRIMINATOR: [u8; 8] = [0x52, 0x45, 0x53, 0x56, 0x00, 0x00, 0x00, 0x00];
    pub const LEN: usize = 8 + 1 + 8 + 32 + 8 + 1 + 1 + 64; // 123 bytes + padding
}

impl Sealed for ReservedVault {}

impl IsInitialized for ReservedVault {
    fn is_initialized(&self) -> bool {
        self.is_initialized
    }
}

impl Pack for ReservedVault {
    const LEN: usize = 256; // Padded for future expansion

    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        let mut data = src;
        let vault = ReservedVault::deserialize(&mut data)
            .map_err(|_| ProgramError::InvalidAccountData)?;
        
        if vault.discriminator != Self::DISCRIMINATOR {
            return Err(ProgramError::InvalidAccountData);
        }
        
        Ok(vault)
    }

    fn pack_into_slice(&self, dst: &mut [u8]) {
        let data = self.try_to_vec().unwrap();
        dst[..data.len()].copy_from_slice(&data);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mmt_config_pack_unpack() {
        let config = MMTConfig {
            discriminator: MMTConfig::DISCRIMINATOR,
            is_initialized: true,
            mint: Pubkey::new_unique(),
            authority: Pubkey::new_unique(),
            total_supply: 100_000_000_000_000,
            circulating_supply: 10_000_000_000_000,
            season_allocation: 10_000_000_000_000,
            current_season: 1,
            season_start_slot: 1000,
            season_emitted: 0,
            locked_supply: 90_000_000_000_000,
            bump: 255,
        };

        let mut packed = vec![0u8; MMTConfig::LEN];
        config.pack_into_slice(&mut packed);

        let unpacked = MMTConfig::unpack_from_slice(&packed).unwrap();
        assert_eq!(unpacked.total_supply, config.total_supply);
        assert_eq!(unpacked.mint, config.mint);
        assert_eq!(unpacked.current_season, config.current_season);
    }

    #[test]
    fn test_stake_account_pack_unpack() {
        let stake = StakeAccount {
            discriminator: StakeAccount::DISCRIMINATOR,
            is_initialized: true,
            owner: Pubkey::new_unique(),
            amount_staked: 1_000_000_000,
            stake_timestamp: 1234567890,
            last_claim_slot: 5000,
            accumulated_rewards: 100_000,
            rebate_percentage: 1500, // 15% = 1500 basis points
            lock_end_slot: Some(10000),
            lock_multiplier: 12500, // 1.25x
            tier: StakingTier::Bronze,
            amount: 1_000_000_000, // Same as amount_staked
            is_locked: true, // Has lock_end_slot
            rewards_earned: 100_000, // Same as accumulated_rewards
        };

        let mut packed = vec![0u8; StakeAccount::LEN];
        stake.pack_into_slice(&mut packed);

        let unpacked = StakeAccount::unpack_from_slice(&packed).unwrap();
        assert_eq!(unpacked.amount_staked, stake.amount_staked);
        assert_eq!(unpacked.lock_end_slot, stake.lock_end_slot);
        assert_eq!(unpacked.lock_multiplier, stake.lock_multiplier);
    }

    #[test]
    fn test_early_trader_registry_capacity() {
        let mut registry = EarlyTraderRegistry {
            discriminator: EarlyTraderRegistry::DISCRIMINATOR,
            is_initialized: true,
            season: 1,
            count: 0,
            traders: Vec::new(),
        };

        // Add maximum number of traders
        for _ in 0..EarlyTraderRegistry::MAX_TRADERS {
            registry.traders.push(Pubkey::new_unique());
            registry.count += 1;
        }

        let mut packed = vec![0u8; EarlyTraderRegistry::LEN];
        registry.pack_into_slice(&mut packed);

        let unpacked = EarlyTraderRegistry::unpack_from_slice(&packed).unwrap();
        assert_eq!(unpacked.count, EarlyTraderRegistry::MAX_TRADERS as u32);
        assert_eq!(unpacked.traders.len(), EarlyTraderRegistry::MAX_TRADERS);
    }
}