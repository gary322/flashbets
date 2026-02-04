use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::{clock::Clock, Sysvar},
    system_instruction,
    program::invoke,
};
use borsh::{BorshDeserialize, BorshSerialize};
use sha2::{Sha256, Digest};

pub mod state;
pub mod instructions;
pub mod errors;
pub mod amm;
pub mod zk;
pub mod utils;

use state::*;
use errors::FlashError;

// Program ID will be set during deployment
solana_program::declare_id!("11111111111111111111111111111112");

// Program entrypoint
entrypoint!(process_instruction);

// Instruction enum for instruction discrimination
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub enum FlashInstruction {
    /// Create a flash verse for sub-minute betting
    /// 
    /// Accounts expected:
    /// 0. [writable] Flash verse account
    /// 1. [signer] Creator account
    /// 2. [] System program
    /// 3. [] Rent sysvar
    CreateFlashVerse {
        title: String,
        sport_type: u8,
        time_left: u64,
        outcomes: Vec<String>,
    },

    /// Trade on a flash market with micro-tau AMM
    /// 
    /// Accounts expected:
    /// 0. [writable] Flash verse account
    /// 1. [signer] Trader account
    /// 2. [writable] Trader token account
    /// 3. [writable] Vault token account
    /// 4. [] Token program
    TradeFlash {
        outcome_index: u8,
        amount: u64,
        max_slippage: u64,
    },

    /// Execute leverage chaining for up to 500x effective leverage
    /// 
    /// Accounts expected:
    /// 0. [writable] Flash verse account
    /// 1. [signer] Trader account
    /// 2. [writable] Position account
    /// 3. [] System program
    /// 4. [] Additional accounts for CPI calls
    ChainLeverage {
        base_amount: u64,
        steps: Vec<ChainStep>,
    },

    /// Resolve flash market with ZK proof
    /// 
    /// Accounts expected:
    /// 0. [writable] Flash verse account
    /// 1. [signer] Resolver account
    /// 2. [] Clock sysvar
    ResolveFlash {
        proof: Vec<u8>,
        outcome_index: u8,
    },

    /// Create quantum flash position for multi-outcome betting
    /// 
    /// Accounts expected:
    /// 0. [writable] Quantum flash account
    /// 1. [] Flash verse account
    /// 2. [signer] Owner account
    /// 3. [] System program
    /// 4. [] Rent sysvar
    CreateQuantumFlash {
        verse_id: u128,
        amount: u64,
        leverage: u8,
    },

    /// Collapse quantum position
    /// 
    /// Accounts expected:
    /// 0. [writable] Quantum flash account
    /// 1. [] Flash verse account
    /// 2. [signer] Owner account
    /// 3. [writable] Owner token account
    /// 4. [writable] Vault token account
    /// 5. [] Token program
    CollapseQuantum {
        proof: Vec<u8>,
    },
}

/// Main instruction processor
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let instruction = FlashInstruction::try_from_slice(instruction_data)
        .map_err(|_| ProgramError::InvalidInstructionData)?;

    match instruction {
        FlashInstruction::CreateFlashVerse {
            title,
            sport_type,
            time_left,
            outcomes,
        } => process_create_flash_verse(program_id, accounts, title, sport_type, time_left, outcomes),

        FlashInstruction::TradeFlash {
            outcome_index,
            amount,
            max_slippage,
        } => process_trade_flash(program_id, accounts, outcome_index, amount, max_slippage),

        FlashInstruction::ChainLeverage {
            base_amount,
            steps,
        } => process_chain_leverage(program_id, accounts, base_amount, steps),

        FlashInstruction::ResolveFlash {
            proof,
            outcome_index,
        } => process_resolve_flash(program_id, accounts, proof, outcome_index),

        FlashInstruction::CreateQuantumFlash {
            verse_id,
            amount,
            leverage,
        } => process_create_quantum_flash(program_id, accounts, verse_id, amount, leverage),

        FlashInstruction::CollapseQuantum { proof } => {
            process_collapse_quantum(program_id, accounts, proof)
        }
    }
}

