use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack, Sealed},
    pubkey::Pubkey,
    sysvar::Sysvar,
};
use std::collections::HashMap;
use crate::error::BettingPlatformError;
use crate::math::U64F64;

pub const MAX_MARKETS_PER_VERSE: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum SyntheticType {
    Verse = 0,          // Hierarchical grouping
    Quantum = 1,        // Multi-proposal bundle
    Distribution = 2,   // Continuous distribution wrapper
}

impl SyntheticType {
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(SyntheticType::Verse),
            1 => Some(SyntheticType::Quantum),
            2 => Some(SyntheticType::Distribution),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum WrapperStatus {
    Active = 0,
    Halted = 1,      // During Polymarket issues
    Migrating = 2,   // During version migration
    Resolved = 3,
}

impl WrapperStatus {
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(WrapperStatus::Active),
            1 => Some(WrapperStatus::Halted),
            2 => Some(WrapperStatus::Migrating),
            3 => Some(WrapperStatus::Resolved),
            _ => None,
        }
    }
}

/// Synthetic wrapper account structure
pub struct SyntheticWrapper {
    pub is_initialized: bool,
    pub synthetic_id: u128,
    pub synthetic_type: SyntheticType,
    pub polymarket_markets: Vec<Pubkey>, // Linked Polymarket market IDs
    pub weights: Vec<U64F64>,            // Weight for each market in derivation
    pub derived_probability: U64F64,      // Weighted average probability
    pub total_volume_7d: u64,            // For weight calculations
    pub last_update_slot: u64,
    pub status: WrapperStatus,
    pub is_verse_level: bool,            // Whether this is a verse-level synthetic
    pub bump: u8,
}

impl Sealed for SyntheticWrapper {}

impl IsInitialized for SyntheticWrapper {
    fn is_initialized(&self) -> bool {
        self.is_initialized
    }
}

impl Pack for SyntheticWrapper {
    const LEN: usize = 1 + // is_initialized
        16 + // synthetic_id
        1 + // synthetic_type
        4 + (32 * MAX_MARKETS_PER_VERSE) + // polymarket_markets vector
        4 + (8 * MAX_MARKETS_PER_VERSE) + // weights vector
        8 + // derived_probability
        8 + // total_volume_7d
        8 + // last_update_slot
        1 + // status
        1 + // is_verse_level
        1; // bump

    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        if src.len() < Self::LEN {
            return Err(ProgramError::InvalidAccountData);
        }

        let src = &src[..Self::LEN];
        let mut offset = 0;

        let is_initialized = src[offset] != 0;
        offset += 1;

        let synthetic_id = u128::from_le_bytes(
            src[offset..offset + 16].try_into().map_err(|_| ProgramError::InvalidAccountData)?
        );
        offset += 16;

        let synthetic_type = SyntheticType::from_u8(src[offset])
            .ok_or(ProgramError::InvalidAccountData)?;
        offset += 1;

        // Read polymarket_markets vector
        let markets_len = u32::from_le_bytes(
            src[offset..offset + 4].try_into().map_err(|_| ProgramError::InvalidAccountData)?
        ) as usize;
        offset += 4;

        if markets_len > MAX_MARKETS_PER_VERSE {
            return Err(ProgramError::InvalidAccountData);
        }

        let mut polymarket_markets = Vec::with_capacity(markets_len);
        for _ in 0..markets_len {
            let pubkey = Pubkey::new_from_array(
                src[offset..offset + 32].try_into().map_err(|_| ProgramError::InvalidAccountData)?
            );
            polymarket_markets.push(pubkey);
            offset += 32;
        }
        offset += (MAX_MARKETS_PER_VERSE - markets_len) * 32; // Skip unused space

        // Read weights vector
        let weights_len = u32::from_le_bytes(
            src[offset..offset + 4].try_into().map_err(|_| ProgramError::InvalidAccountData)?
        ) as usize;
        offset += 4;

        if weights_len != markets_len {
            return Err(ProgramError::InvalidAccountData);
        }

        let mut weights = Vec::with_capacity(weights_len);
        for _ in 0..weights_len {
            let weight = u64::from_le_bytes(
                src[offset..offset + 8].try_into().map_err(|_| ProgramError::InvalidAccountData)?
            );
            weights.push(U64F64::from_bits(weight));
            offset += 8;
        }
        offset += (MAX_MARKETS_PER_VERSE - weights_len) * 8; // Skip unused space

        let derived_probability = U64F64::from_bits(u64::from_le_bytes(
            src[offset..offset + 8].try_into().map_err(|_| ProgramError::InvalidAccountData)?
        ));
        offset += 8;

        let total_volume_7d = u64::from_le_bytes(
            src[offset..offset + 8].try_into().map_err(|_| ProgramError::InvalidAccountData)?
        );
        offset += 8;

        let last_update_slot = u64::from_le_bytes(
            src[offset..offset + 8].try_into().map_err(|_| ProgramError::InvalidAccountData)?
        );
        offset += 8;

        let status = WrapperStatus::from_u8(src[offset])
            .ok_or(ProgramError::InvalidAccountData)?;
        offset += 1;

