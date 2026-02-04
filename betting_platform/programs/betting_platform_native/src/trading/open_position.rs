//! Open position instruction handler
//!
//! Production-grade implementation of position opening logic

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program::invoke,
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::Sysvar,
    clock::Clock,
};

use crate::{
    account_validation::{validate_signer, validate_writable},
    error::BettingPlatformError,
    events::{Event, PositionOpened},
    instruction::OpenPositionParams,
    math::helpers::{calculate_percentage, apply_leverage},
    math::U64F64,
    pda::{GlobalConfigPDA, ProposalPDA, PositionPDA, UserMapPDA, VersePDA},
    state::{ProposalPDA as Proposal, Position, UserMap, VersePDA as Verse, ProposalState, GlobalConfigPDA as GlobalConfig},
    fees::polymarket_fee_integration::calculate_total_fees,
};

/// Process open position instruction
pub fn process_open_position(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    params: OpenPositionParams,
) -> ProgramResult {
    msg!("Processing open position");
    
    // Validate instruction parameters
    validate_params(&params)?;
    
    // Get accounts
    let account_info_iter = &mut accounts.iter();
    
    let user = next_account_info(account_info_iter)?;
    let global_config_info = next_account_info(account_info_iter)?;
    let verse_info = next_account_info(account_info_iter)?;
    let proposal_info = next_account_info(account_info_iter)?;
    let position_info = next_account_info(account_info_iter)?;
    let user_map_info = next_account_info(account_info_iter)?;
    let vault_info = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;
    let rent_sysvar = next_account_info(account_info_iter)?;
    
    // Validate accounts
    validate_signer(user)?;
    validate_writable(position_info)?;
    validate_writable(user_map_info)?;
    validate_writable(vault_info)?;
    validate_writable(global_config_info)?;
    validate_writable(proposal_info)?;
    
    // Validate PDAs
    let (global_config_pda, _) = GlobalConfigPDA::derive(program_id);
    if global_config_info.key != &global_config_pda {
        return Err(ProgramError::InvalidAccountData);
    }
    
    let (verse_pda, _) = VersePDA::derive(program_id, params.proposal_id >> 64);
    if verse_info.key != &verse_pda {
        return Err(ProgramError::InvalidAccountData);
    }
    
    let (proposal_pda, _) = ProposalPDA::derive(program_id, params.proposal_id);
    if proposal_info.key != &proposal_pda {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Determine position index
    let position_index = find_next_position_index(user, params.proposal_id, accounts)?;
    
    let (position_pda, bump) = PositionPDA::derive(
        program_id,
        user.key,
        params.proposal_id,
        position_index,
    );
    if position_info.key != &position_pda {
        return Err(ProgramError::InvalidAccountData);
    }
    
    let (user_map_pda, _) = UserMapPDA::derive(program_id, user.key);
    if user_map_info.key != &user_map_pda {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Load and validate accounts
    let mut global_config = GlobalConfig::try_from_slice(&global_config_info.data.borrow())?;
    global_config.validate()?;
    
    // Check system halt
    if global_config.halt_flag {
        return Err(BettingPlatformError::SystemHalted.into());
    }
    
    let verse = Verse::try_from_slice(&verse_info.data.borrow())?;
    verse.validate()?;
    
    let mut proposal = Proposal::try_from_slice(&proposal_info.data.borrow())?;
    proposal.validate()?;
    
    // Validate proposal state
    if proposal.state != ProposalState::Active {
        return Err(BettingPlatformError::InvalidProposalStatus.into());
    }
    
    if params.outcome >= proposal.outcomes {
        return Err(BettingPlatformError::InvalidOutcome.into());
    }
    
    // Calculate fees including Polymarket routing fee
    let coverage_ratio = U64F64::from_num(global_config.vault as u64) / U64F64::from_num(global_config.total_oi.max(1) as u64);
    
    // Check if this is a bundled trade (if part of a synthetic or verse trade)
    let is_bundled = verse.markets.len() > 1;
    
    // Get user's 7-day volume (stored in user map or default to 0)
    let user_volume_7d = if !user_map_info.data_is_empty() {
        if let Ok(user_map) = UserMap::try_from_slice(&user_map_info.data.borrow()) {
            user_map.total_volume_7d
        } else {
            0
        }
    } else {
        0
    };
    
    // Calculate total fees including Polymarket
    let (total_fee, fee_breakdown) = calculate_total_fees(
        params.size,
        coverage_ratio,
        user_volume_7d,
        is_bundled,
    )?;
    
    msg!("Fee breakdown: Model {}bp, Polymarket {}bp, Total {}bp, Saved {}bp",
        fee_breakdown.model_fee_bps,
        fee_breakdown.polymarket_fee_bps,
        fee_breakdown.total_fee_bps,
        fee_breakdown.savings_bps
    );
    
    // Calculate required collateral
    let leveraged_size = apply_leverage(params.size, params.leverage as u64)?;
    let required_collateral = params.size.saturating_add(total_fee);
    
    // Check user balance
    if **user.lamports.borrow() < required_collateral {
        return Err(BettingPlatformError::InsufficientBalance.into());
    }
    
    // Check coverage ratio
    let new_oi = global_config.total_oi.saturating_add(leveraged_size as u128);
    let coverage = if new_oi > 0 {
        global_config.vault.saturating_mul(10000) / new_oi
    } else {
        u128::MAX
    };
    
    // Validate leverage against coverage
    let max_leverage = get_max_leverage_for_coverage(&global_config, coverage)?;
    if params.leverage > max_leverage {
        return Err(BettingPlatformError::ExcessiveLeverage.into());
    }
    
    // Get current price
    let current_price = proposal.prices[params.outcome as usize];
    
    // Create position account
    let rent = Rent::from_account_info(rent_sysvar)?;
    let position_size = Position::LEN;
    let required_lamports = rent.minimum_balance(position_size);
    
    // Transfer collateral to vault
    **user.lamports.borrow_mut() -= required_collateral;
    **vault_info.lamports.borrow_mut() += params.size; // Collateral to vault
    **global_config_info.lamports.borrow_mut() += total_fee; // Fees to protocol
    
    // Create position PDA
    invoke(
        &solana_program::system_instruction::create_account(
            user.key,
            position_info.key,
            required_lamports,
            position_size as u64,
            program_id,
        ),
        &[
            user.clone(),
            position_info.clone(),
            system_program.clone(),
        ],
    )?;
    
    // Initialize position
    let clock = Clock::get()?;
    let mut position = Position::new(
        *user.key,
        params.proposal_id,
        verse.verse_id,
        params.outcome,
        params.size,
        params.leverage as u64,
        current_price,
        true, // is_long
        clock.unix_timestamp,
    );
    
    // Calculate required margin (size / leverage)
    let required_margin = params.size / params.leverage as u64;
    
    // Check for cross-margin account and apply benefits if enabled
    if let Some(cross_margin_info) = accounts.get(10) {
        if !cross_margin_info.data_is_empty() {
            if let Ok(cross_margin) = crate::margin::cross_margin::CrossMarginAccount::try_from_slice(&cross_margin_info.data.borrow()) {
                if cross_margin.user == *user.key && cross_margin.mode != crate::margin::cross_margin::CrossMarginMode::Isolated {
                    // Apply cross-margin benefits
                    let efficiency_ratio = cross_margin.efficiency_improvement as f64 / 
                                          cross_margin.net_margin_requirement.max(1) as f64;
                    
                    // Reduce margin requirement based on efficiency (capped at 50%)
                    let margin_reduction = ((required_margin as f64 * efficiency_ratio * 0.5) as u64).min(required_margin / 2);
                    position.margin = required_margin - margin_reduction;
                    position.cross_margin_enabled = true;
                    
                    msg!("Cross-margin applied: margin reduced by {} ({}%)", 
                         margin_reduction, 
                         (margin_reduction * 100) / required_margin);
                }
            }
        }
    }
    
    // Set entry funding index from market
    position.entry_funding_index = Some(if position.is_long {
        proposal.funding_state.long_funding_index
    } else {
        proposal.funding_state.short_funding_index
    });
    
    // Write position data
    position.serialize(&mut &mut position_info.data.borrow_mut()[..])?;
    
    // Create auto stop-loss for high leverage positions (>= 50x)
    if params.leverage >= 50 {
        // Auto stop-loss account should be at index 11
        if let Some(stop_loss_account) = accounts.get(11) {
            use crate::trading::auto_stop_loss::{create_auto_stop_loss, AUTO_STOP_LOSS_MIN_LEVERAGE};
            
            // Only create if leverage meets threshold
            if params.leverage >= AUTO_STOP_LOSS_MIN_LEVERAGE {
                msg!("Creating auto stop-loss for {}x leverage position", params.leverage);
                
                // Create auto stop-loss with dedicated accounts
                let stop_loss_accounts = &[
                    user.clone(),
                    stop_loss_account.clone(),
                    system_program.clone(),
                ];
                
                if let Err(e) = create_auto_stop_loss(
                    program_id,
                    &position,
                    params.leverage,
                    current_price,
                    stop_loss_accounts,
                ) {
                    msg!("Warning: Failed to create auto stop-loss: {:?}", e);
                    // Continue with position creation even if stop-loss fails
                }
            }
        } else {
            msg!("Warning: No stop-loss account provided for high leverage position");
        }
    }
    
    // Update user map
    let mut user_map = if user_map_info.data_len() > 0 {
        UserMap::try_from_slice(&user_map_info.data.borrow())?
    } else {
        // Create user map if it doesn't exist
        create_user_map(user_map_info, user.key, program_id, user, system_program, rent_sysvar)?;
        UserMap::new(*user.key)
    };
    
    user_map.add_position(params.proposal_id)?;
    user_map.serialize(&mut &mut user_map_info.data.borrow_mut()[..])?;
    
    // Update global state
    global_config.total_oi = new_oi;
    global_config.vault = global_config.vault.saturating_add(params.size as u128);
    global_config.serialize(&mut &mut global_config_info.data.borrow_mut()[..])?;
    
    // Update proposal volume
    proposal.volumes[params.outcome as usize] = 
        proposal.volumes[params.outcome as usize].saturating_add(leveraged_size);
    proposal.serialize(&mut &mut proposal_info.data.borrow_mut()[..])?;
    
    // Generate position ID
    let position_id = Position::generate_position_id(user.key, params.proposal_id, params.outcome);
    
    // Emit event
    PositionOpened {
        user: *user.key,
        proposal_id: params.proposal_id,
        outcome: params.outcome,
        size: params.size,
        leverage: params.leverage as u64,
        entry_price: current_price,
        is_long: true,
        position_id,
        chain_id: params.chain_id,
    }.emit();
    
    msg!("Position opened successfully");
    Ok(())
}

/// Validate instruction parameters
fn validate_params(params: &OpenPositionParams) -> ProgramResult {
    if params.size == 0 {
        return Err(BettingPlatformError::InvalidInput.into());
    }
    
    if params.leverage == 0 || params.leverage > 100 {
        return Err(BettingPlatformError::InvalidLeverageTier.into());
    }
    
    if params.outcome > 63 {
        return Err(BettingPlatformError::InvalidOutcome.into());
    }
    
    Ok(())
}

/// Calculate dynamic fee based on leverage
fn calculate_dynamic_fee(global_config: &GlobalConfig, leverage: u8) -> Result<u64, ProgramError> {
    // Higher leverage = higher fee
    let leverage_multiplier = if leverage > 10 {
        200 // 2x for high leverage
    } else if leverage > 5 {
        150 // 1.5x for medium leverage
    } else {
        100 // 1x for low leverage
    };
    
    let base_dynamic_fee = global_config.fee_slope as u64;
    Ok(base_dynamic_fee * leverage_multiplier / 100)
}

/// Get maximum allowed leverage for coverage ratio
fn get_max_leverage_for_coverage(
    global_config: &GlobalConfig,
    coverage: u128,
) -> Result<u8, ProgramError> {
    // Find appropriate leverage tier based on coverage
    for tier in &global_config.leverage_tiers {
        if coverage >= tier.n as u128 * 100 {
            return Ok(tier.max);
        }
    }
    
    // Default to minimum leverage if no tier matches
    Ok(1)
}

/// Find next available position index
fn find_next_position_index(
    user: &AccountInfo,
    proposal_id: u128,
    accounts: &[AccountInfo],
) -> Result<u8, ProgramError> {
    // Find next available position index by checking existing positions
    use crate::state::{Position, UserMap};
    use borsh::BorshDeserialize;
    use solana_program::program_pack::Pack;
    
    // Check if user map exists
    if let Some(user_map_account) = accounts.iter().find(|acc| {
        acc.owner == &crate::id() && acc.data_len() == UserMap::LEN
    }) {
        // Deserialize user map
        let user_map = UserMap::unpack_from_slice(&user_map_account.data.borrow())?;
        
        // Count existing positions for this proposal
        let mut max_index = 0u8;
        for (i, pos_pubkey) in user_map.positions.iter().enumerate() {
            if *pos_pubkey != Pubkey::default() {
                // Check if this position is for the same proposal
                if let Some(pos_account) = accounts.iter().find(|acc| acc.key == pos_pubkey) {
                    if let Ok(position) = Position::deserialize(&mut &pos_account.data.borrow()[..]) {
                        if position.proposal_id == proposal_id {
                            max_index = max_index.max(i as u8 + 1);
                        }
                    }
                }
            }
        }
        
        // Return next available index
        if max_index >= 255 {
            return Err(BettingPlatformError::TooManyPositions.into());
        }
        Ok(max_index)
    } else {
        // No user map exists yet, first position
        Ok(0)
    }
}

/// Create user map account
fn create_user_map<'a>(
    user_map_info: &AccountInfo<'a>,
    user_key: &Pubkey,
    program_id: &Pubkey,
    payer: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    rent_sysvar: &AccountInfo<'a>,
) -> ProgramResult {
    let rent = Rent::from_account_info(rent_sysvar)?;
    let space = 1000; // Space for UserMap
    let lamports = rent.minimum_balance(space);
    
    invoke(
        &solana_program::system_instruction::create_account(
            payer.key,
            user_map_info.key,
            lamports,
            space as u64,
            program_id,
        ),
        &[
            payer.clone(),
            user_map_info.clone(),
            system_program.clone(),
        ],
    )?;
    
    Ok(())
}