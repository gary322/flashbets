//! Data Compression for Account Storage Optimization
//!
//! Production-grade compression to minimize storage costs and improve performance

use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    math::U64F64,
    state::{Position, ProposalPDA, UserMap},
};

/// Compression strategies for different data types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CompressionStrategy {
    /// No compression
    None,
    /// Run-length encoding for repetitive data
    RunLength,
    /// Bit packing for boolean/small values
    BitPacking,
    /// Delta encoding for sequential values
    DeltaEncoding,
    /// Dictionary compression for repeated values
    Dictionary,
}

/// Compressed position format (36 bytes vs 200+ bytes uncompressed)
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct CompressedPosition {
    /// Position ID hash (8 bytes instead of 32)
    pub position_id_hash: u64,
    /// User pubkey index in global registry (2 bytes instead of 32)
    pub user_index: u16,
    /// Proposal ID (2 bytes)
    pub proposal_id: u16,
    /// Packed fields (4 bytes):
    /// - outcome: 2 bits
    /// - leverage: 7 bits (0-100)
    /// - is_long: 1 bit
    /// - is_closed: 1 bit
    /// - reserved: 21 bits
    pub packed_fields: u32,
    /// Size in smallest units (8 bytes)
    pub size: u64,
    /// Entry price as basis points from base (2 bytes)
    pub entry_price_bps: u16,
    /// Liquidation price as basis points from entry (2 bytes)
    pub liquidation_price_bps: i16,
    /// Created timestamp as hours since epoch (4 bytes)
    pub created_hours: u32,
}

impl CompressedPosition {
    /// Compress a full position
    pub fn compress(position: &Position, user_registry: &mut UserRegistry) -> Result<Self, ProgramError> {
        // Hash position ID to 8 bytes
        let position_id_hash = hash_to_u64(&position.position_id);
        
        // Look up user index
        let user_index = user_registry.get_or_register(&position.user)?;
        
        // Pack boolean and small fields
        let mut packed_fields = 0u32;
        packed_fields |= (position.outcome as u32 & 0x3) << 30; // 2 bits
        packed_fields |= ((position.leverage as u32).min(127) & 0x7F) << 23; // 7 bits
        packed_fields |= (position.is_long as u32) << 22; // 1 bit
        packed_fields |= (position.is_closed as u32) << 21; // 1 bit
        
        // Convert prices to basis points
        let base_price = 500_000u64; // 0.5 base
        let entry_price_bps = ((position.entry_price as i64 - base_price as i64) / 100) as u16;
        let liquidation_price_bps = ((position.liquidation_price as i64 - position.entry_price as i64) / 100) as i16;
        
        // Convert timestamp to hours since epoch
        let created_hours = (position.created_at / 3600) as u32;
        
        Ok(Self {
            position_id_hash,
            user_index,
            proposal_id: position.proposal_id as u16,
            packed_fields,
            size: position.size,
            entry_price_bps,
            liquidation_price_bps,
            created_hours,
        })
    }
    
    /// Decompress to full position
    pub fn decompress(&self, user_registry: &UserRegistry) -> Result<Position, ProgramError> {
        // Unpack fields
        let outcome = ((self.packed_fields >> 30) & 0x3) as u8;
        let leverage = ((self.packed_fields >> 23) & 0x7F) as u8;
        let is_long = ((self.packed_fields >> 22) & 0x1) != 0;
        let is_closed = ((self.packed_fields >> 21) & 0x1) != 0;
        
        // Reconstruct prices
        let base_price = 500_000u64;
        let entry_price = (base_price as i64 + (self.entry_price_bps as i64 * 100)) as u64;
        let liquidation_price = (entry_price as i64 + (self.liquidation_price_bps as i64 * 100)) as u64;
        
        // Get user pubkey
        let user = user_registry.get_pubkey(self.user_index)?;
        
        // Reconstruct timestamp
        let created_at = (self.created_hours as i64) * 3600;
        
        // Generate position ID from hash (lossy but acceptable for most uses)
        let mut position_id = [0u8; 32];
        position_id[..8].copy_from_slice(&self.position_id_hash.to_le_bytes());
        
        Ok(Position {
            discriminator: [0; 8],
            version: 1,
            user,
            proposal_id: self.proposal_id as u128,
            position_id,
            outcome,
            size: self.size,
            notional: self.size, // Simplified
            leverage: leverage as u64,
            entry_price,
            liquidation_price,
            is_long,
            created_at,
            is_closed,
            partial_liq_accumulator: 0, // Lost in compression
            verse_id: 1, // Default
            margin: self.size / leverage as u64, // Reconstructed
            collateral: 0,
            is_short: !is_long,
            last_mark_price: entry_price, // Default to entry
            unrealized_pnl: 0, // Needs recalculation
            unrealized_pnl_pct: 0, // Needs recalculation
            cross_margin_enabled: false, // Default for compressed positions
            entry_funding_index: Some(U64F64::from_num(0)), // Default for compressed positions
        })
    }
}