        let is_verse_level = src[offset] != 0;
        offset += 1;

        let bump = src[offset];

        Ok(SyntheticWrapper {
            is_initialized,
            synthetic_id,
            synthetic_type,
            polymarket_markets,
            weights,
            derived_probability,
            total_volume_7d,
            last_update_slot,
            status,
            is_verse_level,
            bump,
        })
    }

    fn pack_into_slice(&self, dst: &mut [u8]) {
        // Since this is a trait implementation that doesn't return Result,
        // we need to ensure the buffer is large enough before packing
        assert!(dst.len() >= Self::LEN, "Buffer size must be at least {} bytes", Self::LEN);

        let dst = &mut dst[..Self::LEN];
        let mut offset = 0;

        dst[offset] = self.is_initialized as u8;
        offset += 1;

        dst[offset..offset + 16].copy_from_slice(&self.synthetic_id.to_le_bytes());
        offset += 16;

        dst[offset] = self.synthetic_type as u8;
        offset += 1;

        // Write polymarket_markets vector
        let markets_len = self.polymarket_markets.len() as u32;
        dst[offset..offset + 4].copy_from_slice(&markets_len.to_le_bytes());
        offset += 4;

        for market in &self.polymarket_markets {
            dst[offset..offset + 32].copy_from_slice(market.as_ref());
            offset += 32;
        }
        offset += (MAX_MARKETS_PER_VERSE - self.polymarket_markets.len()) * 32;

        // Write weights vector
        let weights_len = self.weights.len() as u32;
        dst[offset..offset + 4].copy_from_slice(&weights_len.to_le_bytes());
        offset += 4;

        for weight in &self.weights {
            dst[offset..offset + 8].copy_from_slice(&weight.to_bits().to_le_bytes());
            offset += 8;
        }
        offset += (MAX_MARKETS_PER_VERSE - self.weights.len()) * 8;

        dst[offset..offset + 8].copy_from_slice(&self.derived_probability.to_bits().to_le_bytes());
        offset += 8;

        dst[offset..offset + 8].copy_from_slice(&self.total_volume_7d.to_le_bytes());
        offset += 8;

        dst[offset..offset + 8].copy_from_slice(&self.last_update_slot.to_le_bytes());
        offset += 8;

        dst[offset] = self.status as u8;
        offset += 1;

        dst[offset] = self.is_verse_level as u8;
        offset += 1;

        dst[offset] = self.bump;
    }
}

/// Wrapper manager to handle multiple synthetic wrappers
pub struct WrapperManager {
    pub wrappers: HashMap<u128, SyntheticWrapper>,
    pub market_to_wrapper: HashMap<Pubkey, Vec<u128>>, // Reverse mapping
}

impl WrapperManager {
    pub fn new() -> Self {
        Self {
            wrappers: HashMap::new(),
            market_to_wrapper: HashMap::new(),
        }
    }

    pub fn create_verse_wrapper(
        &mut self,
        verse_id: u128,
        polymarket_markets: Vec<Pubkey>,
        initial_weights: Option<Vec<U64F64>>,
        clock: &Clock,
    ) -> ProgramResult {
        if polymarket_markets.len() > MAX_MARKETS_PER_VERSE {
            return Err(BettingPlatformError::TooManyMarkets.into());
        }

        if polymarket_markets.is_empty() {
            return Err(BettingPlatformError::NoMarketsProvided.into());
        }

        let weights = if let Some(w) = initial_weights {
            if w.len() != polymarket_markets.len() {
                return Err(BettingPlatformError::WeightMismatch.into());
            }
            w
        } else {
            // Equal weights if not specified
            let equal_weight = U64F64::from_num(1u64) / U64F64::from_num(polymarket_markets.len() as u64);
            vec![equal_weight; polymarket_markets.len()]
        };

        let wrapper = SyntheticWrapper {
            is_initialized: true,
            synthetic_id: verse_id,
            synthetic_type: SyntheticType::Verse,
            polymarket_markets: polymarket_markets.clone(),
            weights,
            derived_probability: U64F64::from_num(500_000), // Default to 50% (0.5 * 1e6)
            total_volume_7d: 0,
            last_update_slot: clock.slot,
            status: WrapperStatus::Active,
            is_verse_level: true, // This is a verse-level wrapper
            bump: 0, // Will be set when creating PDA
        };

        // Update reverse mappings
        for market in &polymarket_markets {
            self.market_to_wrapper
                .entry(*market)
                .or_insert_with(Vec::new)
                .push(verse_id);
        }

        self.wrappers.insert(verse_id, wrapper);
        Ok(())
    }

    pub fn get_wrapper(&self, verse_id: u128) -> Option<&SyntheticWrapper> {
        self.wrappers.get(&verse_id)
    }

    pub fn get_wrapper_mut(&mut self, verse_id: u128) -> Option<&mut SyntheticWrapper> {
        self.wrappers.get_mut(&verse_id)
    }

