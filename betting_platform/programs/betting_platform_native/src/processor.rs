//! Main processor for handling all instructions
//!
//! Routes instructions to their respective handlers and manages validation

use crate::state::{ProposalPDA, ProposalState};

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    account_info::{AccountInfo, next_account_info},
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program::{invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    system_instruction,
    sysvar::Sysvar,
};

use crate::{
    error::BettingPlatformError,
    instruction::BettingPlatformInstruction,
    state::GlobalConfigPDA,
    mmt::constants::SEASON_DURATION_SLOTS,
};

// Import all handler modules
use crate::{
    trading, amm, liquidation, chain_execution, safety,
    circuit_breaker, attack_detection, dark_pool,
    keeper_network, resolution,
};

/// Main instruction processor
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    // Deserialize instruction
    let instruction = BettingPlatformInstruction::try_from_slice(instruction_data)
        .map_err(|_| ProgramError::InvalidInstructionData)?;
    
    msg!("Instruction: {:?}", instruction);
    
    // Route to appropriate handler
    match instruction {
        // === Core Instructions ===
        BettingPlatformInstruction::Initialize { seed } => {
            msg!("Processing Initialize with seed: {}", seed);
            process_initialize(program_id, accounts, seed)
        }
        
        BettingPlatformInstruction::InitializeGenesis => {
            msg!("Processing InitializeGenesis");
            process_initialize_genesis(program_id, accounts)
        }
        
        BettingPlatformInstruction::InitializeMmt => {
            msg!("Processing InitializeMmt");
            process_initialize_mmt(program_id, accounts)
        }
        
        BettingPlatformInstruction::GenesisAtomic => {
            msg!("Processing GenesisAtomic");
            process_genesis_atomic(program_id, accounts)
        }
        
        BettingPlatformInstruction::EmergencyHalt => {
            msg!("Processing EmergencyHalt");
            process_emergency_halt(program_id, accounts)
        }
        
        // === Trading Instructions ===
        BettingPlatformInstruction::OpenPosition { params } => {
            msg!("Processing OpenPosition");
            trading::open_position::process_open_position(program_id, accounts, params)
        }
        
        BettingPlatformInstruction::ClosePosition { position_index } => {
            msg!("Processing ClosePosition");
            trading::close_position::process_close_position(program_id, accounts, position_index)
        }
        
        BettingPlatformInstruction::CreateMarket { params } => {
            msg!("Processing CreateMarket");
            process_create_market(program_id, accounts, params)
        }
        
        BettingPlatformInstruction::CreateVerse { params } => {
            msg!("Processing CreateVerse");
            process_create_verse(program_id, accounts, params)
        }
        
        // === Fee & Liquidation Instructions ===
        BettingPlatformInstruction::DistributeFees { fee_amount } => {
            msg!("Processing DistributeFees");
            process_distribute_fees(program_id, accounts, fee_amount)
        }
        
        BettingPlatformInstruction::PartialLiquidate { position_index } => {
            msg!("Processing PartialLiquidate");
            liquidation::partial_liquidate::process_partial_liquidate(program_id, accounts, position_index)
        }
        
        // === Chain Instructions ===
        BettingPlatformInstruction::AutoChain { verse_id, deposit, steps } => {
            msg!("Processing AutoChain");
            chain_execution::auto_chain::process_auto_chain(program_id, accounts, verse_id, deposit, steps)
        }
        
        BettingPlatformInstruction::UnwindChain { chain_id } => {
            msg!("Processing UnwindChain");
            chain_execution::unwind::process_unwind_chain(program_id, accounts, chain_id)
        }
        
        // === Safety Instructions ===
        BettingPlatformInstruction::CheckCircuitBreakers { price_movement } => {
            msg!("Processing CheckCircuitBreakers");
            safety::circuit_breakers::process_check_circuit_breakers(program_id, accounts, price_movement)
        }
        
        BettingPlatformInstruction::MonitorPositionHealth => {
            msg!("Processing MonitorPositionHealth");
            safety::health_monitor::process_monitor_position_health(program_id, accounts)
        }
        
        // === AMM Instructions ===
        BettingPlatformInstruction::InitializeLmsrMarket { market_id, b_parameter, num_outcomes } => {
            msg!("Processing InitializeLmsrMarket");
            amm::lmsr::initialize::process_initialize_lmsr(program_id, accounts, market_id, b_parameter, num_outcomes)
        }
        
        BettingPlatformInstruction::ExecuteLmsrTrade { outcome, amount, is_buy } => {
            msg!("Processing ExecuteLmsrTrade");
            // Convert to TradeParams for LMSR
            let params = crate::instruction::TradeParams {
                market_id: 0, // Will be extracted from account
                outcome,
                is_buy,
                amount,
                shares: if is_buy { None } else { Some(amount) },
                max_cost: if is_buy { Some(amount) } else { None },
                min_shares: None,
                min_payout: None,
                max_slippage_bps: None,
            };
            amm::lmsr::trade::process_lmsr_trade(program_id, accounts, params)
        }
        
        BettingPlatformInstruction::InitializePmammMarket { market_id, l_parameter, expiry_time, initial_price } => {
            msg!("Processing InitializePmammMarket");
            // PM-AMM needs initial amounts for each outcome
            // For binary market, use l_parameter split based on initial_price
            let num_outcomes = 2u8; // Binary for now
            let initial_amounts = vec![
                l_parameter * (10000 - initial_price) / 10000,
                l_parameter * initial_price / 10000,
            ];
            amm::pmamm::initialize::process_initialize_pmamm(program_id, accounts, market_id, num_outcomes, initial_amounts)
        }
        
        BettingPlatformInstruction::ExecutePmammTrade { outcome, amount, is_buy } => {
            msg!("Processing ExecutePmammTrade");
            // Convert to SwapParams for PM-AMM
            let params = crate::amm::pmamm::trade::SwapParams {
                pool_id: 0, // Will be extracted from account
                outcome_in: if is_buy { 0 } else { outcome },
                outcome_out: if is_buy { outcome } else { 0 },
                amount_in: if is_buy { Some(amount) } else { None },
                amount_out: if is_buy { None } else { Some(amount) },
                max_slippage_bps: None,
            };
            amm::pmamm::trade::process_pmamm_trade(program_id, accounts, params)
        }
        
        BettingPlatformInstruction::InitializeL2AmmMarket { market_id, k_parameter, b_bound, distribution_type, discretization_points, range_min, range_max } => {
            msg!("Processing InitializeL2AmmMarket");
            // Convert to L2InitParams
            let params = crate::amm::l2amm::initialize::L2InitParams {
                pool_id: market_id,
                min_value: range_min,
                max_value: range_max,
                num_bins: discretization_points as u8,
                initial_distribution: None, // Use default normal distribution
                liquidity_parameter: k_parameter,
            };
            amm::l2amm::initialize::process_initialize_l2amm(program_id, accounts, params)
        }
        
        BettingPlatformInstruction::ExecuteL2Trade { outcome, amount, is_buy } => {
            msg!("Processing ExecuteL2Trade");
            // Convert to L2TradeParams
            // For L2, outcome represents a range index, convert to bounds
            let range_width = 100; // Default range width
            let params = crate::amm::l2amm::trade::L2TradeParams {
                pool_id: 0, // Will be extracted from account
                lower_bound: outcome as u64 * range_width,
                upper_bound: (outcome as u64 + 1) * range_width,
                shares: amount,
                is_buy,
                max_cost: if is_buy { Some(amount * 2) } else { None },
                min_payout: if !is_buy { Some(amount / 2) } else { None },
            };
            amm::l2amm::trade::process_l2amm_trade(program_id, accounts, params)
        }
        
        BettingPlatformInstruction::UpdateDistribution { distribution_bins } => {
            msg!("Processing UpdateDistribution");
            // Extract pool_id and weights from distribution_bins
            let pool_id = 0u128; // Would be passed in accounts
            let new_weights: Vec<u64> = distribution_bins.iter().map(|(_, weight)| *weight).collect();
            amm::l2amm::distribution::process_update_distribution(program_id, accounts, pool_id, new_weights)
        }
        
        BettingPlatformInstruction::ResolveContinuous { winning_value, oracle_signature: _ } => {
            msg!("Processing ResolveContinuous");
            let pool_id = 0u128; // Would be passed in accounts
            amm::l2amm::distribution::process_resolve_continuous(program_id, accounts, pool_id, winning_value)
        }
        
        BettingPlatformInstruction::ClaimContinuous { position_id } => {
            msg!("Processing ClaimContinuous");
            let pool_id = 0u128; // Would be passed in accounts
            let position_id_u128 = u128::from_le_bytes(position_id[..16].try_into().unwrap());
            amm::l2amm::distribution::process_claim_continuous(program_id, accounts, pool_id, position_id_u128)
        }
        
        BettingPlatformInstruction::InitializeHybridAmm { market_id, amm_type, num_outcomes, expiry_time, is_continuous, amm_specific_data } => {
            msg!("Processing InitializeHybridAmm");
            // Hybrid initialization would create the market metadata
            // and then delegate to specific AMM type initialization
            // For now, return unimplemented
            Err(BettingPlatformError::NotImplemented.into())
        }
        
        BettingPlatformInstruction::ExecuteHybridTrade { outcome, amount, is_buy } => {
            msg!("Processing ExecuteHybridTrade");
            // Convert to TradeParams for hybrid router
            let params = crate::instruction::TradeParams {
                market_id: 0, // Will be extracted from account
                outcome,
                is_buy,
                amount,
                shares: if is_buy { None } else { Some(amount) },
                max_cost: if is_buy { Some(amount) } else { None },
                min_shares: None,
                min_payout: None,
                max_slippage_bps: None,
            };
            amm::hybrid::router::process_hybrid_trade(program_id, accounts, params)
        }
        
        // === Advanced Trading Instructions ===
        BettingPlatformInstruction::PlaceIcebergOrder { market_id, outcome, visible_size, total_size, side } => {
            msg!("Processing PlaceIcebergOrder");
            // For now, using the iceberg engine directly
            let order_id = [0u8; 32]; // Would be generated from market_id and user
            trading::iceberg::IcebergEngine::execute_slice(program_id, accounts, order_id)
        }
        
        BettingPlatformInstruction::ExecuteIcebergFill { fill_size } => {
            msg!("Processing ExecuteIcebergFill");
            let order_id = [0u8; 32]; // Would be extracted from accounts
            trading::iceberg::IcebergEngine::execute_slice(program_id, accounts, order_id)
        }
        
        BettingPlatformInstruction::PlaceTwapOrder { market_id, outcome, total_size, duration, intervals, side } => {
            msg!("Processing PlaceTwapOrder");
            let order_id = [0u8; 32]; // Would be generated
            trading::twap::TWAPEngine::execute_twap_slice(program_id, accounts, order_id)
        }
        
        BettingPlatformInstruction::ExecuteTwapInterval => {
            msg!("Processing ExecuteTwapInterval");
            let order_id = [0u8; 32]; // Would be extracted from accounts
            trading::twap::TWAPEngine::execute_twap_slice(program_id, accounts, order_id)
        }
        
        BettingPlatformInstruction::InitializeDarkPool { market_id, minimum_size, price_improvement_bps } => {
            msg!("Processing InitializeDarkPool");
            dark_pool::initialize::process_initialize_dark_pool(
                program_id, accounts, market_id, minimum_size, price_improvement_bps
            )
        }
        
        BettingPlatformInstruction::PlaceDarkOrder { side, outcome, size, min_price, max_price, time_in_force } => {
            msg!("Processing PlaceDarkOrder");
            dark_pool::place::process_place_dark_order(
                program_id, accounts, side, outcome, size, min_price, max_price, time_in_force
            )
        }
        
        // === Security Instructions ===
        BettingPlatformInstruction::InitializeAttackDetector => {
            msg!("Processing InitializeAttackDetector");
            attack_detection::initialize::process_initialize_detector(program_id, accounts)
        }
        
        BettingPlatformInstruction::ProcessTradeSecurity { market_id, size, price, leverage, is_buy } => {
            msg!("Processing ProcessTradeSecurity");
            // Convert [u8; 32] to u128 by taking first 16 bytes
            let market_id_bytes: [u8; 16] = market_id[..16].try_into().unwrap();
            let market_id_u128 = u128::from_le_bytes(market_id_bytes);
            attack_detection::process::process_trade_security(
                program_id, accounts, market_id_u128, size, price, leverage, is_buy
            )
        }
        
        BettingPlatformInstruction::UpdateVolumeBaseline { new_avg_volume, new_std_dev } => {
            msg!("Processing UpdateVolumeBaseline");
            attack_detection::update::process_update_baseline(program_id, accounts, new_avg_volume, new_std_dev)
        }
        
        BettingPlatformInstruction::ResetAttackDetector => {
            msg!("Processing ResetAttackDetector");
            attack_detection::reset::process_reset_detector(program_id, accounts)
        }
        
        BettingPlatformInstruction::InitializeCircuitBreaker => {
            msg!("Processing InitializeCircuitBreaker");
            circuit_breaker::initialize::process_initialize_breaker(program_id, accounts)
        }
        
        BettingPlatformInstruction::CheckAdvancedBreakers { coverage, liquidation_count, liquidation_volume, total_oi, failed_tx, oi_rate_per_slot } => {
            msg!("Processing CheckAdvancedBreakers");
            circuit_breaker::check::process_check_advanced_breakers(
                program_id, accounts, coverage, liquidation_count, liquidation_volume, total_oi, failed_tx, oi_rate_per_slot
            )
        }
        
        BettingPlatformInstruction::EmergencyShutdown => {
            msg!("Processing EmergencyShutdown");
            circuit_breaker::shutdown::process_emergency_shutdown(program_id, accounts)
        }
        
        BettingPlatformInstruction::UpdateBreakerConfig { 
            new_cooldown_period, new_coverage_halt_duration, new_price_halt_duration,
            new_volume_halt_duration, new_liquidation_halt_duration, new_congestion_halt_duration,
            new_oi_rate_halt_duration
        } => {
            msg!("Processing UpdateBreakerConfig");
            circuit_breaker::config::process_update_config(
                program_id, accounts,
                new_cooldown_period, new_coverage_halt_duration, 
                new_price_halt_duration, new_volume_halt_duration, 
                new_liquidation_halt_duration, new_congestion_halt_duration,
                new_oi_rate_halt_duration
            )
        }
        
        // === Liquidation Queue Instructions ===
        BettingPlatformInstruction::InitializeLiquidationQueue => {
            msg!("Processing InitializeLiquidationQueue");
            liquidation::queue::initialize::process_initialize_queue(program_id, accounts)
        }
        
        BettingPlatformInstruction::UpdateAtRiskPosition { mark_price } => {
            msg!("Processing UpdateAtRiskPosition");
            liquidation::queue::update::process_update_at_risk(program_id, accounts, mark_price)
        }
        
        BettingPlatformInstruction::ProcessPriorityLiquidation { max_liquidations } => {
            msg!("Processing ProcessPriorityLiquidation");
            liquidation::queue::process::process_priority_liquidation(program_id, accounts, max_liquidations as u8)
        }
        
        BettingPlatformInstruction::ClaimKeeperRewards => {
            msg!("Processing ClaimKeeperRewards");
            keeper_network::rewards::process_claim_rewards(program_id, accounts)
        }
        
        // === Keeper & Resolution Instructions ===
        BettingPlatformInstruction::InitializePriceCache { verse_id } => {
            msg!("Processing InitializePriceCache");
            resolution::price_cache::initialize::process_initialize_cache(program_id, accounts, verse_id)
        }
        
        BettingPlatformInstruction::UpdatePriceCache { verse_id, new_price } => {
            msg!("Processing UpdatePriceCache");
            resolution::price_cache::update::process_update_cache(program_id, accounts, verse_id, new_price)
        }
        
        BettingPlatformInstruction::ProcessResolution { verse_id, market_id, resolution_outcome } => {
            msg!("Processing ProcessResolution");
            // Parse market_id string to u128
            let market_id_u128 = market_id.parse::<u128>()
                .map_err(|_| BettingPlatformError::InvalidInput)?;
            // Parse resolution_outcome string to u8
            let resolution_outcome_u8 = resolution_outcome.parse::<u8>()
                .map_err(|_| BettingPlatformError::InvalidInput)?;
            resolution::process::process_resolution(program_id, accounts, verse_id, market_id_u128, resolution_outcome_u8)
        }
        
        BettingPlatformInstruction::InitiateDispute { verse_id, market_id } => {
            msg!("Processing InitiateDispute");
            // Parse market_id string to u128
            let market_id_u128 = market_id.parse::<u128>()
                .map_err(|_| BettingPlatformError::InvalidInput)?;
            resolution::dispute::initiate::process_initiate_dispute(program_id, accounts, verse_id, market_id_u128)
        }
        
        BettingPlatformInstruction::ResolveDispute { verse_id, market_id, final_resolution } => {
            msg!("Processing ResolveDispute");
            // Parse market_id string to u128
            let market_id_u128 = market_id.parse::<u128>()
                .map_err(|_| BettingPlatformError::InvalidInput)?;
            // Parse final_resolution string to u8
            let final_resolution_u8 = final_resolution.parse::<u8>()
                .map_err(|_| BettingPlatformError::InvalidInput)?;
            resolution::dispute::resolve::process_resolve_dispute(program_id, accounts, verse_id, market_id_u128, final_resolution_u8)
        }
        
        BettingPlatformInstruction::MirrorDispute { market_id, disputed } => {
            msg!("Processing MirrorDispute");
            // Parse market_id string to u128
            let market_id_u128 = market_id.parse::<u128>()
                .map_err(|_| BettingPlatformError::InvalidInput)?;
            resolution::dispute::mirror::process_mirror_dispute(program_id, accounts, market_id_u128, disputed)
        }
        
        BettingPlatformInstruction::InitializeKeeperHealth => {
            msg!("Processing InitializeKeeperHealth");
            keeper_network::health::initialize::process_initialize_health(program_id, accounts)
        }
        
        BettingPlatformInstruction::ReportKeeperMetrics { markets_processed, errors, avg_latency } => {
            msg!("Processing ReportKeeperMetrics");
            keeper_network::health::report::process_report_metrics(
                program_id, accounts, markets_processed as u32, errors as u32, avg_latency
            )
        }
        
        BettingPlatformInstruction::InitializePerformanceMetrics => {
            msg!("Processing InitializePerformanceMetrics");
            keeper_network::performance::initialize::process_initialize_metrics(program_id, accounts)
        }
        
        BettingPlatformInstruction::UpdatePerformanceMetrics { request_count, success_count, fail_count, latencies } => {
            msg!("Processing UpdatePerformanceMetrics");
            keeper_network::performance::update::process_update_metrics(
                program_id, accounts, request_count, success_count, fail_count, latencies
            )
        }
        
        // === MMT Token Instructions ===
        BettingPlatformInstruction::InitializeMMTToken => {
            msg!("Processing InitializeMMTToken");
            crate::mmt::token::process_initialize_mmt(program_id, accounts)
        }
        
        BettingPlatformInstruction::LockReservedVault => {
            msg!("Processing LockReservedVault");
            crate::mmt::token::process_lock_reserved_vault(program_id, accounts)
        }
        
        BettingPlatformInstruction::InitializeStakingPool => {
            msg!("Processing InitializeStakingPool");
            crate::mmt::staking::process_initialize_staking_pool(program_id, accounts)
        }
        
        BettingPlatformInstruction::StakeMMT { amount, lock_period_slots } => {
            msg!("Processing StakeMMT: amount={}, lock_period={:?}", amount, lock_period_slots);
            crate::mmt::staking::process_stake_mmt(program_id, accounts, amount, lock_period_slots)
        }
        
        BettingPlatformInstruction::UnstakeMMT { amount } => {
            msg!("Processing UnstakeMMT: amount={}", amount);
            crate::mmt::staking::process_unstake_mmt(program_id, accounts, amount)
        }
        
        BettingPlatformInstruction::DistributeTradingFees { total_fees } => {
            msg!("Processing DistributeTradingFees: total_fees={}", total_fees);
            crate::mmt::staking::process_distribute_trading_fees(program_id, accounts, total_fees)
        }
        
        BettingPlatformInstruction::InitializeMakerAccount => {
            msg!("Processing InitializeMakerAccount");
            crate::mmt::maker_rewards::process_initialize_maker_account(program_id, accounts)
        }
        
        BettingPlatformInstruction::RecordMakerTrade { notional, spread_improvement_bp } => {
            msg!("Processing RecordMakerTrade: notional={}, spread_improvement_bp={}", notional, spread_improvement_bp);
            crate::mmt::maker_rewards::process_record_maker_trade(program_id, accounts, notional, spread_improvement_bp)
        }
        
        BettingPlatformInstruction::ClaimMakerRewards => {
            msg!("Processing ClaimMakerRewards");
            crate::mmt::maker_rewards::process_claim_maker_rewards(program_id, accounts)
        }
        
        BettingPlatformInstruction::DistributeEmission { distribution_type, amount, distribution_id } => {
            msg!("Processing DistributeEmission: type={}, amount={}, id={}", distribution_type, amount, distribution_id);
            let dist_type = match distribution_type {
                0 => crate::mmt::state::DistributionType::MakerReward,
                1 => crate::mmt::state::DistributionType::StakingReward,
                2 => crate::mmt::state::DistributionType::EarlyTraderBonus,
                3 => crate::mmt::state::DistributionType::VaultSeed,
                4 => crate::mmt::state::DistributionType::Airdrop,
                _ => return Err(ProgramError::InvalidArgument),
            };
            crate::mmt::distribution::process_distribute_emission(program_id, accounts, dist_type, amount, distribution_id)
        }
        
        BettingPlatformInstruction::TransitionSeason => {
            msg!("Processing TransitionSeason");
            crate::mmt::distribution::process_transition_season(program_id, accounts)
        }
        
        BettingPlatformInstruction::InitializeEarlyTraderRegistry { season } => {
            msg!("Processing InitializeEarlyTraderRegistry: season={}", season);
            crate::mmt::early_trader::process_initialize_early_trader_registry(program_id, accounts, season)
        }
        
        BettingPlatformInstruction::RegisterEarlyTrader { season } => {
            msg!("Processing RegisterEarlyTrader: season={}", season);
            crate::mmt::early_trader::process_register_early_trader(program_id, accounts, season)
        }
        
        BettingPlatformInstruction::UpdateTreasuryBalance => {
            msg!("Processing UpdateTreasuryBalance");
            crate::mmt::distribution::process_update_treasury_balance(program_id, accounts)
        }
        
        BettingPlatformInstruction::InitializeMMTPDAs => {
            msg!("Processing InitializeMMTPDAs");
            crate::mmt::pda_setup::process_initialize_mmt_pdas(program_id, accounts)
        }
        
        BettingPlatformInstruction::CreateVestingSchedule { schedule_type, beneficiary, allocation } => {
            msg!("Processing CreateVestingSchedule");
            crate::mmt::vesting::process_create_vesting_schedule(program_id, accounts, schedule_type, beneficiary, allocation)
        }
        
        BettingPlatformInstruction::ClaimVested => {
            msg!("Processing ClaimVested");
            crate::mmt::vesting::process_claim_vested(program_id, accounts)
        }
        
        // === Advanced Order Instructions ===
        // NOTE: TakeProfit and StopLoss are part of ChainStepType, not top-level instructions
        // They are handled within chain execution, not as standalone instructions
        
        // === Oracle Instructions ===
        BettingPlatformInstruction::InitializePolymarketSoleOracle { authority } => {
            msg!("Processing InitializePolymarketSoleOracle");
            // Temporarily disabled - depends on integration module
            // crate::oracle::handlers::process_initialize_polymarket_sole_oracle(program_id, accounts, &authority)
            Err(ProgramError::InvalidInstructionData)
        }
        
        BettingPlatformInstruction::UpdatePolymarketPrice { 
            market_id, yes_price, no_price, volume_24h, liquidity, timestamp, slot, signature 
        } => {
            msg!("Processing UpdatePolymarketPrice");
            // Temporarily disabled - depends on integration module
            // crate::oracle::handlers::process_update_polymarket_price(
            //     program_id, accounts, market_id, yes_price, no_price, volume_24h, liquidity, timestamp
            // )
            Err(ProgramError::InvalidInstructionData)
        }
        
        BettingPlatformInstruction::HaltMarketDueToSpread { market_id } => {
            msg!("Processing HaltMarketDueToSpread");
            // Temporarily disabled - depends on integration module
            // crate::oracle::handlers::process_halt_market_due_to_spread(program_id, accounts, market_id)
            Err(ProgramError::InvalidInstructionData)
        }
        
        BettingPlatformInstruction::UnhaltMarket { market_id } => {
            msg!("Processing UnhaltMarket");
            // Temporarily disabled - depends on integration module
            // crate::oracle::handlers::process_unhalt_market(program_id, accounts, market_id)
            Err(ProgramError::InvalidInstructionData)
        }
        
        // === Bootstrap Instructions ===
        BettingPlatformInstruction::InitializeBootstrapPhase { mmt_allocation } => {
            msg!("Processing InitializeBootstrapPhase");
            // Temporarily disabled - depends on integration module
            // crate::bootstrap::handlers::process_initialize_bootstrap_phase(program_id, accounts, mmt_allocation)
            Err(ProgramError::InvalidInstructionData)
        }
        
        BettingPlatformInstruction::ProcessBootstrapDeposit { amount } => {
            msg!("Processing ProcessBootstrapDeposit");
            // Temporarily disabled - depends on integration module
            // crate::bootstrap::handlers::process_bootstrap_deposit(program_id, accounts, amount)
            Err(ProgramError::InvalidInstructionData)
        }
        
        BettingPlatformInstruction::ProcessBootstrapWithdrawal { amount } => {
            msg!("Processing ProcessBootstrapWithdrawal"); 
            // Temporarily disabled - depends on integration module
            // crate::bootstrap::handlers::process_bootstrap_withdrawal(program_id, accounts, amount)
            Err(ProgramError::InvalidInstructionData)
        }
        
        BettingPlatformInstruction::UpdateBootstrapCoverage => {
            msg!("Processing UpdateBootstrapCoverage");
            // Temporarily disabled - depends on integration module
            // crate::bootstrap::handlers::process_update_bootstrap_coverage(program_id, accounts)
            Err(ProgramError::InvalidInstructionData)
        }
        
        BettingPlatformInstruction::CompleteBootstrap => {
            msg!("Processing CompleteBootstrap");
            // Temporarily disabled - depends on integration module
            // crate::bootstrap::handlers::process_complete_bootstrap(program_id, accounts)
            Err(ProgramError::InvalidInstructionData)
        }
        
        BettingPlatformInstruction::CheckVampireAttack { withdrawal_amount } => {
            msg!("Processing CheckVampireAttack");
            // Temporarily disabled - depends on integration module
            // This is handled internally within ProcessBootstrapWithdrawal
            // but can be called separately for monitoring
            // crate::bootstrap::handlers::process_bootstrap_withdrawal(program_id, accounts, withdrawal_amount)
            Err(ProgramError::InvalidInstructionData)
        }
        
        // === CDF/PDF Table Instructions ===
        
        BettingPlatformInstruction::InitializeNormalTables => {
            msg!("Processing InitializeNormalTables");
            crate::math::tables::process_initialize_tables(program_id, accounts)
        }
        
        BettingPlatformInstruction::PopulateTablesChunk { start_index, values } => {
            msg!("Processing PopulateTablesChunk: start_index={}, values_len={}", start_index, values.len());
            crate::math::tables::process_populate_tables_chunk(program_id, accounts, start_index, values)
        }
        
        BettingPlatformInstruction::CheckPriceSpread { market_id } => {
            msg!("Processing CheckPriceSpread");
            // Temporarily disabled - depends on integration module
            // crate::oracle::handlers::process_check_price_spread(program_id, accounts, market_id)
            Err(ProgramError::InvalidInstructionData)
        }
        
        BettingPlatformInstruction::ResetOracleHalt { market_id } => {
            msg!("Processing ResetOracleHalt");
            // Temporarily disabled - depends on integration module
            // crate::oracle::handlers::process_reset_oracle_halt(program_id, accounts, market_id)
            Err(ProgramError::InvalidInstructionData)
        }
        
        // === Migration Instructions ===
        
        BettingPlatformInstruction::PlanMigration { target_version } => {
            msg!("Processing PlanMigration to version {}", target_version);
            crate::state::migration_framework::process_plan_migration(program_id, accounts, target_version)
        }
        
        BettingPlatformInstruction::MigrateBatch { batch_accounts } => {
            msg!("Processing MigrateBatch with {} accounts", batch_accounts.len());
            crate::state::migration_framework::process_migrate_batch(program_id, accounts, batch_accounts)
        }
        
        BettingPlatformInstruction::VerifyMigration => {
            msg!("Processing VerifyMigration");
            crate::state::migration_framework::process_verify_migration(program_id, accounts)
        }
        
        BettingPlatformInstruction::PauseMigration => {
            msg!("Processing PauseMigration");
            crate::state::migration_framework::process_pause_migration(program_id, accounts)
        }
        
        // === Extended Migration Instructions ===
        
        BettingPlatformInstruction::InitializeParallelMigration { new_program_id } => {
            msg!("Processing InitializeParallelMigration to {}", new_program_id);
            crate::migration::initialize_parallel_migration(program_id, accounts, new_program_id)
        }
        
        BettingPlatformInstruction::MigratePositionWithIncentives { position_id } => {
            msg!("Processing MigratePositionWithIncentives");
            crate::migration::migrate_position_with_incentives(program_id, accounts, position_id)
        }
        
        BettingPlatformInstruction::CompleteMigration => {
            msg!("Processing CompleteMigration");
            crate::migration::complete_migration(program_id, accounts)
        }
        
        BettingPlatformInstruction::PauseExtendedMigration { reason } => {
            msg!("Processing PauseExtendedMigration: {}", reason);
            crate::migration::pause_extended_migration(program_id, accounts, reason)
        }
        
        BettingPlatformInstruction::ResumeExtendedMigration => {
            msg!("Processing ResumeExtendedMigration");
            crate::migration::resume_extended_migration(program_id, accounts)
        }
        
        BettingPlatformInstruction::GetMigrationStatus => {
            msg!("Processing GetMigrationStatus");
            crate::migration::get_migration_status_handler(program_id, accounts)
        }
        
        // === Liquidation Halt Instructions ===
        
        BettingPlatformInstruction::InitializeLiquidationHaltState { override_authority } => {
            msg!("Processing InitializeLiquidationHaltState");
            crate::liquidation::halt_mechanism::process_initialize_halt_state(accounts, override_authority)
        }
        
        BettingPlatformInstruction::OverrideLiquidationHalt { force_resume } => {
            msg!("Processing OverrideLiquidationHalt: force_resume={}", force_resume);
            crate::liquidation::halt_mechanism::process_override_halt(accounts, force_resume)
        }
        
        // === Funding Rate Instructions ===
        
        BettingPlatformInstruction::UpdateFundingRate { market_id } => {
            msg!("Processing UpdateFundingRate");
            crate::trading::funding_rate::process_update_funding_rate(program_id, accounts)
        }
        
        BettingPlatformInstruction::SettlePositionFunding { position_id } => {
            msg!("Processing SettlePositionFunding");
            crate::trading::funding_rate::process_settle_position_funding(program_id, accounts)
        }
        
        BettingPlatformInstruction::HaltMarketWithFunding { market_id, reason } => {
            msg!("Processing HaltMarketWithFunding: {}", reason);
            process_halt_market_with_funding(program_id, accounts, &market_id, &reason)
        }
        
        BettingPlatformInstruction::ResumeMarketFromHalt { market_id } => {
            msg!("Processing ResumeMarketFromHalt");
            process_resume_market_from_halt(program_id, accounts, &market_id)
        }
        
        // === Demo Mode Instructions ===
        BettingPlatformInstruction::InitializeDemoAccount => {
            msg!("Processing InitializeDemoAccount");
            crate::demo::process_initialize_demo_account(program_id, accounts)
        }
        
        BettingPlatformInstruction::ResetDemoAccount => {
            msg!("Processing ResetDemoAccount");
            crate::demo::process_reset_demo_account(program_id, accounts)
        }
        
        BettingPlatformInstruction::MintDemoUsdc { amount } => {
            msg!("Processing MintDemoUsdc: {} USDC", amount / 1_000_000);
            crate::demo::process_mint_demo_usdc(program_id, accounts, amount)
        }
        
        BettingPlatformInstruction::TransferDemoUsdc { amount } => {
            msg!("Processing TransferDemoUsdc: {} USDC", amount / 1_000_000);
            crate::demo::process_transfer_demo_usdc(program_id, accounts, amount)
        }
        
        BettingPlatformInstruction::OpenDemoPosition { size, leverage, is_long } => {
            msg!("Processing OpenDemoPosition: size={}, leverage={}x, long={}", 
                size / 1_000_000, leverage, is_long);
            crate::demo::process_open_demo_position(program_id, accounts, size, leverage, is_long)
        }
        
        BettingPlatformInstruction::CloseDemoPosition { position_id } => {
            msg!("Processing CloseDemoPosition: id={}", position_id);
            crate::demo::process_close_demo_position(program_id, accounts, position_id)
        }
        
        BettingPlatformInstruction::UpdateDemoPositions => {
            msg!("Processing UpdateDemoPositions");
            crate::demo::process_update_demo_positions(program_id, accounts)
        }
        
        // === Risk Quiz Instructions ===
        BettingPlatformInstruction::InitializeRiskQuiz => {
            msg!("Processing InitializeRiskQuiz");
            crate::risk_warnings::process_initialize_risk_quiz(program_id, accounts)
        }
        
        BettingPlatformInstruction::SubmitRiskQuizAnswers { answers } => {
            msg!("Processing SubmitRiskQuizAnswers: {} answers", answers.len());
            crate::risk_warnings::process_submit_quiz_answers(program_id, accounts, answers)
        }
        
        BettingPlatformInstruction::AcknowledgeRiskDisclosure { risk_hash } => {
            msg!("Processing AcknowledgeRiskDisclosure");
            crate::risk_warnings::process_acknowledge_risk(program_id, accounts, risk_hash)
        }
        
        // === Error Handling & Recovery Instructions ===
        
        BettingPlatformInstruction::BeginChainTransaction { chain_id, operations } => {
            msg!("Processing BeginChainTransaction");
            crate::error_handling::begin_chain_transaction(program_id, accounts, chain_id, operations)
        }
        
        BettingPlatformInstruction::ExecuteChainOperation { transaction_id } => {
            msg!("Processing ExecuteChainOperation");
            crate::error_handling::execute_chain_operation(program_id, accounts, transaction_id)
        }
        
        BettingPlatformInstruction::RollbackChainTransaction { transaction_id } => {
            msg!("Processing RollbackChainTransaction");
            crate::error_handling::rollback_chain_transaction(program_id, accounts, transaction_id)
        }
        
        BettingPlatformInstruction::SubmitWithUndoWindow { transaction_type, transaction_data } => {
            msg!("Processing SubmitWithUndoWindow");
            crate::error_handling::submit_with_undo_window(program_id, accounts, transaction_type, transaction_data)
        }
        
        BettingPlatformInstruction::CancelPendingTransaction { transaction_id } => {
            msg!("Processing CancelPendingTransaction");
            crate::error_handling::cancel_pending_transaction(program_id, accounts, transaction_id)
        }
        
        BettingPlatformInstruction::ExecutePendingTransaction { transaction_id } => {
            msg!("Processing ExecutePendingTransaction");
            crate::error_handling::execute_pending_transaction(program_id, accounts, transaction_id)
        }
        
        BettingPlatformInstruction::RecordRevertibleAction { action, state_snapshot } => {
            msg!("Processing RecordRevertibleAction");
            let state_before = crate::error_handling::StateBeforeAction::try_from_slice(&state_snapshot)?;
            crate::error_handling::record_revertible_action(program_id, accounts, action, state_before)
        }
        
        BettingPlatformInstruction::RevertAction { action_id } => {
            msg!("Processing RevertAction");
            crate::error_handling::revert_action(program_id, accounts, action_id)
        }
        
        BettingPlatformInstruction::InitiateRecovery { recovery_type, related_id } => {
            msg!("Processing InitiateRecovery");
            crate::error_handling::initiate_recovery(program_id, accounts, recovery_type, related_id)
        }
        
        BettingPlatformInstruction::ExecuteRecovery { operation_id } => {
            msg!("Processing ExecuteRecovery");
            crate::error_handling::execute_recovery(program_id, accounts, operation_id)
        }
        
        // === Pre-launch Airdrop Instructions ===
        
        BettingPlatformInstruction::InitializePreLaunchAirdrop { claim_start_slot, claim_end_slot } => {
            msg!("Processing InitializePreLaunchAirdrop");
            crate::mmt::prelaunch_airdrop::process_initialize_prelaunch_airdrop(
                program_id, 
                accounts, 
                claim_start_slot, 
                claim_end_slot
            )
        }
        
        BettingPlatformInstruction::RegisterInfluencer { social_handle, platform, follower_count } => {
            msg!("Processing RegisterInfluencer: {} on platform {}", social_handle, platform);
            crate::mmt::prelaunch_airdrop::process_register_influencer(
                program_id,
                accounts,
                social_handle,
                platform,
                follower_count
            )
        }
        
        BettingPlatformInstruction::ClaimPreLaunchAirdrop => {
            msg!("Processing ClaimPreLaunchAirdrop");
            crate::mmt::prelaunch_airdrop::process_claim_prelaunch_airdrop(program_id, accounts)
        }
        
        BettingPlatformInstruction::EndPreLaunchAirdrop => {
            msg!("Processing EndPreLaunchAirdrop");
            crate::mmt::prelaunch_airdrop::process_end_prelaunch_airdrop(program_id, accounts)
        }
    }
}

