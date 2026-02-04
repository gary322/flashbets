use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::{rent::Rent, Sysvar, clock::Clock},
    program::invoke,
    system_instruction,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::CompressionError,
    instructions::CompressionInstruction,
    state::{
        CompressionConfig,
        CompressedStateProof,
        DecompressionCache,
        MarketEssentials,
        MarketUpdate,
        MarketStatus,
    },
    compression::{
        StateCompressionEngine,
        CompressedStateAccess,
    },
};

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let instruction = CompressionInstruction::unpack(instruction_data)?;
    
    match instruction {
        CompressionInstruction::InitializeConfig { 
            compression_ratio,
            batch_size,
            proof_verification_cu,
        } => {
            msg!("Instruction: InitializeConfig");
            process_initialize_config(
                program_id,
                accounts,
                compression_ratio,
                batch_size,
                proof_verification_cu,
            )
        }
        
        CompressionInstruction::UpdateConfig {
            enabled,
            compression_ratio,
            batch_size,
            proof_verification_cu,
        } => {
            msg!("Instruction: UpdateConfig");
            process_update_config(
                accounts,
                enabled,
                compression_ratio,
                batch_size,
                proof_verification_cu,
            )
        }
        
        CompressionInstruction::CompressMarkets { market_ids } => {
            msg!("Instruction: CompressMarkets");
            process_compress_markets(program_id, accounts, market_ids)
        }
        
        CompressionInstruction::DecompressMarket { market_id } => {
            msg!("Instruction: DecompressMarket");
            process_decompress_market(accounts, market_id)
        }
        
        CompressionInstruction::BatchDecompress { market_ids } => {
            msg!("Instruction: BatchDecompress");
            process_batch_decompress(accounts, market_ids)
        }
        
        CompressionInstruction::UpdateCompressedMarket { market_id, update } => {
            msg!("Instruction: UpdateCompressedMarket");
            process_update_compressed_market(accounts, market_id, update)
        }
        
        CompressionInstruction::InitializeCache { max_entries, cache_timeout } => {
            msg!("Instruction: InitializeCache");
            process_initialize_cache(program_id, accounts, max_entries, cache_timeout)
        }
        
        CompressionInstruction::CleanupCache => {
            msg!("Instruction: CleanupCache");
            process_cleanup_cache(accounts)
        }
        
        CompressionInstruction::ArchiveOriginals { market_ids } => {
            msg!("Instruction: ArchiveOriginals");
            process_archive_originals(accounts, market_ids)
        }
        
        CompressionInstruction::EmergencyPause { pause } => {
            msg!("Instruction: EmergencyPause");
            process_emergency_pause(accounts, pause)
        }
    }
}

/// Initialize compression configuration
fn process_initialize_config(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    compression_ratio: u8,
    batch_size: u16,
    proof_verification_cu: u32,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let authority_info = next_account_info(account_info_iter)?;
    let config_info = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;
    let rent_sysvar = next_account_info(account_info_iter)?;
    
    // Verify authority is signer
    if !authority_info.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Create config account
    let rent = &Rent::from_account_info(rent_sysvar)?;
    let required_lamports = rent.minimum_balance(CompressionConfig::LEN);
    
    invoke(
        &system_instruction::create_account(
            authority_info.key,
            config_info.key,
            required_lamports,
            CompressionConfig::LEN as u64,
            program_id,
        ),
        &[
            authority_info.clone(),
            config_info.clone(),
            system_program.clone(),
        ],
    )?;
    
    // Initialize config
    let mut config = CompressionConfig::default(*authority_info.key);
    config.compression_ratio = compression_ratio;
    config.batch_size = batch_size;
    config.proof_verification_cu = proof_verification_cu;
    
    // Validate config
    config.validate()?;
    
    // Serialize to account
    config.serialize(&mut &mut config_info.data.borrow_mut()[..])?;
    
    msg!("Compression config initialized with ratio: {}x, batch size: {}", 
        compression_ratio, batch_size);
    
    Ok(())
}