    pub fn update_derived_probability(
        &mut self,
        verse_id: u128,
        new_probability: U64F64,
        clock: &Clock,
    ) -> ProgramResult {
        if let Some(wrapper) = self.wrappers.get_mut(&verse_id) {
            wrapper.derived_probability = new_probability;
            wrapper.last_update_slot = clock.slot;
            Ok(())
        } else {
            Err(BettingPlatformError::WrapperNotFound.into())
        }
    }

    pub fn update_volume(
        &mut self,
        verse_id: u128,
        additional_volume: u64,
    ) -> ProgramResult {
        if let Some(wrapper) = self.wrappers.get_mut(&verse_id) {
            wrapper.total_volume_7d = wrapper.total_volume_7d
                .checked_add(additional_volume)
                .ok_or(ProgramError::InvalidAccountData)?;
            Ok(())
        } else {
            Err(BettingPlatformError::WrapperNotFound.into())
        }
    }

    pub fn set_wrapper_status(
        &mut self,
        verse_id: u128,
        status: WrapperStatus,
    ) -> ProgramResult {
        if let Some(wrapper) = self.wrappers.get_mut(&verse_id) {
            wrapper.status = status;
            Ok(())
        } else {
            Err(BettingPlatformError::WrapperNotFound.into())
        }
    }

    pub fn get_markets_for_wrapper(&self, verse_id: u128) -> Option<&Vec<Pubkey>> {
        self.wrappers.get(&verse_id).map(|w| &w.polymarket_markets)
    }

    pub fn get_wrappers_for_market(&self, market: &Pubkey) -> Option<&Vec<u128>> {
        self.market_to_wrapper.get(market)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_program::clock::Clock;

    #[test]
    fn test_synthetic_wrapper_pack_unpack() {
        let wrapper = SyntheticWrapper {
            is_initialized: true,
            synthetic_id: 12345,
            synthetic_type: SyntheticType::Verse,
            polymarket_markets: vec![Pubkey::new_unique(), Pubkey::new_unique()],
            weights: vec![U64F64::from_num(1) / U64F64::from_num(2), U64F64::from_num(1) / U64F64::from_num(2)], // 0.5, 0.5
            derived_probability: U64F64::from_num(3) / U64F64::from_num(4), // 0.75
            total_volume_7d: 1000000,
            last_update_slot: 100,
            status: WrapperStatus::Active,
            is_verse_level: true,
            bump: 1,
        };

        let mut packed = vec![0u8; SyntheticWrapper::LEN];
        wrapper.pack_into_slice(&mut packed);

        let unpacked = SyntheticWrapper::unpack_from_slice(&packed).unwrap();

        assert_eq!(wrapper.is_initialized, unpacked.is_initialized);
        assert_eq!(wrapper.synthetic_id, unpacked.synthetic_id);
        assert_eq!(wrapper.synthetic_type, unpacked.synthetic_type);
        assert_eq!(wrapper.polymarket_markets, unpacked.polymarket_markets);
        assert_eq!(wrapper.weights.len(), unpacked.weights.len());
        assert_eq!(wrapper.derived_probability, unpacked.derived_probability);
        assert_eq!(wrapper.total_volume_7d, unpacked.total_volume_7d);
        assert_eq!(wrapper.last_update_slot, unpacked.last_update_slot);
        assert_eq!(wrapper.status, unpacked.status);
        assert_eq!(wrapper.bump, unpacked.bump);
    }

    #[test]
    fn test_wrapper_manager_creation() {
        let mut manager = WrapperManager::new();
        let verse_id = 1u128;
        let markets = vec![Pubkey::new_unique(), Pubkey::new_unique()];
        let clock = Clock {
            slot: 100,
            epoch_start_timestamp: 0,
            epoch: 0,
            leader_schedule_epoch: 0,
            unix_timestamp: 0,
        };

        manager.create_verse_wrapper(
            verse_id,
            markets.clone(),
            None,
            &clock,
        ).unwrap();

        let wrapper = manager.get_wrapper(verse_id).unwrap();
        assert_eq!(wrapper.polymarket_markets.len(), 2);
        assert_eq!(wrapper.weights[0], U64F64::from_num(1) / U64F64::from_num(2)); // 0.5
        assert_eq!(wrapper.status, WrapperStatus::Active);
    }

    #[test]
    fn test_market_to_wrapper_mapping() {
        let mut manager = WrapperManager::new();
        let market1 = Pubkey::new_unique();
        let market2 = Pubkey::new_unique();
        let clock = Clock::default();

        // Create first verse with market1 and market2
        manager.create_verse_wrapper(1, vec![market1, market2], None, &clock).unwrap();

        // Create second verse with market1
        manager.create_verse_wrapper(2, vec![market1], None, &clock).unwrap();

        // Check mappings
        let wrappers_for_market1 = manager.get_wrappers_for_market(&market1).unwrap();
        assert_eq!(wrappers_for_market1.len(), 2);
        assert!(wrappers_for_market1.contains(&1));
        assert!(wrappers_for_market1.contains(&2));

        let wrappers_for_market2 = manager.get_wrappers_for_market(&market2).unwrap();
        assert_eq!(wrappers_for_market2.len(), 1);
        assert!(wrappers_for_market2.contains(&1));
    }
}