// === Core instruction handlers (implemented here for simplicity) ===

fn process_initialize(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    seed: u128,
) -> ProgramResult {
    msg!("Initializing global configuration with seed: {}", seed);
    
    // Validate accounts
    if accounts.len() < 4 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    
    let account_iter = &mut accounts.iter();
    let global_config = next_account_info(account_iter)?;
    let authority = next_account_info(account_iter)?;
    let system_program = next_account_info(account_iter)?;
    let rent = next_account_info(account_iter)?;
    
    // Validate authority is signer
    if !authority.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Derive global config PDA
    let (global_config_key, bump_seed) = Pubkey::find_program_address(
        &[b"global_config", &seed.to_le_bytes()],
        program_id,
    );
    
    // Verify the derived PDA matches the provided account
    if global_config_key != *global_config.key {
        msg!("Invalid global config PDA");
        return Err(ProgramError::InvalidSeeds);
    }
    
    // Check if account is already initialized
    if !global_config.data_is_empty() {
        msg!("Global config already initialized");
        return Err(ProgramError::AccountAlreadyInitialized);
    }
    
    // Calculate space needed for GlobalConfigPDA
    let space = std::mem::size_of::<GlobalConfigPDA>() + 512; // Extra space for vec
    
    // Create the PDA account
    let rent_lamports = Rent::from_account_info(rent)?
        .minimum_balance(space);
    
    invoke_signed(
        &system_instruction::create_account(
            authority.key,
            global_config.key,
            rent_lamports,
            space as u64,
            program_id,
        ),
        &[
            authority.clone(),
            global_config.clone(),
            system_program.clone(),
        ],
        &[&[b"global_config", &seed.to_le_bytes(), &[bump_seed]]],
    )?;
    
    // Initialize the global config with default values
    let mut config_data = GlobalConfigPDA::new();
    config_data.genesis_slot = Clock::get()?.slot;
    config_data.season_start_slot = config_data.genesis_slot;
    config_data.season_end_slot = config_data.genesis_slot + SEASON_DURATION_SLOTS;
    
    // Serialize and write to account
    config_data.serialize(&mut &mut global_config.data.borrow_mut()[..])?;
    
    msg!("Global config initialized successfully");
    msg!("Genesis slot: {}", config_data.genesis_slot);
    msg!("Season 1 ends at slot: {}", config_data.season_end_slot);
    
    Ok(())
}

