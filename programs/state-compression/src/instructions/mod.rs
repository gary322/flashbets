use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    instruction::{AccountMeta, Instruction},
    program_error::ProgramError,
    pubkey::Pubkey,
};

use crate::state::{MarketEssentials, MarketUpdate};

/// State compression program instructions
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub enum CompressionInstruction {
    /// Initialize compression configuration
    /// 
    /// Accounts:
    /// 0. `[signer]` Authority
    /// 1. `[writable]` Compression config account
    /// 2. `[]` System program
    /// 3. `[]` Rent sysvar
    InitializeConfig {
        compression_ratio: u8,
        batch_size: u16,
        proof_verification_cu: u32,
    },
    
    /// Update compression configuration
    /// 
    /// Accounts:
    /// 0. `[signer]` Authority
    /// 1. `[writable]` Compression config account
    UpdateConfig {
        enabled: Option<bool>,
        compression_ratio: Option<u8>,
        batch_size: Option<u16>,
        proof_verification_cu: Option<u32>,
    },
    
    /// Compress market states
    /// 
    /// Accounts:
    /// 0. `[signer]` Authority
    /// 1. `[writable]` Compression config account
    /// 2. `[writable]` Compressed state proof account
    /// 3..N. `[]` Market accounts to compress
    /// N+1. `[]` System program
    /// N+2. `[]` Clock sysvar
    CompressMarkets {
        market_ids: Vec<[u8; 32]>,
    },
    
    /// Decompress and verify state
    /// 
    /// Accounts:
    /// 0. `[]` Compressed state proof account
    /// 1. `[]` Compression config account
    /// 2. `[writable]` Decompression cache account
    /// 3. `[]` Clock sysvar
    DecompressMarket {
        market_id: [u8; 32],
    },
    
    /// Batch decompress markets
    /// 
    /// Accounts:
    /// 0. `[]` Compression config account
    /// 1. `[writable]` Decompression cache account
    /// 2..N. `[]` Compressed state proof accounts
    /// N+1. `[]` Clock sysvar
    BatchDecompress {
        market_ids: Vec<[u8; 32]>,
    },
    
    /// Update compressed market
    /// 
    /// Accounts:
    /// 0. `[signer]` Authority
    /// 1. `[]` Compressed state proof account
    /// 2. `[writable]` Decompression cache account
    /// 3. `[writable]` Recompression queue account
    /// 4. `[]` Clock sysvar
    UpdateCompressedMarket {
        market_id: [u8; 32],
        update: MarketUpdate,
    },
    
    /// Initialize decompression cache
    /// 
    /// Accounts:
    /// 0. `[signer]` Authority
    /// 1. `[writable]` Cache account
    /// 2. `[]` System program
    /// 3. `[]` Rent sysvar
    InitializeCache {
        max_entries: u32,
        cache_timeout: i64,
    },
    
    /// Clean stale cache entries
    /// 
    /// Accounts:
    /// 0. `[signer]` Authority
    /// 1. `[writable]` Cache account
    /// 2. `[]` Clock sysvar
    CleanupCache,
    
    /// Archive original PDAs after compression
    /// 
    /// Accounts:
    /// 0. `[signer]` Authority
    /// 1. `[]` Compression config account
    /// 2. `[]` Compressed state proof account
    /// 3..N. `[writable]` Original market PDAs to archive
    ArchiveOriginals {
        market_ids: Vec<[u8; 32]>,
    },
    
    /// Emergency pause compression
    /// 
    /// Accounts:
    /// 0. `[signer]` Authority
    /// 1. `[writable]` Compression config account
    EmergencyPause {
        pause: bool,
    },
}

impl CompressionInstruction {
    /// Unpack instruction from bytes
    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        Self::try_from_slice(input)
            .map_err(|_| ProgramError::InvalidInstructionData)
    }
    
    /// Pack instruction to bytes
    pub fn pack(&self) -> Vec<u8> {
        self.try_to_vec().unwrap()
    }
}

/// Create initialize config instruction
pub fn initialize_config(
    program_id: &Pubkey,
    authority: &Pubkey,
    config_account: &Pubkey,
    compression_ratio: u8,
    batch_size: u16,
    proof_verification_cu: u32,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new(*authority, true),
        AccountMeta::new(*config_account, false),
        AccountMeta::new_readonly(solana_program::system_program::id(), false),
        AccountMeta::new_readonly(solana_program::sysvar::rent::id(), false),
    ];
    
    let data = CompressionInstruction::InitializeConfig {
        compression_ratio,
        batch_size,
        proof_verification_cu,
    }.pack();
    
    Instruction {
        program_id: *program_id,
        accounts,
        data,
    }
}

/// Create compress markets instruction
pub fn compress_markets(
    program_id: &Pubkey,
    authority: &Pubkey,
    config_account: &Pubkey,
    proof_account: &Pubkey,
    market_accounts: &[Pubkey],
    market_ids: Vec<[u8; 32]>,
) -> Instruction {
    let mut accounts = vec![
        AccountMeta::new(*authority, true),
        AccountMeta::new(*config_account, false),
        AccountMeta::new(*proof_account, false),
    ];
    
    // Add market accounts
    for market in market_accounts {
        accounts.push(AccountMeta::new_readonly(*market, false));
    }
    
    accounts.push(AccountMeta::new_readonly(solana_program::system_program::id(), false));
    accounts.push(AccountMeta::new_readonly(solana_program::sysvar::clock::id(), false));
    
    let data = CompressionInstruction::CompressMarkets { market_ids }.pack();
    
    Instruction {
        program_id: *program_id,
        accounts,
        data,
    }
}

/// Create decompress market instruction
pub fn decompress_market(
    program_id: &Pubkey,
    proof_account: &Pubkey,
    config_account: &Pubkey,
    cache_account: &Pubkey,
    market_id: [u8; 32],
) -> Instruction {
    let accounts = vec![
        AccountMeta::new_readonly(*proof_account, false),
        AccountMeta::new_readonly(*config_account, false),
        AccountMeta::new(*cache_account, false),
        AccountMeta::new_readonly(solana_program::sysvar::clock::id(), false),
    ];
    
    let data = CompressionInstruction::DecompressMarket { market_id }.pack();
    
    Instruction {
        program_id: *program_id,
        accounts,
        data,
    }
}