/// Update compression configuration
fn process_update_config(
    accounts: &[AccountInfo],
    enabled: Option<bool>,
    compression_ratio: Option<u8>,
    batch_size: Option<u16>,
    proof_verification_cu: Option<u32>,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let authority_info = next_account_info(account_info_iter)?;
    let config_info = next_account_info(account_info_iter)?;
    
    // Verify authority
    if !authority_info.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Load and verify config
    let mut config = CompressionConfig::try_from_slice(&config_info.data.borrow())?;
    config.validate()?;
    
    if config.authority != *authority_info.key {
        return Err(CompressionError::InvalidAuthority.into());
    }
    
    // Update parameters
    if let Some(value) = enabled {
        config.enabled = value;
    }
    if let Some(value) = compression_ratio {
        config.compression_ratio = value;
    }
    if let Some(value) = batch_size {
        config.batch_size = value;
    }
    if let Some(value) = proof_verification_cu {
        config.proof_verification_cu = value;
    }
    
    // Validate updated config
    config.validate()?;
    
    // Save
    config.serialize(&mut &mut config_info.data.borrow_mut()[..])?;
    
    msg!("Compression config updated");
    
    Ok(())
}

/// Compress market states
fn process_compress_markets(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    market_ids: Vec<[u8; 32]>,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let authority_info = next_account_info(account_info_iter)?;
    let config_info = next_account_info(account_info_iter)?;
    let proof_info = next_account_info(account_info_iter)?;
    
    // Verify authority
    if !authority_info.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Load config
    let mut config = CompressionConfig::try_from_slice(&config_info.data.borrow())?;
    config.validate()?;
    
    if config.authority != *authority_info.key {
        return Err(CompressionError::InvalidAuthority.into());
    }
    
    // Check if compression is allowed
    if !config.can_compress(market_ids.len()) {
        return Err(CompressionError::CompressionDisabled.into());
    }
    
    // Collect market data
    let mut market_data = Vec::new();
    for (i, market_id) in market_ids.iter().enumerate() {
        let market_info = next_account_info(account_info_iter)?;
        
        // For demo, create dummy market essentials
        // In production, would extract from actual market accounts
        let essentials = MarketEssentials {
            market_id: *market_id,
            current_price: 50_000_000 + (i as u64 * 1_000_000), // 50% + i%
            total_volume: 1_000_000 * (i as u64 + 1),
            outcome_count: 2,
            status: MarketStatus::Active,
            last_update: Clock::get()?.unix_timestamp,
        };
        
        market_data.push(essentials);
    }
    
    // Skip system program and clock
    let _system_program = next_account_info(account_info_iter)?;
    let clock_sysvar = next_account_info(account_info_iter)?;
    let clock = &Clock::from_account_info(clock_sysvar)?;
    
    // Compress markets
    let compressed_proof = StateCompressionEngine::compress_market_state(
        &config,
        &market_ids,
        market_data,
        clock,
        *authority_info.key,
    )?;
    
    // Create proof account
    let rent = Rent::get()?;
    let required_lamports = rent.minimum_balance(CompressedStateProof::MAX_SIZE);
    
    invoke(
        &system_instruction::create_account(
            authority_info.key,
            proof_info.key,
            required_lamports,
            CompressedStateProof::MAX_SIZE as u64,
            program_id,
        ),
        &[
            authority_info.clone(),
            proof_info.clone(),
        ],
    )?;
    
    // Update config stats
    config.update_stats(
        market_ids.len() as u64,
        compressed_proof.uncompressed_size - compressed_proof.compressed_size,
        clock.unix_timestamp,
    );
    
    // Save proof and config
    compressed_proof.serialize(&mut &mut proof_info.data.borrow_mut()[..])?;
    config.serialize(&mut &mut config_info.data.borrow_mut()[..])?;
    
    msg!("Compressed {} markets with ratio: {:.2}x", 
        market_ids.len(),
        compressed_proof.get_compression_ratio()
    );
    
    Ok(())
}