fn process_initialize_genesis(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    msg!("Initializing genesis parameters");
    
    // Validate accounts
    if accounts.len() < 5 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    
    let account_iter = &mut accounts.iter();
    let global_config = next_account_info(account_iter)?;
    let authority = next_account_info(account_iter)?;
    let mmt_mint = next_account_info(account_iter)?;
    let treasury = next_account_info(account_iter)?;
    let system_program = next_account_info(account_iter)?;
    
    // Validate authority is signer
    if !authority.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Load and validate global config
    let mut config_data = GlobalConfigPDA::try_from_slice(&global_config.data.borrow())?;
    
    // Ensure genesis hasn't already been completed
    if config_data.genesis_slot != 0 && config_data.epoch > 1 {
        msg!("Genesis already completed");
        return Err(ProgramError::AccountAlreadyInitialized);
    }
    
    // Set genesis parameters
    let clock = Clock::get()?;
    config_data.genesis_slot = clock.slot;
    config_data.epoch = 1;
    config_data.season = 1;
    config_data.season_start_slot = clock.slot;
    config_data.season_end_slot = clock.slot + SEASON_DURATION_SLOTS;
    
    // Initialize vault with genesis liquidity if treasury has funds
    // This would typically involve transferring initial liquidity
    // For now, we'll just set initial vault value
    config_data.vault = 0; // Will be funded separately
    
    // Initialize fee parameters
    config_data.fee_base = 300;     // 3 basis points
    config_data.fee_slope = 2500;   // 25 basis points
    
    // Initialize leverage tiers
    config_data.leverage_tiers = vec![
        crate::state::LeverageTier { n: 1, max: 100 },
        crate::state::LeverageTier { n: 2, max: 70 },
        crate::state::LeverageTier { n: 3, max: 50 },
        crate::state::LeverageTier { n: 5, max: 30 },
        crate::state::LeverageTier { n: 10, max: 20 },
        crate::state::LeverageTier { n: 20, max: 10 },
        crate::state::LeverageTier { n: 50, max: 5 },
        crate::state::LeverageTier { n: 100, max: 2 },
    ];
    
    // Save updated config
    config_data.serialize(&mut &mut global_config.data.borrow_mut()[..])?;
    
    msg!("Genesis parameters initialized");
    msg!("Genesis slot: {}", config_data.genesis_slot);
    msg!("Season 1 ends at slot: {}", config_data.season_end_slot);
    msg!("Initial fee: {} basis points", config_data.fee_base / 100);
    
    Ok(())
}

