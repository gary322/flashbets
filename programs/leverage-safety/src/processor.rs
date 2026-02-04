use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::{rent::Rent, Sysvar, clock::Clock},
    program::{invoke, invoke_signed},
    system_instruction,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::LeverageSafetyError,
    instructions::LeverageSafetyInstruction,
    state::{
        LeverageSafetyConfig, PositionHealth, LiquidationQueue,
        TierCap, ChainStepType,
    },
    engine::{LeverageSafetyEngine, ONE},
};

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let instruction = LeverageSafetyInstruction::unpack(instruction_data)?;
    
    match instruction {
        LeverageSafetyInstruction::InitializeSafetyConfig { 
            max_base_leverage,
            max_effective_leverage,
        } => {
            msg!("Instruction: InitializeSafetyConfig");
            process_initialize_safety_config(
                program_id,
                accounts,
                max_base_leverage,
                max_effective_leverage,
            )
        }
        
        LeverageSafetyInstruction::UpdateSafetyParameters {
            max_base_leverage,
            max_effective_leverage,
            chain_depth_multiplier,
            coverage_minimum,
            correlation_penalty,
            volatility_adjustment,
        } => {
            msg!("Instruction: UpdateSafetyParameters");
            process_update_safety_parameters(
                accounts,
                max_base_leverage,
                max_effective_leverage,
                chain_depth_multiplier,
                coverage_minimum,
                correlation_penalty,
                volatility_adjustment,
            )
        }
        
        LeverageSafetyInstruction::UpdateLiquidationParameters {
            partial_liq_percent,
            liq_buffer_bps,
            min_health_ratio,
            liquidation_fee_bps,
            liquidation_cooldown,
        } => {
            msg!("Instruction: UpdateLiquidationParameters");
            process_update_liquidation_parameters(
                accounts,
                partial_liq_percent,
                liq_buffer_bps,
                min_health_ratio,
                liquidation_fee_bps,
                liquidation_cooldown,
            )
        }
        
        LeverageSafetyInstruction::InitializePositionHealth {
            position_id,
            market_id,
            trader,
            entry_price,
            side,
            base_leverage,
        } => {
            msg!("Instruction: InitializePositionHealth");
            process_initialize_position_health(
                program_id,
                accounts,
                position_id,
                market_id,
                trader,
                entry_price,
                side,
                base_leverage,
            )
        }
        
        LeverageSafetyInstruction::MonitorPosition {
            current_price,
            price_staleness_threshold,
        } => {
            msg!("Instruction: MonitorPosition");
            process_monitor_position(
                accounts,
                current_price,
                price_staleness_threshold,
            )
        }
        
        LeverageSafetyInstruction::AddChainStep { step_type } => {
            msg!("Instruction: AddChainStep");
            process_add_chain_step(accounts, step_type)
        }
        
        LeverageSafetyInstruction::ProcessPartialLiquidation { liquidation_amount } => {
            msg!("Instruction: ProcessPartialLiquidation");
            process_partial_liquidation(accounts, liquidation_amount)
        }
        
        LeverageSafetyInstruction::InitializeLiquidationQueue => {
            msg!("Instruction: InitializeLiquidationQueue");
            process_initialize_liquidation_queue(program_id, accounts)
        }
        
        LeverageSafetyInstruction::ToggleEmergencyHalt { halt } => {
            msg!("Instruction: ToggleEmergencyHalt");
            process_toggle_emergency_halt(accounts, halt)
        }
        
        LeverageSafetyInstruction::UpdateTierCaps { tier_caps } => {
            msg!("Instruction: UpdateTierCaps");
            process_update_tier_caps(accounts, tier_caps)
        }
    }
}