/// Create a flash verse for sub-minute betting
pub fn process_create_flash_verse(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    title: String,
    sport_type: u8,
    time_left: u64,
    outcomes: Vec<String>,
) -> ProgramResult {
    let accounts_iter = &mut accounts.iter();
    let flash_verse_account = next_account_info(accounts_iter)?;
    let creator = next_account_info(accounts_iter)?;
    let system_program = next_account_info(accounts_iter)?;
    let rent_sysvar = next_account_info(accounts_iter)?;

    // Validate inputs
    if time_left > 14400 {
        return Err(FlashError::NotFlashMarket.into());
    }
    if outcomes.len() < 2 {
        return Err(FlashError::InsufficientOutcomes.into());
    }
    if outcomes.len() > 10 {
        return Err(FlashError::TooManyOutcomes.into());
    }

    let rent = Rent::from_account_info(rent_sysvar)?;
    let clock = Clock::get()?;

    // Create account if needed
    let space = std::mem::size_of::<FlashVerse>();
    let lamports = rent.minimum_balance(space);

    if flash_verse_account.data_len() == 0 {
        invoke(
            &system_instruction::create_account(
                creator.key,
                flash_verse_account.key,
                lamports,
                space as u64,
                program_id,
            ),
            &[creator.clone(), flash_verse_account.clone(), system_program.clone()],
        )?;
    }

    let mut flash_verse_data = flash_verse_account.try_borrow_mut_data()?;
    let mut flash_verse = FlashVerse::try_from_slice(&flash_verse_data)
        .unwrap_or_else(|_| FlashVerse::default());

    // Generate unique ID
    let mut hasher = Sha256::new();
    hasher.update(title.as_bytes());
    hasher.update(clock.slot.to_le_bytes());
    let hash = hasher.finalize();
    flash_verse.id = u128::from_le_bytes(hash[..16].try_into().unwrap());

    // Find and link to parent verse via CPI
    let parent_id = utils::find_parent_verse(&title, sport_type)?;
    flash_verse.parent_id = parent_id;

    // Set flash parameters with duration-based configuration
    flash_verse.title = title.clone();
    flash_verse.sport_type = sport_type;
    flash_verse.time_left = time_left;
    flash_verse.tau = utils::calculate_tau(time_left);
    flash_verse.settle_slot = clock.slot + (time_left * 2); // ~0.5s per slot

    // Set leverage based on duration tier
    flash_verse.max_leverage = match time_left {
        0..=60 => 500,      // Ultra-flash: 500x
        61..=600 => 250,    // Quick-flash (10 min): 250x
        601..=1800 => 150,  // Half-flash (30 min): 150x
        1801..=3600 => 100, // Hour-flash: 100x
        _ => 75,            // Match-long: 75x
    };

    // Initialize outcomes
    flash_verse.outcomes = outcomes
        .iter()
        .map(|name| Outcome {
            name: name.clone(),
            probability: 1.0 / outcomes.len() as f64, // Equal initial probability
            volume: 0,
            odds: outcomes.len() as f64,
        })
        .collect();

    flash_verse.total_volume = 0;
    flash_verse.leverage_mult = 1;
    flash_verse.is_resolved = false;
    flash_verse.proof_hash = [0u8; 32];

    // Serialize and save
    flash_verse.serialize(&mut *flash_verse_data)?;

    // CPI to main program to link parent
    if parent_id > 0 {
        instructions::link_to_parent(accounts, parent_id)?;
    }

    msg!(
        "FlashVerseCreated: verse_id={}, parent_id={}, title={}, sport_type={}, time_left={}, tau={}",
        flash_verse.id,
        parent_id,
        flash_verse.title,
        sport_type,
        time_left,
        flash_verse.tau
    );

    Ok(())
}