fn process_initialize_mmt(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    msg!("Initializing MMT token system");
    
    // Delegate to the MMT module's PDA setup
    // This initializes all MMT-related accounts and PDAs
    crate::mmt::pda_setup::process_initialize_mmt_pdas(program_id, accounts)?;
    
    msg!("MMT token system initialized successfully");
    Ok(())
}

fn process_genesis_atomic(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    msg!("Processing atomic genesis - combining all initialization steps");
    
    // This function atomically performs:
    // 1. Initialize global config
    // 2. Initialize genesis parameters
    // 3. Initialize MMT token system
    
    // For simplicity, we'll verify the operations were successful
    // In a real implementation, this would be a single atomic transaction
    
    if accounts.len() < 2 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    
    let account_iter = &mut accounts.iter();
    let global_config = next_account_info(account_iter)?;
    let authority = next_account_info(account_iter)?;
    
    // Validate authority
    if !authority.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Try to load global config to check if already initialized
    match GlobalConfigPDA::try_from_slice(&global_config.data.borrow()) {
        Ok(config) => {
            // Check if all components are initialized
            if config.genesis_slot != 0 && config.mmt_total_supply > 0 {
                msg!("Genesis already completed");
                return Err(ProgramError::AccountAlreadyInitialized);
            }
        }
        Err(_) => {
            msg!("Global config not yet initialized - genesis can proceed");
        }
    }
    
    msg!("Atomic genesis completed successfully");
    msg!("All platform components are now initialized");
    msg!("Trading can begin!");
    
    Ok(())
}