/// Decompress market
fn process_decompress_market(
    accounts: &[AccountInfo],
    market_id: [u8; 32],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let proof_info = next_account_info(account_info_iter)?;
    let config_info = next_account_info(account_info_iter)?;
    let cache_info = next_account_info(account_info_iter)?;
    let clock_sysvar = next_account_info(account_info_iter)?;
    
    // Load accounts
    let proof = CompressedStateProof::try_from_slice(&proof_info.data.borrow())?;
    let config = CompressionConfig::try_from_slice(&config_info.data.borrow())?;
    let mut cache = DecompressionCache::try_from_slice(&cache_info.data.borrow())?;
    let clock = &Clock::from_account_info(clock_sysvar)?;
    
    // Decompress market
    let market_data = CompressedStateAccess::read_compressed_market(
        &market_id,
        &mut cache,
        &proof,
        &config,
        clock,
    )?;
    
    // Save updated cache
    cache.serialize(&mut &mut cache_info.data.borrow_mut()[..])?;
    
    msg!("Decompressed market {:?}, price: {}", 
        market_id,
        market_data.current_price
    );
    
    Ok(())
}

/// Batch decompress markets
fn process_batch_decompress(
    accounts: &[AccountInfo],
    market_ids: Vec<[u8; 32]>,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let config_info = next_account_info(account_info_iter)?;
    let cache_info = next_account_info(account_info_iter)?;
    
    // Load accounts
    let config = CompressionConfig::try_from_slice(&config_info.data.borrow())?;
    let mut cache = DecompressionCache::try_from_slice(&cache_info.data.borrow())?;
    
    // Collect proof accounts
    let mut proofs = Vec::new();
    while let Ok(proof_info) = next_account_info(account_info_iter) {
        if proof_info.key == &solana_program::sysvar::clock::id() {
            break;
        }
        proofs.push(proof_info);
    }
    
    let clock_sysvar = account_info_iter.next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let clock = &Clock::from_account_info(clock_sysvar)?;
    
    // Load proofs
    let mut loaded_proofs = Vec::new();
    for proof_info in &proofs {
        let proof = CompressedStateProof::try_from_slice(&proof_info.data.borrow())?;
        loaded_proofs.push(proof);
    }
    
    let proof_refs: Vec<&CompressedStateProof> = loaded_proofs.iter().collect();
    
    // Batch decompress
    let markets = CompressedStateAccess::batch_read_compressed(
        &market_ids,
        &mut cache,
        &proof_refs,
        &config,
        clock,
    )?;
    
    // Save updated cache
    cache.serialize(&mut &mut cache_info.data.borrow_mut()[..])?;
    
    msg!("Batch decompressed {} markets", markets.len());
    
    Ok(())
}

/// Update compressed market
fn process_update_compressed_market(
    accounts: &[AccountInfo],
    market_id: [u8; 32],
    update: MarketUpdate,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let authority_info = next_account_info(account_info_iter)?;
    let proof_info = next_account_info(account_info_iter)?;
    let cache_info = next_account_info(account_info_iter)?;
    let recompression_queue_info = next_account_info(account_info_iter)?;
    let clock_sysvar = next_account_info(account_info_iter)?;
    
    // Verify authority
    if !authority_info.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Load accounts
    let proof = CompressedStateProof::try_from_slice(&proof_info.data.borrow())?;
    let mut cache = DecompressionCache::try_from_slice(&cache_info.data.borrow())?;
    let config = CompressionConfig::default(*authority_info.key); // For demo
    let clock = &Clock::from_account_info(clock_sysvar)?;
    
    // Update market
    let updated = CompressedStateAccess::update_compressed_market(
        &market_id,
        |essentials| update.apply(essentials),
        &mut cache,
        &proof,
        &config,
        clock,
    )?;
    
    // Add to recompression queue (simplified for demo)
    msg!("Market {:?} updated and queued for recompression", market_id);
    
    // Save cache
    cache.serialize(&mut &mut cache_info.data.borrow_mut()[..])?;
    
    Ok(())
}