/// Trade on a flash market with micro-tau AMM
pub fn process_trade_flash(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    outcome_index: u8,
    amount: u64,
    max_slippage: u64,
) -> ProgramResult {
    let accounts_iter = &mut accounts.iter();
    let flash_verse_account = next_account_info(accounts_iter)?;
    let trader = next_account_info(accounts_iter)?;
    let trader_token_account = next_account_info(accounts_iter)?;
    let vault_token_account = next_account_info(accounts_iter)?;
    let token_program = next_account_info(accounts_iter)?;

    let mut flash_verse_data = flash_verse_account.try_borrow_mut_data()?;
    let mut flash_verse = FlashVerse::try_from_slice(&flash_verse_data)?;
    let clock = Clock::get()?;

    // Check market is still open
    if flash_verse.is_resolved {
        return Err(FlashError::MarketResolved.into());
    }
    if clock.slot >= flash_verse.settle_slot {
        return Err(FlashError::MarketExpired.into());
    }
    if (outcome_index as usize) >= flash_verse.outcomes.len() {
        return Err(FlashError::InvalidOutcome.into());
    }

    // Calculate trade using micro-tau AMM
    let tau = flash_verse.tau;
    let current_prob = flash_verse.outcomes[outcome_index as usize].probability;
    let (new_prob, actual_amount) = amm::micro_tau::calculate_trade(
        current_prob,
        amount,
        tau,
        max_slippage,
    )?;

    // Update outcome
    flash_verse.outcomes[outcome_index as usize].probability = new_prob;
    flash_verse.outcomes[outcome_index as usize].volume += actual_amount;
    flash_verse.outcomes[outcome_index as usize].odds = 1.0 / new_prob;

    // Update total volume
    flash_verse.total_volume += actual_amount;

    // Transfer funds using SPL tokens
    instructions::token::transfer_tokens(
        token_program,
        trader_token_account,
        vault_token_account,
        trader,
        actual_amount,
    )?;

    // Serialize updated state
    flash_verse.serialize(&mut *flash_verse_data)?;

    msg!(
        "FlashTrade: verse_id={}, trader={}, outcome_index={}, amount={}, new_probability={}, new_odds={}",
        flash_verse.id,
        trader.key,
        outcome_index,
        actual_amount,
        new_prob,
        1.0 / new_prob
    );

    Ok(())
}

/// Execute leverage chaining for up to 500x effective leverage
pub fn process_chain_leverage(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    base_amount: u64,
    steps: Vec<ChainStep>,
) -> ProgramResult {
    if steps.len() > 5 {
        return Err(FlashError::TooManyChainSteps.into());
    }
    if base_amount == 0 {
        return Err(FlashError::InvalidAmount.into());
    }

    let accounts_iter = &mut accounts.iter();
    let flash_verse_account = next_account_info(accounts_iter)?;
    let _trader = next_account_info(accounts_iter)?;
    let _position_account = next_account_info(accounts_iter)?;
    let _system_program = next_account_info(accounts_iter)?;

    let mut flash_verse_data = flash_verse_account.try_borrow_mut_data()?;
    let mut flash_verse = FlashVerse::try_from_slice(&flash_verse_data)?;

    let mut current_amount = base_amount;
    let mut total_multiplier = 1.0;

    for step in steps.iter() {
        match step.action {
            ChainAction::Borrow => {
                // CPI to lending protocol
                let borrowed = instructions::borrow_funds(accounts, current_amount)?;
                current_amount += borrowed;
                total_multiplier *= 1.5;
            }
            ChainAction::Liquidate => {
                // CPI to liquidation pool
                let bonus = instructions::liquidate_for_bonus(accounts, current_amount)?;
                current_amount += bonus;
                total_multiplier *= 1.2;
            }
            ChainAction::Stake => {
                // CPI to staking program
                let rewards = instructions::stake_for_boost(accounts, current_amount)?;
                current_amount += rewards;
                total_multiplier *= 1.1;
            }
        }
    }

    // Apply micro-tau efficiency bonus
    let tau_bonus = 1.0 + flash_verse.tau * 1500.0;
    total_multiplier *= tau_bonus;

    // Cap at 500x
    let final_multiplier = total_multiplier.min(500.0);
    flash_verse.leverage_mult = final_multiplier as u16;

    // Serialize updated state
    flash_verse.serialize(&mut *flash_verse_data)?;

    msg!(
        "LeverageChained: verse_id={}, base_amount={}, final_amount={}, multiplier={}, steps_count={}",
        flash_verse.id,
        base_amount,
        current_amount,
        final_multiplier,
        steps.len()
    );

    Ok(())
}