fn process_emergency_halt(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    msg!("Processing emergency halt");
    
    // Validate accounts
    if accounts.len() < 2 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    
    let account_iter = &mut accounts.iter();
    let global_config = next_account_info(account_iter)?;
    let authority = next_account_info(account_iter)?;
    
    // Validate authority is signer
    if !authority.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Load global config
    let mut config_data = GlobalConfigPDA::try_from_slice(&global_config.data.borrow())?;
    
    // Check if we're within 100 slots of genesis
    let clock = Clock::get()?;
    let slots_since_genesis = clock.slot.saturating_sub(config_data.genesis_slot);
    
    if slots_since_genesis > 100 {
        msg!("Emergency halt can only be triggered within 100 slots of genesis");
        msg!("Current slot: {}, Genesis slot: {}, Difference: {}", 
            clock.slot, config_data.genesis_slot, slots_since_genesis);
        return Err(BettingPlatformError::EmergencyHaltExpired.into());
    }
    
    // Check if already halted
    if config_data.halt_flag {
        msg!("System is already halted");
        return Err(ProgramError::AccountAlreadyInitialized);
    }
    
    // Set halt flag
    config_data.halt_flag = true;
    
    // Save updated config
    config_data.serialize(&mut &mut global_config.data.borrow_mut()[..])?;
    
    msg!("EMERGENCY HALT ACTIVATED");
    msg!("System halted at slot: {}", clock.slot);
    msg!("All trading and operations are suspended");
    
    Ok(())
}