/// User registry for index-based compression
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct UserRegistry {
    /// Mapping of user pubkeys to indices
    pub users: Vec<Pubkey>,
    /// Reverse lookup
    pub index_map: std::collections::HashMap<Pubkey, u16>,
}

impl UserRegistry {
    pub fn new() -> Self {
        Self {
            users: Vec::new(),
            index_map: std::collections::HashMap::new(),
        }
    }
    
    /// Get or register a user
    pub fn get_or_register(&mut self, user: &Pubkey) -> Result<u16, ProgramError> {
        if let Some(&index) = self.index_map.get(user) {
            Ok(index)
        } else {
            let index = self.users.len() as u16;
            if index == u16::MAX {
                return Err(BettingPlatformError::RegistryFull.into());
            }
            self.users.push(*user);
            self.index_map.insert(*user, index);
            Ok(index)
        }
    }
    
    /// Get pubkey by index
    pub fn get_pubkey(&self, index: u16) -> Result<Pubkey, ProgramError> {
        self.users.get(index as usize)
            .copied()
            .ok_or(BettingPlatformError::InvalidIndex.into())
    }
}

/// Bit-packed order book entry (8 bytes vs 64+ bytes)
#[derive(Debug, Clone, Copy)]
pub struct PackedOrder {
    /// Price in basis points (2 bytes)
    pub price_bps: u16,
    /// Size in units (4 bytes)
    pub size: u32,
    /// Packed flags (2 bytes):
    /// - user_index: 14 bits
    /// - is_buy: 1 bit
    /// - is_active: 1 bit
    pub flags: u16,
}

impl PackedOrder {
    pub fn pack(price: u64, size: u64, user_index: u16, is_buy: bool) -> Self {
        let price_bps = (price / 100) as u16;
        let size_u32 = size.min(u32::MAX as u64) as u32;
        
        let mut flags = 0u16;
        flags |= (user_index & 0x3FFF) << 2; // 14 bits
        flags |= (is_buy as u16) << 1; // 1 bit
        flags |= 1; // is_active = true
        
        Self {
            price_bps,
            size: size_u32,
            flags,
        }
    }
    
    pub fn unpack(&self) -> (u64, u64, u16, bool, bool) {
        let price = (self.price_bps as u64) * 100;
        let size = self.size as u64;
        let user_index = (self.flags >> 2) & 0x3FFF;
        let is_buy = ((self.flags >> 1) & 0x1) != 0;
        let is_active = (self.flags & 0x1) != 0;
        
        (price, size, user_index, is_buy, is_active)
    }
}

/// Delta-encoded price series for efficient storage
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct DeltaEncodedPrices {
    /// Base price
    pub base_price: u64,
    /// Base timestamp
    pub base_timestamp: i64,
    /// Delta-encoded prices (1 byte each for small changes)
    pub deltas: Vec<i8>,
    /// Time intervals (1 byte each for regular intervals)
    pub time_deltas: Vec<u8>,
}

impl DeltaEncodedPrices {
    pub fn encode(prices: &[(u64, i64)]) -> Result<Self, ProgramError> {
        if prices.is_empty() {
            return Err(BettingPlatformError::EmptyPriceSeries.into());
        }
        
        let base_price = prices[0].0;
        let base_timestamp = prices[0].1;
        
        let mut deltas = Vec::with_capacity(prices.len() - 1);
        let mut time_deltas = Vec::with_capacity(prices.len() - 1);
        
        for i in 1..prices.len() {
            // Price delta in basis points
            let price_delta = ((prices[i].0 as i64 - prices[i-1].0 as i64) / 100) as i8;
            deltas.push(price_delta);
            
            // Time delta in minutes
            let time_delta = ((prices[i].1 - prices[i-1].1) / 60).min(255) as u8;
            time_deltas.push(time_delta);
        }
        
        Ok(Self {
            base_price,
            base_timestamp,
            deltas,
            time_deltas,
        })
    }
    
    pub fn decode(&self) -> Vec<(u64, i64)> {
        let mut prices = Vec::with_capacity(self.deltas.len() + 1);
        prices.push((self.base_price, self.base_timestamp));
        
        let mut current_price = self.base_price;
        let mut current_timestamp = self.base_timestamp;
        
        for (delta, time_delta) in self.deltas.iter().zip(self.time_deltas.iter()) {
            current_price = (current_price as i64 + (*delta as i64 * 100)) as u64;
            current_timestamp += (*time_delta as i64) * 60;
            prices.push((current_price, current_timestamp));
        }
        
        prices
    }
}