/// Resolve flash market with ZK proof
pub fn process_resolve_flash(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    proof: Vec<u8>,
    outcome_index: u8,
) -> ProgramResult {
    let accounts_iter = &mut accounts.iter();
    let flash_verse_account = next_account_info(accounts_iter)?;
    let _resolver = next_account_info(accounts_iter)?;
    let _clock_sysvar = next_account_info(accounts_iter)?;

    let mut flash_verse_data = flash_verse_account.try_borrow_mut_data()?;
    let mut flash_verse = FlashVerse::try_from_slice(&flash_verse_data)?;
    let clock = Clock::get()?;

    if flash_verse.is_resolved {
        return Err(FlashError::AlreadyResolved.into());
    }
    if (outcome_index as usize) >= flash_verse.outcomes.len() {
        return Err(FlashError::InvalidOutcome.into());
    }

    // Only allow resolution at/after the precomputed settlement slot.
    if clock.slot < flash_verse.settle_slot {
        return Err(FlashError::MarketNotSettled.into());
    }

    // Verify ZK proof
    let is_valid = zk::verifier::verify_outcome_proof(
        &proof,
        flash_verse.id,
        outcome_index,
        flash_verse.settle_slot,
    )?;
    if !is_valid {
        return Err(FlashError::InvalidProof.into());
    }

    // Calculate proof hash
    let mut hasher = Sha256::new();
    hasher.update(&proof);
    let hash = hasher.finalize();
    flash_verse.proof_hash = hash.into();

    // Mark as resolved
    flash_verse.is_resolved = true;
    flash_verse.winning_outcome = Some(outcome_index);

    // Serialize updated state
    flash_verse.serialize(&mut *flash_verse_data)?;

    msg!(
        "FlashResolved: verse_id={}, winning_outcome={}, proof_hash={:?}, resolved_slot={}, total_volume={}",
        flash_verse.id,
        outcome_index,
        flash_verse.proof_hash,
        clock.slot,
        flash_verse.total_volume
    );

    Ok(())
}

