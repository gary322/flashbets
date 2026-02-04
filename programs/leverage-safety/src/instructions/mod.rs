use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    pubkey::Pubkey,
    instruction::{AccountMeta, Instruction},
    program_error::ProgramError,
};

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub enum LeverageSafetyInstruction {
    /// Initialize the leverage safety configuration
    /// Accounts:
    /// 0. `[signer]` Authority
    /// 1. `[writable]` Safety config account
    /// 2. `[]` System program
    /// 3. `[]` Rent sysvar
    InitializeSafetyConfig {
        max_base_leverage: u64,
        max_effective_leverage: u64,
    },
    
    /// Update safety parameters
    /// Accounts:
    /// 0. `[signer]` Authority
    /// 1. `[writable]` Safety config account
    UpdateSafetyParameters {
        max_base_leverage: Option<u64>,
        max_effective_leverage: Option<u64>,
        chain_depth_multiplier: Option<u64>,
        coverage_minimum: Option<u64>,
        correlation_penalty: Option<u64>,
        volatility_adjustment: Option<bool>,
    },
    
    /// Update liquidation parameters
    /// Accounts:
    /// 0. `[signer]` Authority
    /// 1. `[writable]` Safety config account
    UpdateLiquidationParameters {
        partial_liq_percent: Option<u16>,
        liq_buffer_bps: Option<u16>,
        min_health_ratio: Option<u64>,
        liquidation_fee_bps: Option<u16>,
        liquidation_cooldown: Option<u64>,
    },
    
    /// Initialize position health tracking
    /// Accounts:
    /// 0. `[signer]` Payer
    /// 1. `[writable]` Position health account
    /// 2. `[]` System program
    /// 3. `[]` Rent sysvar
    InitializePositionHealth {
        position_id: [u8; 32],
        market_id: [u8; 32],
        trader: Pubkey,
        entry_price: u64,
        side: bool,
        base_leverage: u64,
    },
    
    /// Monitor high leverage position
    /// Accounts:
    /// 0. `[signer]` Monitor authority (keeper)
    /// 1. `[]` Safety config account
    /// 2. `[writable]` Position health account
    /// 3. `[writable]` Liquidation queue account (optional)
    /// 4. `[]` Clock sysvar
    MonitorPosition {
        current_price: u64,
        price_staleness_threshold: i64,
    },
    
    /// Add chain step to position
    /// Accounts:
    /// 0. `[signer]` Authority or trader
    /// 1. `[]` Safety config account
    /// 2. `[writable]` Position health account
    /// 3. `[]` Clock sysvar
    AddChainStep {
        step_type: u8, // 0=Borrow, 1=Liquidity, 2=Stake
    },
    
    /// Process partial liquidation
    /// Accounts:
    /// 0. `[signer]` Liquidator
    /// 1. `[]` Safety config account
    /// 2. `[writable]` Position health account
    /// 3. `[writable]` Liquidation queue account
    /// 4. `[]` Clock sysvar
    ProcessPartialLiquidation {
        liquidation_amount: u64,
    },
    
    /// Initialize liquidation queue
    /// Accounts:
    /// 0. `[signer]` Authority
    /// 1. `[writable]` Liquidation queue account
    /// 2. `[]` System program
    /// 3. `[]` Rent sysvar
    InitializeLiquidationQueue,
    
    /// Toggle emergency halt
    /// Accounts:
    /// 0. `[signer]` Authority
    /// 1. `[writable]` Safety config account
    ToggleEmergencyHalt {
        halt: bool,
    },
    
    /// Update tier caps
    /// Accounts:
    /// 0. `[signer]` Authority
    /// 1. `[writable]` Safety config account
    UpdateTierCaps {
        tier_caps: Vec<(u8, u8, u64)>, // (min_outcomes, max_outcomes, max_leverage)
    },
}

impl LeverageSafetyInstruction {
    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        let (&variant, rest) = input.split_first()
            .ok_or(ProgramError::InvalidInstructionData)?;
        