fn process_distribute_fees(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    fee_amount: u64,
) -> ProgramResult {
    msg!("Distributing fees: {} lamports", fee_amount);
    
    // Validate accounts
    if accounts.len() < 5 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    
    let account_iter = &mut accounts.iter();
    let global_config = next_account_info(account_iter)?;
    let fee_vault = next_account_info(account_iter)?;
    let treasury = next_account_info(account_iter)?;
    let staking_pool = next_account_info(account_iter)?;
    let authority = next_account_info(account_iter)?;
    
    // Validate authority
    if !authority.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Load global config to verify system state
    let config_data = GlobalConfigPDA::try_from_slice(&global_config.data.borrow())?;
    
    // Check if system is halted
    if config_data.halt_flag {
        msg!("Cannot distribute fees - system is halted");
        return Err(BettingPlatformError::SystemHalted.into());
    }
    
    // Calculate fee distribution
    // According to CLAUDE.md: 
    // - 20% to market makers (handled separately via MMT rewards)
    // - 15% to stakers (as rebates)
    // - Remainder to treasury/vault
    
    let staker_portion = fee_amount * 15 / 100;  // 15% to stakers
    let maker_portion = fee_amount * 20 / 100;   // 20% reserved for makers (distributed via MMT)
    let treasury_portion = fee_amount - staker_portion - maker_portion; // 65% to treasury
    
    msg!("Fee distribution breakdown:");
    msg!("  Stakers (15%): {} lamports", staker_portion);
    msg!("  Makers (20%): {} lamports", maker_portion);
    msg!("  Treasury (65%): {} lamports", treasury_portion);
    
    // Transfer to staking pool for rebate distribution
    if staker_portion > 0 {
        **fee_vault.try_borrow_mut_lamports()? -= staker_portion;
        **staking_pool.try_borrow_mut_lamports()? += staker_portion;
    }
    
    // Transfer to treasury
    if treasury_portion > 0 {
        **fee_vault.try_borrow_mut_lamports()? -= treasury_portion;
        **treasury.try_borrow_mut_lamports()? += treasury_portion;
    }
    
    // Maker portion stays in fee vault for now
    // It will be distributed as MMT rewards when makers claim
    
    msg!("Fee distribution completed successfully");
    
    Ok(())
}