/// Initialize safety configuration
fn process_initialize_safety_config(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    max_base_leverage: u64,
    max_effective_leverage: u64,
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
    let required_lamports = rent.minimum_balance(LeverageSafetyConfig::LEN);
    
    invoke(
        &system_instruction::create_account(
            authority_info.key,
            config_info.key,
            required_lamports,
            LeverageSafetyConfig::LEN as u64,
            program_id,
        ),
        &[
            authority_info.clone(),
            config_info.clone(),
            system_program.clone(),
        ],
    )?;
    
    // Initialize config
    let mut config = LeverageSafetyConfig::default(*authority_info.key);
    config.max_base_leverage = max_base_leverage;
    config.max_effective_leverage = max_effective_leverage;
    config.last_update = Clock::get()?.unix_timestamp;
    
    // Validate config
    config.validate()?;
    
    // Serialize to account
    config.serialize(&mut &mut config_info.data.borrow_mut()[..])?;
    
    msg!("Safety config initialized with max base leverage: {}x, max effective: {}x", 
        max_base_leverage, max_effective_leverage);
    
    Ok(())
}

/// Update safety parameters
fn process_update_safety_parameters(
    accounts: &[AccountInfo],
    max_base_leverage: Option<u64>,
    max_effective_leverage: Option<u64>,
    chain_depth_multiplier: Option<u64>,
    coverage_minimum: Option<u64>,
    correlation_penalty: Option<u64>,
    volatility_adjustment: Option<bool>,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let authority_info = next_account_info(account_info_iter)?;
    let config_info = next_account_info(account_info_iter)?;
    
    // Verify authority
    if !authority_info.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Load and verify config
    let mut config = LeverageSafetyConfig::try_from_slice(&config_info.data.borrow())?;
    config.validate()?;
    
    if config.authority != *authority_info.key {
        return Err(LeverageSafetyError::InvalidAuthority.into());
    }
    
    // Check emergency halt
    if config.emergency_halt {
        return Err(LeverageSafetyError::EmergencyHaltActive.into());
    }
    
    // Update parameters
    if let Some(value) = max_base_leverage {
        config.max_base_leverage = value;
    }
    if let Some(value) = max_effective_leverage {
        config.max_effective_leverage = value;
    }
    if let Some(value) = chain_depth_multiplier {
        config.chain_depth_multiplier = value;
    }
    if let Some(value) = coverage_minimum {
        config.coverage_minimum = value;
    }
    if let Some(value) = correlation_penalty {
        config.correlation_penalty = value;
    }
    if let Some(value) = volatility_adjustment {
        config.volatility_adjustment = value;
    }
    
    config.last_update = Clock::get()?.unix_timestamp;
    
    // Validate updated config
    config.validate()?;
    
    // Save
    config.serialize(&mut &mut config_info.data.borrow_mut()[..])?;
    
    msg!("Safety parameters updated");
    
    Ok(())
}

/// Update liquidation parameters
fn process_update_liquidation_parameters(
    accounts: &[AccountInfo],
    partial_liq_percent: Option<u16>,
    liq_buffer_bps: Option<u16>,
    min_health_ratio: Option<u64>,
    liquidation_fee_bps: Option<u16>,
    liquidation_cooldown: Option<u64>,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let authority_info = next_account_info(account_info_iter)?;
    let config_info = next_account_info(account_info_iter)?;
    
    // Verify authority
    if !authority_info.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Load config
    let mut config = LeverageSafetyConfig::try_from_slice(&config_info.data.borrow())?;
    config.validate()?;
    
    if config.authority != *authority_info.key {
        return Err(LeverageSafetyError::InvalidAuthority.into());
    }
    
    // Update parameters
    if let Some(value) = partial_liq_percent {
        config.liquidation_params.partial_liq_percent = value;
    }
    if let Some(value) = liq_buffer_bps {
        config.liquidation_params.liq_buffer_bps = value;
    }
    if let Some(value) = min_health_ratio {
        config.liquidation_params.min_health_ratio = value;
    }
    if let Some(value) = liquidation_fee_bps {
        config.liquidation_params.liquidation_fee_bps = value;
    }
    if let Some(value) = liquidation_cooldown {
        config.liquidation_params.liquidation_cooldown = value;
    }
    
    config.last_update = Clock::get()?.unix_timestamp;
    
    // Save
    config.serialize(&mut &mut config_info.data.borrow_mut()[..])?;
    
    msg!("Liquidation parameters updated");
    
    Ok(())
}