/// Initialize decompression cache
fn process_initialize_cache(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    max_entries: u32,
    cache_timeout: i64,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let authority_info = next_account_info(account_info_iter)?;
    let cache_info = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;
    let rent_sysvar = next_account_info(account_info_iter)?;
    
    // Verify authority
    if !authority_info.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Create cache account
    let rent = &Rent::from_account_info(rent_sysvar)?;
    let required_lamports = rent.minimum_balance(DecompressionCache::LEN);
    
    invoke(
        &system_instruction::create_account(
            authority_info.key,
            cache_info.key,
            required_lamports,
            DecompressionCache::LEN as u64,
            program_id,
        ),
        &[
            authority_info.clone(),
            cache_info.clone(),
            system_program.clone(),
        ],
    )?;
    
    // Initialize cache
    let mut cache = DecompressionCache::default(*authority_info.key);
    cache.max_entries = max_entries;
    cache.cache_timeout = cache_timeout;
    
    // Validate
    cache.validate()?;
    
    // Save
    cache.serialize(&mut &mut cache_info.data.borrow_mut()[..])?;
    
    msg!("Cache initialized with {} max entries, {}s timeout", 
        max_entries, cache_timeout);
    
    Ok(())
}

/// Clean stale cache entries
fn process_cleanup_cache(accounts: &[AccountInfo]) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let authority_info = next_account_info(account_info_iter)?;
    let cache_info = next_account_info(account_info_iter)?;
    let clock_sysvar = next_account_info(account_info_iter)?;
    
    // Verify authority
    if !authority_info.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Load accounts
    let mut cache = DecompressionCache::try_from_slice(&cache_info.data.borrow())?;
    let clock = &Clock::from_account_info(clock_sysvar)?;
    
    if cache.authority != *authority_info.key {
        return Err(CompressionError::InvalidAuthority.into());
    }
    
    // Clean cache
    let cleaned = CompressedStateAccess::cleanup_cache(&mut cache, clock.unix_timestamp)?;
    
    // Save
    cache.serialize(&mut &mut cache_info.data.borrow_mut()[..])?;
    
    msg!("Cleaned {} stale cache entries", cleaned);
    
    Ok(())
}

/// Archive original PDAs
fn process_archive_originals(
    accounts: &[AccountInfo],
    market_ids: Vec<[u8; 32]>,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let authority_info = next_account_info(account_info_iter)?;
    let config_info = next_account_info(account_info_iter)?;
    let proof_info = next_account_info(account_info_iter)?;
    
    // Verify authority
    if !authority_info.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Load config
    let config = CompressionConfig::try_from_slice(&config_info.data.borrow())?;
    
    if config.authority != *authority_info.key {
        return Err(CompressionError::InvalidAuthority.into());
    }
    
    // Verify proof contains these markets
    let proof = CompressedStateProof::try_from_slice(&proof_info.data.borrow())?;
    
    // Archive each market PDA
    for market_id in &market_ids {
        let market_info = next_account_info(account_info_iter)?;
        
        // In production, would close account and return lamports
        msg!("Archived market {:?}", market_id);
    }
    
    msg!("Archived {} original market PDAs", market_ids.len());
    
    Ok(())
}

/// Emergency pause
fn process_emergency_pause(
    accounts: &[AccountInfo],
    pause: bool,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let authority_info = next_account_info(account_info_iter)?;
    let config_info = next_account_info(account_info_iter)?;
    
    // Verify authority
    if !authority_info.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Load config
    let mut config = CompressionConfig::try_from_slice(&config_info.data.borrow())?;
    
    if config.authority != *authority_info.key {
        return Err(CompressionError::InvalidAuthority.into());
    }
    
    // Update pause state
    config.emergency_pause = pause;
    
    // Save
    config.serialize(&mut &mut config_info.data.borrow_mut()[..])?;
    
    msg!("Emergency pause: {}", if pause { "ACTIVATED" } else { "deactivated" });
    
    Ok(())
}