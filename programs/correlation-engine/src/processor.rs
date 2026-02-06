use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program::invoke_signed,
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    system_instruction,
    sysvar::Sysvar,
    clock::Clock,
};
use borsh::BorshDeserialize;

use crate::{
    error::CorrelationError,
    instruction::CorrelationInstruction,
    state::{
        CorrelationEngine, CorrelationMatrix, VerseTailLoss, VerseTracking,
        MarketPriceHistory,
    },
    math::{
        calculate_pearson_correlation,
    },
};

pub struct Processor;

impl Processor {
    fn borsh_deserialize_unchecked<T: BorshDeserialize>(data: &[u8]) -> Result<T, ProgramError> {
        let mut cursor: &[u8] = data;
        T::deserialize(&mut cursor).map_err(|_| ProgramError::InvalidAccountData)
    }

    pub fn process(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        instruction_data: &[u8],
    ) -> ProgramResult {
        let instruction = CorrelationInstruction::unpack(instruction_data)?;
        
        match instruction {
            CorrelationInstruction::InitializeEngine => {
                msg!("Instruction: InitializeEngine");
                Self::process_initialize_engine(accounts, program_id)
            }
            CorrelationInstruction::InitializeVerseTracking { verse_id } => {
                msg!("Instruction: InitializeVerseTracking");
                Self::process_initialize_verse_tracking(accounts, program_id, verse_id)
            }
            CorrelationInstruction::UpdatePriceHistory { market_id, price, volume } => {
                msg!("Instruction: UpdatePriceHistory");
                Self::process_update_price_history(accounts, program_id, market_id, price, volume)
            }
            CorrelationInstruction::CalculateCorrelations { verse_id } => {
                msg!("Instruction: CalculateCorrelations");
                Self::process_calculate_correlations(accounts, program_id, verse_id)
            }
            CorrelationInstruction::UpdateTailLoss { verse_id, outcome_count } => {
                msg!("Instruction: UpdateTailLoss");
                Self::process_update_tail_loss(accounts, program_id, verse_id, outcome_count)
            }
            CorrelationInstruction::UpdateMarketWeights { verse_id, market_weights } => {
                msg!("Instruction: UpdateMarketWeights");
                Self::process_update_market_weights(accounts, program_id, verse_id, market_weights)
            }
        }
    }
    
    fn process_initialize_engine(
        accounts: &[AccountInfo],
        program_id: &Pubkey,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let authority_info = next_account_info(account_info_iter)?;
        let engine_info = next_account_info(account_info_iter)?;
        let system_program = next_account_info(account_info_iter)?;
        let rent = &Rent::from_account_info(next_account_info(account_info_iter)?)?;
        
        // Verify authority is signer
        if !authority_info.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }
        
        // Derive PDA for engine
        let (engine_pubkey, engine_bump) = Pubkey::find_program_address(
            &[b"correlation_engine"],
            program_id,
        );
        
        if engine_pubkey != *engine_info.key {
            return Err(CorrelationError::InvalidPDA.into());
        }
        
        // Create engine account
        let engine_size = CorrelationEngine::LEN;
        let engine_lamports = rent.minimum_balance(engine_size);
        
        invoke_signed(
            &system_instruction::create_account(
                authority_info.key,
                engine_info.key,
                engine_lamports,
                engine_size as u64,
                program_id,
            ),
            &[authority_info.clone(), engine_info.clone(), system_program.clone()],
            &[&[b"correlation_engine", &[engine_bump]]],
        )?;
        
        // Initialize engine
        let engine = CorrelationEngine::new(*authority_info.key, engine_bump);
        borsh::to_writer(&mut engine_info.try_borrow_mut_data()?.as_mut(), &engine)
            .map_err(|_| ProgramError::InvalidAccountData)?;
        