/// Create quantum flash position for multi-outcome betting
pub fn process_create_quantum_flash(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    verse_id: u128,
    amount: u64,
    leverage: u8,
) -> ProgramResult {
    if leverage > 100 {
        return Err(FlashError::ExcessiveLeverage.into());
    }
    if amount == 0 {
        return Err(FlashError::InvalidAmount.into());
    }

    let accounts_iter = &mut accounts.iter();
    let quantum_account = next_account_info(accounts_iter)?;
    let flash_verse_account = next_account_info(accounts_iter)?;
    let owner = next_account_info(accounts_iter)?;
    let system_program = next_account_info(accounts_iter)?;
    let rent_sysvar = next_account_info(accounts_iter)?;

    let rent = Rent::from_account_info(rent_sysvar)?;
    let flash_verse_data = flash_verse_account.try_borrow_data()?;
    let flash_verse = FlashVerse::try_from_slice(&flash_verse_data)?;

    // Create quantum account if needed
    let space = std::mem::size_of::<QuantumFlash>();
    let lamports = rent.minimum_balance(space);

    if quantum_account.data_len() == 0 {
        invoke(
            &system_instruction::create_account(
                owner.key,
                quantum_account.key,
                lamports,
                space as u64,
                program_id,
            ),
            &[owner.clone(), quantum_account.clone(), system_program.clone()],
        )?;
    }

    let mut quantum_data = quantum_account.try_borrow_mut_data()?;
    let mut quantum = QuantumFlash::try_from_slice(&quantum_data)
        .unwrap_or_else(|_| QuantumFlash::default());

    // Initialize quantum states from all outcomes
    quantum.position_id = utils::generate_position_id();
    quantum.verse_id = verse_id;
    quantum.owner = *owner.key;
    quantum.states = flash_verse
        .outcomes
        .iter()
        .map(|outcome| QuantumState {
            outcome: outcome.name.clone(),
            probability: outcome.probability,
            amplitude: outcome.probability.sqrt(),
            phase: std::f64::consts::PI * outcome.probability,
        })
        .collect();

    quantum.leverage = leverage;
    quantum.base_amount = amount;
    quantum.total_exposure = amount * leverage as u64;
    quantum.is_collapsed = false;

    // Set collapse trigger based on time
    quantum.collapse_trigger = if flash_verse.time_left < 30 {
        CollapseTrigger::EventOccurrence { threshold: 0.8 }
    } else {
        CollapseTrigger::TimeExpiry {
            slot: flash_verse.settle_slot,
        }
    };

    // Serialize and save
    quantum.serialize(&mut *quantum_data)?;

    msg!(
        "QuantumFlashCreated: position_id={}, verse_id={}, states_count={}, leverage={}, total_exposure={}",
        quantum.position_id,
        verse_id,
        quantum.states.len(),
        leverage,
        quantum.total_exposure
    );

    Ok(())
}

/// Collapse quantum position
pub fn process_collapse_quantum(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    proof: Vec<u8>,
) -> ProgramResult {
    let accounts_iter = &mut accounts.iter();
    let quantum_account = next_account_info(accounts_iter)?;
    let flash_verse_account = next_account_info(accounts_iter)?;
    let owner = next_account_info(accounts_iter)?;
    let owner_token_account = next_account_info(accounts_iter)?;
    let vault_token_account = next_account_info(accounts_iter)?;
    let token_program = next_account_info(accounts_iter)?;

    let mut quantum_data = quantum_account.try_borrow_mut_data()?;
    let mut quantum = QuantumFlash::try_from_slice(&quantum_data)?;
    let flash_verse_data = flash_verse_account.try_borrow_data()?;
    let flash_verse = FlashVerse::try_from_slice(&flash_verse_data)?;

    if quantum.is_collapsed {
        return Err(FlashError::AlreadyCollapsed.into());
    }
    if !flash_verse.is_resolved {
        return Err(FlashError::MarketNotResolved.into());
    }

    // Verify proof and get winning outcome
    let winning_outcome = flash_verse
        .winning_outcome
        .ok_or(FlashError::NoWinningOutcome)?;

    let proof_valid = zk::groth16_verifier::Groth16Verifier::verify_quantum_collapse_proof(
        &proof,
        quantum.position_id,
        quantum.verse_id,
        quantum.leverage,
        winning_outcome,
    )?;
    if !proof_valid {
        return Err(FlashError::InvalidProof.into());
    }

    // Calculate payout based on quantum state
    let winning_state = &quantum.states[winning_outcome as usize];
    let payout = (quantum.total_exposure as f64 * winning_state.probability) as u64;

    // Mark as collapsed
    quantum.is_collapsed = true;
    quantum.collapsed_outcome = Some(winning_outcome);
    quantum.payout = payout;

    // Transfer payout
    if payout > 0 {
        instructions::token::transfer_tokens(
            token_program,
            vault_token_account,
            owner_token_account,
            owner,
            payout,
        )?;
    }

    // Serialize updated state
    quantum.serialize(&mut *quantum_data)?;

    msg!(
        "QuantumCollapsed: position_id={}, winning_outcome={}, payout={}, leverage={}",
        quantum.position_id,
        winning_outcome,
        payout,
        quantum.leverage
    );

    Ok(())
}