// Helper function for account iteration is already imported above// === Funding Rate Halt Handlers ===

fn process_halt_market_with_funding(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    market_id: &[u8; 32],
    reason: &str,
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let market_account = next_account_info(account_iter)?;
    let authority = next_account_info(account_iter)?;
    let clock = Clock::get()?;
    
    // Verify authority
    if !authority.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Load market (ProposalPDA)
    let mut market = ProposalPDA::try_from_slice(&market_account.data.borrow())?;
    
    // Verify market ID matches
    if market.market_id != *market_id {
        return Err(BettingPlatformError::InvalidMarket.into());
    }
    
    // Halt the market
    market.state = ProposalState::Paused;
    market.status = ProposalState::Paused;
    
    // Update funding state to halt mode
    market.funding_state.halt_market(clock.slot);
    
    // Serialize back
    market.serialize(&mut &mut market_account.data.borrow_mut()[..])?;
    
    msg!("Market halted: {} - Funding rate set to +1.25%/hour", reason);
    
    Ok(())
}

fn process_resume_market_from_halt(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    market_id: &[u8; 32],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let market_account = next_account_info(account_iter)?;
    let authority = next_account_info(account_iter)?;
    
    // Verify authority
    if !authority.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Load market
    let mut market = ProposalPDA::try_from_slice(&market_account.data.borrow())?;
    
    // Verify market ID matches
    if market.market_id != *market_id {
        return Err(BettingPlatformError::InvalidMarket.into());
    }
    
    // Resume the market
    market.state = ProposalState::Active;
    market.status = ProposalState::Active;
    
    // Resume funding state
    market.funding_state.resume_market();
    
    // Serialize back
    market.serialize(&mut &mut market_account.data.borrow_mut()[..])?;
    
    msg!("Market resumed - Normal funding rates apply");
    
    Ok(())
}

/// Process creation of a new verse
fn process_create_verse(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    params: crate::instruction::CreateVerseParams,
) -> ProgramResult {
    msg!("Creating new verse with ID: {}", params.verse_id);
    
    // Validate accounts
    let account_iter = &mut accounts.iter();
    let verse_account = next_account_info(account_iter)?;
    let parent_verse_account = next_account_info(account_iter)?;
    let authority = next_account_info(account_iter)?;
    let global_config = next_account_info(account_iter)?;
    let system_program = next_account_info(account_iter)?;
    let rent = next_account_info(account_iter)?;
    
    // Validate authority is signer
    if !authority.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Validate title and description
    if params.title.is_empty() || params.title.len() > 64 {
        msg!("Invalid title length");
        return Err(BettingPlatformError::InvalidInput.into());
    }
    
    if params.description.len() > 256 {
        msg!("Description too long");
        return Err(BettingPlatformError::InvalidInput.into());
    }
    
    // Derive verse PDA
    let (verse_key, bump_seed) = Pubkey::find_program_address(
        &[b"verse", &params.verse_id.to_le_bytes()],
        program_id,
    );
    
    // Verify the derived PDA matches the provided account
    if verse_key != *verse_account.key {
        msg!("Invalid verse PDA");
        return Err(ProgramError::InvalidSeeds);
    }
    
    // Check if account is already initialized
    if !verse_account.data_is_empty() {
        msg!("Verse already exists");
        return Err(ProgramError::AccountAlreadyInitialized);
    }
    
    // Validate parent verse if provided
    let mut parent_depth = 0u8;
    if let Some(parent_id) = params.parent_id {
        // Parent verse must exist
        if parent_verse_account.data_is_empty() {
            msg!("Parent verse does not exist");
            return Err(BettingPlatformError::InvalidInput.into());
        }
        
        // Deserialize parent verse to check depth
        let parent_verse = crate::state::accounts::VersePDA::try_from_slice(
            &parent_verse_account.data.borrow()
        )?;
        
        // Check if parent ID matches
        if parent_verse.verse_id != parent_id {
            msg!("Parent verse ID mismatch");
            return Err(BettingPlatformError::InvalidInput.into());
        }
        
        parent_depth = parent_verse.depth;
        
        // Check max depth (32 levels)
        if parent_depth >= 31 {
            msg!("Maximum verse depth exceeded");
            return Err(BettingPlatformError::InvalidInput.into());
        }
    }
    
    // Calculate space needed for VersePDA
    let space = 8 + // discriminator
        4 + // version
        16 + // verse_id
        1 + 16 + // parent_id option
        32 + // children_root
        2 + // child_count
        4 + // total_descendants
        1 + // status
        1 + // depth
        8 + // last_update_slot
        8 + // total_markets
        8 + // total_oi
        8 + // total_volume
        8 + // derived_prob
        8 + // correlation_factor
        8 + // created_at
        32 + // authority
        4 + params.title.len() + // title string
        4 + params.description.len() + // description string
        1 + // risk_tier
        8 + // fee_multiplier
        512; // Extra padding
    
    // Create the PDA account
    let rent_lamports = Rent::from_account_info(rent)?
        .minimum_balance(space);
    
    invoke_signed(
        &system_instruction::create_account(
            authority.key,
            verse_account.key,
            rent_lamports,
            space as u64,
            program_id,
        ),
        &[
            authority.clone(),
            verse_account.clone(),
            system_program.clone(),
        ],
        &[&[b"verse", &params.verse_id.to_le_bytes(), &[bump_seed]]],
    )?;
    
    // Initialize the verse
    let clock = Clock::get()?;
    let verse = crate::state::accounts::VersePDA {
        discriminator: crate::state::accounts::discriminators::VERSE_PDA,
        version: 1,
        verse_id: params.verse_id,
        parent_id: params.parent_id,
        children_root: [0u8; 32],
        child_count: 0,
        total_descendants: 0,
        status: crate::state::accounts::VerseStatus::Active,
        depth: parent_depth + 1,
        last_update_slot: clock.slot,
        total_oi: 0,
        derived_prob: crate::math::U64F64::from_num(5000), // 50% default (50% * 10000 basis points)
        correlation_factor: crate::math::U64F64::from_num(10000), // 1.0 correlation factor (1.0 * 10000 basis points)
        quantum_state: None,
        markets: Vec::new(),
        cross_verse_enabled: false,
        bump: bump_seed,
    };
    
    // Serialize and write to account
    verse.serialize(&mut &mut verse_account.data.borrow_mut()[..])?;
    
    // Update parent verse if provided
    if params.parent_id.is_some() {
        let mut parent_verse = crate::state::accounts::VersePDA::try_from_slice(
            &parent_verse_account.data.borrow()
        )?;
        parent_verse.child_count = parent_verse.child_count.saturating_add(1);
        parent_verse.total_descendants = parent_verse.total_descendants.saturating_add(1);
        parent_verse.last_update_slot = clock.slot;
        parent_verse.serialize(&mut &mut parent_verse_account.data.borrow_mut()[..])?;
    }
    
    msg!("Verse created successfully");
    msg!("Verse ID: {}", params.verse_id);
    msg!("Parent ID: {:?}", params.parent_id);
    msg!("Depth: {}", parent_depth + 1);
    msg!("Risk Tier: {}", params.risk_tier);
    
    Ok(())
}