/// Initialize position health tracking
fn process_initialize_position_health(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    position_id: [u8; 32],
    market_id: [u8; 32],
    trader: Pubkey,
    entry_price: u64,
    side: bool,
    base_leverage: u64,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let payer_info = next_account_info(account_info_iter)?;
    let position_health_info = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;
    let rent_sysvar = next_account_info(account_info_iter)?;
    
    // Verify payer is signer
    if !payer_info.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Create position health account
    let rent = &Rent::from_account_info(rent_sysvar)?;
    let required_lamports = rent.minimum_balance(PositionHealth::LEN);
    
    invoke(
        &system_instruction::create_account(
            payer_info.key,
            position_health_info.key,
            required_lamports,
            PositionHealth::LEN as u64,
            program_id,
        ),
        &[
            payer_info.clone(),
            position_health_info.clone(),
            system_program.clone(),
        ],
    )?;
    
    // Initialize position health
    let mut position_health = PositionHealth::new(
        position_id,
        market_id,
        trader,
        entry_price,
        side,
        base_leverage,
    );
    
    // Calculate initial liquidation price
    position_health.calculate_liquidation_price()?;
    
    // Serialize
    position_health.serialize(&mut &mut position_health_info.data.borrow_mut()[..])?;
    
    msg!("Position health initialized for position: {:?}", position_id);
    
    Ok(())
}

/// Monitor high leverage position
fn process_monitor_position(
    accounts: &[AccountInfo],
    current_price: u64,
    price_staleness_threshold: i64,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let monitor_authority = next_account_info(account_info_iter)?;
    let config_info = next_account_info(account_info_iter)?;
    let position_health_info = next_account_info(account_info_iter)?;
    let liquidation_queue_info = next_account_info(account_info_iter)?; // Optional
    let clock_sysvar = next_account_info(account_info_iter)?;
    
    // Verify monitor authority
    if !monitor_authority.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Load accounts
    let config = LeverageSafetyConfig::try_from_slice(&config_info.data.borrow())?;
    config.validate()?;
    
    let mut position_health = PositionHealth::try_from_slice(&position_health_info.data.borrow())?;
    
    // Get clock
    let clock = &Clock::from_account_info(clock_sysvar)?;
    
    // Monitor position
    let result = LeverageSafetyEngine::monitor_high_leverage_position(
        &config,
        &mut position_health,
        current_price,
        clock,
        price_staleness_threshold,
    )?;
    
    // Update stats
    let mut config_mut = LeverageSafetyConfig::try_from_slice(&config_info.data.borrow())?;
    config_mut.total_positions_monitored += 1;
    
    if result.warning_issued {
        config_mut.total_warnings_issued += 1;
    }
    
    // Handle liquidation queue if needed
    if result.add_to_queue && liquidation_queue_info.key != &Pubkey::default() {
        let mut queue = LiquidationQueue::try_from_slice(&liquidation_queue_info.data.borrow())?;
        
        if result.health_ratio < 1_050_000 { // Critical
            queue.add_high_priority(
                position_health.position_id,
                *position_health_info.key,
                position_health.trader,
                result.health_ratio,
                result.effective_leverage,
                clock.slot,
                clock.unix_timestamp,
            )?;
        } else {
            queue.add_medium_priority(
                position_health.position_id,
                *position_health_info.key,
                position_health.trader,
                result.health_ratio,
                result.effective_leverage,
                clock.slot,
                clock.unix_timestamp,
            )?;
        }
        
        queue.serialize(&mut &mut liquidation_queue_info.data.borrow_mut()[..])?;
    }
    
    // Save updates
    position_health.serialize(&mut &mut position_health_info.data.borrow_mut()[..])?;
    config_mut.serialize(&mut &mut config_info.data.borrow_mut()[..])?;
    
    msg!("Position monitored - Health: {}, Leverage: {}x, Needs liquidation: {}", 
        result.health_ratio,
        result.effective_leverage / ONE,
        result.needs_liquidation
    );
    
    Ok(())
}