        Ok(match variant {
            0 => {
                let payload = InitializeSafetyConfigPayload::try_from_slice(rest)?;
                Self::InitializeSafetyConfig {
                    max_base_leverage: payload.max_base_leverage,
                    max_effective_leverage: payload.max_effective_leverage,
                }
            },
            1 => {
                let payload = UpdateSafetyParametersPayload::try_from_slice(rest)?;
                Self::UpdateSafetyParameters {
                    max_base_leverage: payload.max_base_leverage,
                    max_effective_leverage: payload.max_effective_leverage,
                    chain_depth_multiplier: payload.chain_depth_multiplier,
                    coverage_minimum: payload.coverage_minimum,
                    correlation_penalty: payload.correlation_penalty,
                    volatility_adjustment: payload.volatility_adjustment,
                }
            },
            2 => {
                let payload = UpdateLiquidationParametersPayload::try_from_slice(rest)?;
                Self::UpdateLiquidationParameters {
                    partial_liq_percent: payload.partial_liq_percent,
                    liq_buffer_bps: payload.liq_buffer_bps,
                    min_health_ratio: payload.min_health_ratio,
                    liquidation_fee_bps: payload.liquidation_fee_bps,
                    liquidation_cooldown: payload.liquidation_cooldown,
                }
            },
            3 => {
                let payload = InitializePositionHealthPayload::try_from_slice(rest)?;
                Self::InitializePositionHealth {
                    position_id: payload.position_id,
                    market_id: payload.market_id,
                    trader: payload.trader,
                    entry_price: payload.entry_price,
                    side: payload.side,
                    base_leverage: payload.base_leverage,
                }
            },
            4 => {
                let payload = MonitorPositionPayload::try_from_slice(rest)?;
                Self::MonitorPosition {
                    current_price: payload.current_price,
                    price_staleness_threshold: payload.price_staleness_threshold,
                }
            },
            5 => {
                let payload = AddChainStepPayload::try_from_slice(rest)?;
                Self::AddChainStep { step_type: payload.step_type }
            },
            6 => {
                let payload = ProcessPartialLiquidationPayload::try_from_slice(rest)?;
                Self::ProcessPartialLiquidation { liquidation_amount: payload.liquidation_amount }
            },
            7 => Self::InitializeLiquidationQueue,
            8 => {
                let payload = ToggleEmergencyHaltPayload::try_from_slice(rest)?;
                Self::ToggleEmergencyHalt { halt: payload.halt }
            },
            9 => {
                let payload = UpdateTierCapsPayload::try_from_slice(rest)?;
                Self::UpdateTierCaps { tier_caps: payload.tier_caps }
            },
            _ => return Err(ProgramError::InvalidInstructionData),
        })
    }
}

// Payload structs for complex instructions
#[derive(BorshSerialize, BorshDeserialize)]
struct InitializeSafetyConfigPayload {
    max_base_leverage: u64,
    max_effective_leverage: u64,
}

#[derive(BorshSerialize, BorshDeserialize)]
struct UpdateSafetyParametersPayload {
    max_base_leverage: Option<u64>,
    max_effective_leverage: Option<u64>,
    chain_depth_multiplier: Option<u64>,
    coverage_minimum: Option<u64>,
    correlation_penalty: Option<u64>,
    volatility_adjustment: Option<bool>,
}

#[derive(BorshSerialize, BorshDeserialize)]
struct UpdateLiquidationParametersPayload {
    partial_liq_percent: Option<u16>,
    liq_buffer_bps: Option<u16>,
    min_health_ratio: Option<u64>,
    liquidation_fee_bps: Option<u16>,
    liquidation_cooldown: Option<u64>,
}

#[derive(BorshSerialize, BorshDeserialize)]
struct InitializePositionHealthPayload {
    position_id: [u8; 32],
    market_id: [u8; 32],
    trader: Pubkey,
    entry_price: u64,
    side: bool,
    base_leverage: u64,
}

#[derive(BorshSerialize, BorshDeserialize)]
struct MonitorPositionPayload {
    current_price: u64,
    price_staleness_threshold: i64,
}

#[derive(BorshSerialize, BorshDeserialize)]
struct AddChainStepPayload {
    step_type: u8,
}

#[derive(BorshSerialize, BorshDeserialize)]
struct ProcessPartialLiquidationPayload {
    liquidation_amount: u64,
}

#[derive(BorshSerialize, BorshDeserialize)]
struct ToggleEmergencyHaltPayload {
    halt: bool,
}

#[derive(BorshSerialize, BorshDeserialize)]
struct UpdateTierCapsPayload {
    tier_caps: Vec<(u8, u8, u64)>,
}

// Helper functions to create instructions
pub fn initialize_safety_config(
    program_id: &Pubkey,
    authority: &Pubkey,
    config_account: &Pubkey,
    max_base_leverage: u64,
    max_effective_leverage: u64,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new(*authority, true),
        AccountMeta::new(*config_account, false),
        AccountMeta::new_readonly(solana_program::system_program::id(), false),
        AccountMeta::new_readonly(solana_program::sysvar::rent::id(), false),
    ];
    
    let data = LeverageSafetyInstruction::InitializeSafetyConfig {
        max_base_leverage,
        max_effective_leverage,
    };
    
    Instruction {
        program_id: *program_id,
        accounts,
        data: borsh::to_vec(&data).unwrap(),
    }
}