/// Compressed user map using bit vectors
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct CompressedUserMap {
    /// User pubkey hash (8 bytes)
    pub user_hash: u64,
    /// Bit vector of active positions (each bit = 1 position)
    pub position_bits: Vec<u8>,
    /// Compressed stats
    pub stats: CompressedUserStats,
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct CompressedUserStats {
    /// Total volume in millions (2 bytes)
    pub volume_millions: u16,
    /// Win rate in basis points (2 bytes)
    pub win_rate_bps: u16,
    /// Active positions count (1 byte)
    pub active_positions: u8,
    /// Flags (1 byte)
    pub flags: u8,
}

/// Memory-efficient batch compressor
pub struct BatchCompressor {
    /// Compression buffer
    buffer: Vec<u8>,
    /// Compression strategy
    strategy: CompressionStrategy,
}

impl BatchCompressor {
    pub fn new(strategy: CompressionStrategy) -> Self {
        Self {
            buffer: Vec::with_capacity(1024),
            strategy,
        }
    }
    
    /// Compress multiple positions in batch
    pub fn compress_positions_batch(
        &mut self,
        positions: &[Position],
        user_registry: &mut UserRegistry,
    ) -> Result<Vec<u8>, ProgramError> {
        self.buffer.clear();
        
        // Write header
        self.buffer.extend_from_slice(&(positions.len() as u32).to_le_bytes());
        
        // Compress each position
        for position in positions {
            let compressed = CompressedPosition::compress(position, user_registry)?;
            compressed.serialize(&mut self.buffer)?;
        }
        
        Ok(self.buffer.clone())
    }
    
    /// Decompress batch
    pub fn decompress_positions_batch(
        &mut self,
        data: &[u8],
        user_registry: &UserRegistry,
    ) -> Result<Vec<Position>, ProgramError> {
        let mut cursor = 0;
        
        // Read header
        let count = u32::from_le_bytes(data[0..4].try_into().unwrap()) as usize;
        cursor += 4;
        
        let mut positions = Vec::with_capacity(count);
        
        // Decompress each position
        for _ in 0..count {
            let compressed = CompressedPosition::deserialize(&mut &data[cursor..])?;
            cursor += std::mem::size_of::<CompressedPosition>();
            
            let position = compressed.decompress(user_registry)?;
            positions.push(position);
        }
        
        Ok(positions)
    }
}

/// Hash bytes to u64 for compression
fn hash_to_u64(bytes: &[u8]) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    std::hash::Hash::hash_slice(bytes, &mut hasher);
    std::hash::Hasher::finish(&hasher)
}

/// Compression metrics
#[derive(Debug)]
pub struct CompressionMetrics {
    pub original_size: usize,
    pub compressed_size: usize,
    pub compression_ratio: f64,
    pub compression_time_us: u64,
}

impl CompressionMetrics {
    pub fn calculate(original: usize, compressed: usize, time_us: u64) -> Self {
        let compression_ratio = original as f64 / compressed as f64;
        Self {
            original_size: original,
            compressed_size: compressed,
            compression_ratio,
            compression_time_us: time_us,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_position_compression() {
        let mut registry = UserRegistry::new();
        let user = Pubkey::new_unique();
        
        let position = Position {
            discriminator: [0; 8],
            version: 1,
            user,
            proposal_id: 42,
            position_id: [1; 32],
            outcome: 1,
            size: 1_000_000_000,
            notional: 1_000_000_000,
            leverage: 10,
            entry_price: 525_000,
            liquidation_price: 520_000,
            is_long: true,
            created_at: 1_700_000_000,
            entry_funding_index: Some(U64F64::from_num(0)),
            is_closed: false,
            partial_liq_accumulator: 0,
            verse_id: 1,
            margin: 100_000_000,
            collateral: 0,
            is_short: false,
            last_mark_price: 525_000,
            unrealized_pnl: 0,
            cross_margin_enabled: false,
            unrealized_pnl_pct: 0,
        };
        
        let compressed = CompressedPosition::compress(&position, &mut registry).unwrap();
        assert_eq!(std::mem::size_of::<CompressedPosition>(), 36);
        
        let decompressed = compressed.decompress(&registry).unwrap();
        assert_eq!(decompressed.user, position.user);
        assert_eq!(decompressed.size, position.size);
        assert_eq!(decompressed.leverage, position.leverage);
    }
    
    #[test]
    fn test_packed_order() {
        let packed = PackedOrder::pack(525_000, 1_000_000, 123, true);
        let (price, size, user_index, is_buy, is_active) = packed.unpack();
        
        assert_eq!(price, 525_000);
        assert_eq!(size, 1_000_000);
        assert_eq!(user_index, 123);
        assert_eq!(is_buy, true);
        assert_eq!(is_active, true);
    }
}