/// Add chain step to position
fn process_add_chain_step(
    accounts: &[AccountInfo],
    step_type: u8,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let authority_info = next_account_info(account_info_iter)?;
    let config_info = next_account_info(account_info_iter)?;
    let position_health_info = next_account_info(account_info_iter)?;
    let clock_sysvar = next_account_info(account_info_iter)?;
    
    // Verify authority
    if !authority_info.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Load accounts
    let config = LeverageSafetyConfig::try_from_slice(&config_info.data.borrow())?;
    let mut position_health = PositionHealth::try_from_slice(&position_health_info.data.borrow())?;
    
    // Verify authority is trader or config authority
    if *authority_info.key != position_health.trader && *authority_info.key != config.authority {
        return Err(LeverageSafetyError::InvalidAuthority.into());
    }
    
    // Convert step type
    let chain_step_type = match step_type {
        0 => ChainStepType::Borrow,
        1 => ChainStepType::Liquidity,
        2 => ChainStepType::Stake,
        _ => return Err(ProgramError::InvalidInstructionData),
    };
    
    // Get current slot
    let clock = &Clock::from_account_info(clock_sysvar)?;
    
    // Add chain step
    position_health.add_chain_step(chain_step_type, clock.slot)?;
    
    // Check if leverage exceeds maximum
    if position_health.effective_leverage > config.max_effective_leverage * ONE {
        return Err(LeverageSafetyError::MaxLeverageExceeded.into());
    }
    
    // Recalculate health metrics
    position_health.calculate_liquidation_price()?;
    
    // Save
    position_health.serialize(&mut &mut position_health_info.data.borrow_mut()[..])?;
    
    msg!("Chain step added: {:?}, new effective leverage: {}x", 
        chain_step_type,
        position_health.effective_leverage / ONE
    );
    
    Ok(())
}

/// Process partial liquidation
fn process_partial_liquidation(
    accounts: &[AccountInfo],
    liquidation_amount: u64,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let liquidator_info = next_account_info(account_info_iter)?;
    let config_info = next_account_info(account_info_iter)?;
    let position_health_info = next_account_info(account_info_iter)?;
    let liquidation_queue_info = next_account_info(account_info_iter)?;
    let clock_sysvar = next_account_info(account_info_iter)?;
    
    // Verify liquidator
    if !liquidator_info.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Load accounts
    let mut config = LeverageSafetyConfig::try_from_slice(&config_info.data.borrow())?;
    let mut position_health = PositionHealth::try_from_slice(&position_health_info.data.borrow())?;
    let mut queue = LiquidationQueue::try_from_slice(&liquidation_queue_info.data.borrow())?;
    
    // Check emergency halt
    if config.emergency_halt {
        return Err(LeverageSafetyError::EmergencyHaltActive.into());
    }
    
    // Verify position needs liquidation
    if !position_health.should_liquidate() {
        return Err(LeverageSafetyError::PositionHealthCritical.into());
    }
    
    // Get clock
    let clock = &Clock::from_account_info(clock_sysvar)?;
    
    // Check cooldown
    if position_health.partial_liquidations > 0 {
        let last_liquidation_slot = position_health.last_check_slot;
        if clock.slot < last_liquidation_slot + config.liquidation_params.liquidation_cooldown {
            return Err(ProgramError::InvalidInstructionData);
        }
    }
    
    // Calculate max liquidation amount (8% of position)
    // Note: In real implementation, would need actual position size from trading program
    let max_liquidation = liquidation_amount; // Simplified for now
    
    if liquidation_amount > max_liquidation {
        return Err(LeverageSafetyError::LiquidationAmountExceedsLimit.into());
    }
    
    // Update position health
    position_health.partial_liquidations += 1;
    position_health.total_liquidated += liquidation_amount;
    position_health.last_check_slot = clock.slot;
    position_health.last_check_timestamp = clock.unix_timestamp;
    
    // Update queue stats
    queue.update_stats(clock.slot, true);
    
    // Update config stats
    config.total_liquidations += 1;
    
    // Remove from queue if fully liquidated
    if position_health.health_ratio < 500_000 { // Very low health
        queue.remove_position(&position_health.position_id);
    }
    
    // Save all updates
    position_health.serialize(&mut &mut position_health_info.data.borrow_mut()[..])?;
    queue.serialize(&mut &mut liquidation_queue_info.data.borrow_mut()[..])?;
    config.serialize(&mut &mut config_info.data.borrow_mut()[..])?;
    
    msg!("Partial liquidation processed: {} units", liquidation_amount);
    
    Ok(())
}