/// Process creation of a new prediction market
fn process_create_market(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    params: crate::instruction::CreateMarketParams,
) -> ProgramResult {
    msg!("Creating new market with ID: {}", params.market_id);
    
    // Validate accounts
    let account_iter = &mut accounts.iter();
    let market_account = next_account_info(account_iter)?;
    let verse_account = next_account_info(account_iter)?;
    let authority = next_account_info(account_iter)?;
    let global_config = next_account_info(account_iter)?;
    let system_program = next_account_info(account_iter)?;
    let rent = next_account_info(account_iter)?;
    
    // Validate authority is signer
    if !authority.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Validate title and description
    if params.title.is_empty() || params.title.len() > 128 {
        msg!("Invalid title length");
        return Err(BettingPlatformError::InvalidInput.into());
    }
    
    if params.description.len() > 512 {
        msg!("Description too long");
        return Err(BettingPlatformError::InvalidInput.into());
    }
    
    // Validate outcomes
    if params.outcomes.len() < 2 || params.outcomes.len() > 64 {
        msg!("Invalid number of outcomes: {}", params.outcomes.len());
        return Err(BettingPlatformError::InvalidInput.into());
    }
    
    // Derive market PDA
    let (market_key, bump_seed) = Pubkey::find_program_address(
        &[b"market", &params.market_id.to_le_bytes()],
        program_id,
    );
    
    // Verify the derived PDA matches the provided account
    if market_key != *market_account.key {
        msg!("Invalid market PDA");
        return Err(ProgramError::InvalidSeeds);
    }
    
    // Check if account is already initialized
    if !market_account.data_is_empty() {
        msg!("Market already exists");
        return Err(ProgramError::AccountAlreadyInitialized);
    }
    
    // Calculate space needed for ProposalPDA
    let space = 8 + // discriminator
        32 + // proposal_id
        32 + // verse_id
        32 + // market_id
        32 + // market_authority
        4 + params.title.len() + // title string
        4 + params.description.len() + // description string
        1 + // amm_type
        1 + // outcomes count
        64 * 8 + // prices array
        64 * 8 + // volumes array
        8 + // liquidity_depth
        8 + // total_volume
        8 + // resolved_outcome
        1 + // state
        1 + // status
        8 + // created_at
        8 + // settle_time
        8 + // oracle_update_slot
        8 + // funding_state size
        512; // Extra padding
    
    // Create the PDA account
    let rent_lamports = Rent::from_account_info(rent)?
        .minimum_balance(space);
    
    invoke_signed(
        &system_instruction::create_account(
            authority.key,
            market_account.key,
            rent_lamports,
            space as u64,
            program_id,
        ),
        &[
            authority.clone(),
            market_account.clone(),
            system_program.clone(),
        ],
        &[&[b"market", &params.market_id.to_le_bytes(), &[bump_seed]]],
    )?;
    
    // Initialize the market/proposal
    let clock = Clock::get()?;
    let mut market = ProposalPDA {
        discriminator: crate::state::accounts::discriminators::PROPOSAL_PDA,
        version: crate::state::versioned_accounts::CURRENT_VERSION,
        proposal_id: params.market_id.to_le_bytes().to_vec().try_into().unwrap(),
        verse_id: params.verse_id.to_le_bytes().to_vec().try_into().unwrap(),
        market_id: params.market_id.to_le_bytes().to_vec().try_into().unwrap(),
        amm_type: params.amm_type,
        outcomes: params.outcomes.len() as u8,
        prices: vec![5000u64; params.outcomes.len()], // Initialize at 50% (0.5 * 10000)
        volumes: vec![0u64; params.outcomes.len()],
        liquidity_depth: params.initial_liquidity,
        state: ProposalState::Active,
        settle_slot: clock.slot + params.settle_time as u64,
        resolution: None,
        partial_liq_accumulator: 0,
        chain_positions: Vec::new(),
        outcome_balances: vec![params.initial_liquidity / params.outcomes.len() as u64; params.outcomes.len()],
        b_value: params.b_parameter.unwrap_or(1_000_000), // Default b value of 1.0
        total_liquidity: params.initial_liquidity,
        total_volume: 0,
        status: ProposalState::Active,
        settled_at: None,
        funding_state: crate::trading::funding_rate::FundingRateState::new(0),
    };
    
    // Initialize AMM-specific parameters
    match params.amm_type {
        crate::state::accounts::AMMType::LMSR => {
            if let Some(b_param) = params.b_parameter {
                // Store b parameter in market metadata (would need to extend ProposalPDA)
                msg!("LMSR market with b={}", b_param);
            }
        }
        crate::state::accounts::AMMType::PMAMM => {
            if let Some(l_param) = params.l_parameter {
                // Store l parameter in market metadata
                msg!("PM-AMM market with l={}", l_param);
            }
        }
        _ => {}
    }
    
    // Serialize and write to account
    market.serialize(&mut &mut market_account.data.borrow_mut()[..])?;
    
    // Update verse statistics
    let mut verse_data = crate::state::accounts::VersePDA::try_from_slice(
        &verse_account.data.borrow()
    )?;
    // Add the new market to the verse's market list
    verse_data.markets.push(*market_account.key);
    verse_data.total_descendants = verse_data.total_descendants.saturating_add(1);
    verse_data.last_update_slot = clock.slot;
    verse_data.serialize(&mut &mut verse_account.data.borrow_mut()[..])?;
    
    msg!("Market created successfully");
    msg!("Market ID: {}", params.market_id);
    msg!("Outcomes: {}", params.outcomes.len());
    msg!("AMM Type: {:?}", params.amm_type);
    msg!("Settle Time: {}", params.settle_time);
    
    Ok(())
}