        msg!("Correlation engine initialized successfully");
        Ok(())
    }
    
    fn process_initialize_verse_tracking(
        accounts: &[AccountInfo],
        program_id: &Pubkey,
        verse_id: [u8; 16],
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let authority_info = next_account_info(account_info_iter)?;
        let engine_info = next_account_info(account_info_iter)?;
        let verse_tracking_info = next_account_info(account_info_iter)?;
        let correlation_matrix_info = next_account_info(account_info_iter)?;
        let tail_loss_info = next_account_info(account_info_iter)?;
        let system_program = next_account_info(account_info_iter)?;
        let rent = &Rent::from_account_info(next_account_info(account_info_iter)?)?;
        
        // Load and verify engine
        let engine_data = engine_info.try_borrow_data()?;
        let engine: CorrelationEngine = CorrelationEngine::try_from_slice(&engine_data)
            .map_err(|_| ProgramError::InvalidAccountData)?;
        
        if !engine.is_initialized {
            return Err(ProgramError::UninitializedAccount);
        }
        
        if *authority_info.key != engine.authority {
            return Err(CorrelationError::InvalidAuthority.into());
        }
        
        // Derive PDAs
        let (verse_tracking_pubkey, verse_tracking_bump) = Pubkey::find_program_address(
            &[b"verse_tracking", &verse_id],
            program_id,
        );
        
        if verse_tracking_pubkey != *verse_tracking_info.key {
            return Err(CorrelationError::InvalidPDA.into());
        }
        
        let (correlation_matrix_pubkey, matrix_bump) = Pubkey::find_program_address(
            &[b"correlation_matrix", &verse_id],
            program_id,
        );
        
        let (tail_loss_pubkey, tail_loss_bump) = Pubkey::find_program_address(
            &[b"tail_loss", &verse_id],
            program_id,
        );
        
        // Create verse tracking account
        let tracking_size = VerseTracking::calculate_size(20); // Max 20 markets (reasonable for Solana account limits)
        let tracking_lamports = rent.minimum_balance(tracking_size);
        
        invoke_signed(
            &system_instruction::create_account(
                authority_info.key,
                verse_tracking_info.key,
                tracking_lamports,
                tracking_size as u64,
                program_id,
            ),
            &[authority_info.clone(), verse_tracking_info.clone(), system_program.clone()],
            &[&[b"verse_tracking", &verse_id, &[verse_tracking_bump]]],
        )?;
        
        // Initialize verse tracking
        let mut verse_tracking = VerseTracking::new(verse_id, verse_tracking_bump);
        verse_tracking.correlation_matrix_pda = correlation_matrix_pubkey;
        verse_tracking.tail_loss_pda = tail_loss_pubkey;
        
        borsh::to_writer(&mut verse_tracking_info.try_borrow_mut_data()?.as_mut(), &verse_tracking)
            .map_err(|_| ProgramError::InvalidAccountData)?;
        
        // Create correlation matrix account
        let matrix_size = CorrelationMatrix::calculate_size(20); // Match the tracking size
        let matrix_lamports = rent.minimum_balance(matrix_size);
        
        invoke_signed(
            &system_instruction::create_account(
                authority_info.key,
                correlation_matrix_info.key,
                matrix_lamports,
                matrix_size as u64,
                program_id,
            ),
            &[authority_info.clone(), correlation_matrix_info.clone(), system_program.clone()],
            &[&[b"correlation_matrix", &verse_id, &[matrix_bump]]],
        )?;
        
        // Initialize correlation matrix
        let correlation_matrix = CorrelationMatrix::new(verse_id, matrix_bump);
        borsh::to_writer(&mut correlation_matrix_info.try_borrow_mut_data()?.as_mut(), &correlation_matrix)
            .map_err(|_| ProgramError::InvalidAccountData)?;
        
        // Create tail loss account
        let tail_loss_size = VerseTailLoss::LEN;
        let tail_loss_lamports = rent.minimum_balance(tail_loss_size);
        
        invoke_signed(
            &system_instruction::create_account(
                authority_info.key,
                tail_loss_info.key,
                tail_loss_lamports,
                tail_loss_size as u64,
                program_id,
            ),
            &[authority_info.clone(), tail_loss_info.clone(), system_program.clone()],
            &[&[b"tail_loss", &verse_id, &[tail_loss_bump]]],
        )?;
        
        // Initialize tail loss
        let tail_loss = VerseTailLoss::new(verse_id, tail_loss_bump);
        borsh::to_writer(&mut tail_loss_info.try_borrow_mut_data()?.as_mut(), &tail_loss)
            .map_err(|_| ProgramError::InvalidAccountData)?;
        
        msg!("Verse tracking initialized for {:?}", verse_id);
        Ok(())
    }
    
    fn process_update_price_history(
        accounts: &[AccountInfo],
        program_id: &Pubkey,
        market_id: [u8; 16],
        price: u64,
        volume: u64,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let keeper_info = next_account_info(account_info_iter)?;
        let price_history_info = next_account_info(account_info_iter)?;
        let system_program_info = account_info_iter.next();
        let clock = Clock::get()?;
        
        // Verify keeper is signer
        if !keeper_info.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }

        let (expected_pubkey, bump) =
            Pubkey::find_program_address(&[b"price_history", &market_id], program_id);

        if expected_pubkey != *price_history_info.key {
            return Err(CorrelationError::InvalidPDA.into());
        }

        // Create the PDA account on first use (clients can't create PDAs directly).
        if price_history_info.owner != program_id {
            let system_program_info =
                system_program_info.ok_or(ProgramError::NotEnoughAccountKeys)?;

            if *system_program_info.key != solana_program::system_program::id() {
                return Err(ProgramError::IncorrectProgramId);
            }

            let rent = Rent::get()?;
            let price_history_size = MarketPriceHistory::calculate_size();
            let price_history_lamports = rent.minimum_balance(price_history_size);

            invoke_signed(
                &system_instruction::create_account(
                    keeper_info.key,
                    price_history_info.key,
                    price_history_lamports,
                    price_history_size as u64,
                    program_id,
                ),
                &[
                    keeper_info.clone(),
                    price_history_info.clone(),
                    system_program_info.clone(),
                ],
                &[&[b"price_history", &market_id, &[bump]]],
            )?;
        }

        // Load or initialize price history
        let mut price_history_data = price_history_info.try_borrow_mut_data()?;
        let is_initialized = price_history_data.first().copied().unwrap_or(0) != 0;
        let mut price_history = if is_initialized {
            Self::borsh_deserialize_unchecked::<MarketPriceHistory>(&price_history_data)?
        } else {
            MarketPriceHistory::new(market_id, bump)
        };
        
        // Add price point
        price_history.add_price_point(
            price,
            clock.unix_timestamp,
            clock.slot,
            volume,
        )?;
        
        // Save back
        borsh::to_writer(&mut price_history_data.as_mut(), &price_history)
            .map_err(|_| ProgramError::InvalidAccountData)?;
        
        msg!("Price updated for market {:?}: {} @ slot {}", market_id, price, clock.slot);
        Ok(())
    }
    
    fn process_calculate_correlations(
        accounts: &[AccountInfo],
        _program_id: &Pubkey,
        verse_id: [u8; 16],
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let authority_info = next_account_info(account_info_iter)?;
        let engine_info = next_account_info(account_info_iter)?;
        let verse_tracking_info = next_account_info(account_info_iter)?;
        let correlation_matrix_info = next_account_info(account_info_iter)?;
        
        // Remaining accounts are market price histories
        let price_history_accounts: Vec<_> = account_info_iter.collect();
        
        // Load engine and verify authority
        let engine_data = engine_info.try_borrow_data()?;
        let engine: CorrelationEngine = CorrelationEngine::try_from_slice(&engine_data)
            .map_err(|_| ProgramError::InvalidAccountData)?;
        
        if *authority_info.key != engine.authority {
            return Err(CorrelationError::InvalidAuthority.into());
        }
        
        // Load verse tracking
        let verse_tracking_data = verse_tracking_info.try_borrow_data()?;
        let _verse_tracking: VerseTracking =
            Self::borsh_deserialize_unchecked::<VerseTracking>(&verse_tracking_data)?;
        
        // Load price histories
        let mut price_series = Vec::new();
        for (i, account) in price_history_accounts.iter().enumerate() {
            let data = account.try_borrow_data()?;
            let history: MarketPriceHistory =
                Self::borsh_deserialize_unchecked::<MarketPriceHistory>(&data)?;
            
            if history.has_sufficient_data() {
                price_series.push((i, history.get_daily_prices()));
            }
        }
        
        // Calculate pairwise correlations
        let mut correlation_matrix_data = correlation_matrix_info.try_borrow_mut_data()?;
        let mut correlation_matrix: CorrelationMatrix =
            Self::borsh_deserialize_unchecked::<CorrelationMatrix>(&correlation_matrix_data)?;
        
        let clock = Clock::get()?;
        
        for i in 0..price_series.len() {
            for j in (i + 1)..price_series.len() {
                let (idx_i, prices_i) = &price_series[i];
                let (idx_j, prices_j) = &price_series[j];
                
                let correlation = calculate_pearson_correlation(prices_i, prices_j)?;
                
                correlation_matrix.update_correlation(
                    *idx_i as u16,
                    *idx_j as u16,
                    correlation as i64,
                    clock.unix_timestamp,
                    prices_i.len() as u32,
                )?;
            }
        }
        
        // Calculate average correlation
        correlation_matrix.calculate_average_correlation()?;
        correlation_matrix.last_calculated = clock.unix_timestamp;
        correlation_matrix.calculation_version += 1;
        
        // Save back
        borsh::to_writer(&mut correlation_matrix_data.as_mut(), &correlation_matrix)
            .map_err(|_| ProgramError::InvalidAccountData)?;
        
        msg!("Correlations calculated for verse {:?}", verse_id);
        Ok(())
    }
    
    fn process_update_tail_loss(
        accounts: &[AccountInfo],
        _program_id: &Pubkey,
        _verse_id: [u8; 16],
        outcome_count: u32,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let authority_info = next_account_info(account_info_iter)?;
        let engine_info = next_account_info(account_info_iter)?;
        let _verse_tracking_info = next_account_info(account_info_iter)?;
        let correlation_matrix_info = next_account_info(account_info_iter)?;
        let tail_loss_info = next_account_info(account_info_iter)?;
        
        // Load engine and verify authority
        let engine_data = engine_info.try_borrow_data()?;
        let engine: CorrelationEngine = CorrelationEngine::try_from_slice(&engine_data)
            .map_err(|_| ProgramError::InvalidAccountData)?;
        
        if *authority_info.key != engine.authority {
            return Err(CorrelationError::InvalidAuthority.into());
        }
        
        // Load correlation matrix
        let correlation_matrix_data = correlation_matrix_info.try_borrow_data()?;
        let correlation_matrix: CorrelationMatrix =
            Self::borsh_deserialize_unchecked::<CorrelationMatrix>(&correlation_matrix_data)?;
        
        // Load and update tail loss
        let mut tail_loss_data = tail_loss_info.try_borrow_mut_data()?;
        let mut tail_loss: VerseTailLoss = VerseTailLoss::try_from_slice(&tail_loss_data)
            .map_err(|_| ProgramError::InvalidAccountData)?;
        
        let clock = Clock::get()?;
        
        // Update with correlation factor
        tail_loss.update(
            outcome_count,
            correlation_matrix.average_correlation,
            clock.unix_timestamp,
        )?;
        
        // Save back
        borsh::to_writer(&mut tail_loss_data.as_mut(), &tail_loss)
            .map_err(|_| ProgramError::InvalidAccountData)?;
        
        msg!("Tail loss updated: base={}, corr={}, enhanced={}",
            tail_loss.parameters.base_tail_loss,
            tail_loss.parameters.correlation_factor,
            tail_loss.parameters.enhanced_tail_loss
        );
        
        Ok(())
    }
    
    fn process_update_market_weights(
        accounts: &[AccountInfo],
        _program_id: &Pubkey,
        _verse_id: [u8; 16],
        market_weights: Vec<(u16, u64)>,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let authority_info = next_account_info(account_info_iter)?;
        let engine_info = next_account_info(account_info_iter)?;
        let verse_tracking_info = next_account_info(account_info_iter)?;
        
        // Load engine and verify authority
        let engine_data = engine_info.try_borrow_data()?;
        let engine: CorrelationEngine = CorrelationEngine::try_from_slice(&engine_data)
            .map_err(|_| ProgramError::InvalidAccountData)?;
        
        if *authority_info.key != engine.authority {
            return Err(CorrelationError::InvalidAuthority.into());
        }
        
        // Load and update verse tracking
        let mut verse_tracking_data = verse_tracking_info.try_borrow_mut_data()?;
        let mut verse_tracking: VerseTracking =
            Self::borsh_deserialize_unchecked::<VerseTracking>(&verse_tracking_data)?;
        
        // Update weights
        for (market_index, weight) in market_weights {
            if let Some(market_weight) = verse_tracking.market_weights.get_mut(market_index as usize) {
                market_weight.weight = weight;
                market_weight.last_updated = Clock::get()?.unix_timestamp;
            }
        }
        
        // Save back
        borsh::to_writer(&mut verse_tracking_data.as_mut(), &verse_tracking)
            .map_err(|_| ProgramError::InvalidAccountData)?;
        
        msg!("Market weights updated");
        Ok(())
    }
}