/// Initialize liquidation queue
fn process_initialize_liquidation_queue(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let authority_info = next_account_info(account_info_iter)?;
    let queue_info = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;
    let rent_sysvar = next_account_info(account_info_iter)?;
    
    // Verify authority
    if !authority_info.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Create queue account
    let rent = &Rent::from_account_info(rent_sysvar)?;
    let required_lamports = rent.minimum_balance(LiquidationQueue::LEN);
    
    invoke(
        &system_instruction::create_account(
            authority_info.key,
            queue_info.key,
            required_lamports,
            LiquidationQueue::LEN as u64,
            program_id,
        ),
        &[
            authority_info.clone(),
            queue_info.clone(),
            system_program.clone(),
        ],
    )?;
    
    // Initialize queue
    let queue = LiquidationQueue::new(*authority_info.key);
    queue.serialize(&mut &mut queue_info.data.borrow_mut()[..])?;
    
    msg!("Liquidation queue initialized");
    
    Ok(())
}

/// Toggle emergency halt
fn process_toggle_emergency_halt(
    accounts: &[AccountInfo],
    halt: bool,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let authority_info = next_account_info(account_info_iter)?;
    let config_info = next_account_info(account_info_iter)?;
    
    // Verify authority
    if !authority_info.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Load config
    let mut config = LeverageSafetyConfig::try_from_slice(&config_info.data.borrow())?;
    
    if config.authority != *authority_info.key {
        return Err(LeverageSafetyError::InvalidAuthority.into());
    }
    
    // Update halt status
    config.emergency_halt = halt;
    config.last_update = Clock::get()?.unix_timestamp;
    
    // Save
    config.serialize(&mut &mut config_info.data.borrow_mut()[..])?;
    
    msg!("Emergency halt {}", if halt { "ACTIVATED" } else { "deactivated" });
    
    Ok(())
}

/// Update tier caps
fn process_update_tier_caps(
    accounts: &[AccountInfo],
    tier_caps: Vec<(u8, u8, u64)>,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let authority_info = next_account_info(account_info_iter)?;
    let config_info = next_account_info(account_info_iter)?;
    
    // Verify authority
    if !authority_info.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Load config
    let mut config = LeverageSafetyConfig::try_from_slice(&config_info.data.borrow())?;
    
    if config.authority != *authority_info.key {
        return Err(LeverageSafetyError::InvalidAuthority.into());
    }
    
    // Update tier caps
    config.tier_caps.clear();
    for (min, max, leverage) in tier_caps {
        config.tier_caps.push(TierCap {
            min_outcomes: min,
            max_outcomes: max,
            max_leverage: leverage,
        });
    }
    
    config.last_update = Clock::get()?.unix_timestamp;
    
    // Validate updated config
    config.validate()?;
    
    // Save
    config.serialize(&mut &mut config_info.data.borrow_mut()[..])?;
    
    msg!("Tier caps updated: {} tiers", config.tier_caps.len());
    
    Ok(())
}