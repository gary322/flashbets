use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    instruction::{AccountMeta, Instruction},
    program_error::ProgramError,
    pubkey::Pubkey,
};

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub enum CorrelationInstruction {
    /// Initialize the correlation engine
    /// Accounts:
    /// 0. `[signer]` Authority
    /// 1. `[writable]` Correlation engine PDA
    /// 2. `[]` System program
    /// 3. `[]` Rent sysvar
    InitializeEngine,
    
    /// Initialize tracking for a verse
    /// Accounts:
    /// 0. `[signer]` Authority
    /// 1. `[]` Correlation engine PDA
    /// 2. `[writable]` Verse tracking PDA
    /// 3. `[writable]` Correlation matrix PDA
    /// 4. `[writable]` Tail loss PDA
    /// 5. `[]` System program
    /// 6. `[]` Rent sysvar
    InitializeVerseTracking {
        verse_id: [u8; 16],
    },
    
    /// Update price history for a market
    /// Accounts:
    /// 0. `[signer]` Price authority (keeper)
    /// 1. `[writable]` Market price history PDA
    /// 2. `[]` System program (required when creating PDA)
    UpdatePriceHistory {
        market_id: [u8; 16],
        price: u64,
        volume: u64,
    },
    
    /// Calculate correlations for a verse
    /// Accounts:
    /// 0. `[signer]` Authority
    /// 1. `[]` Correlation engine PDA
    /// 2. `[]` Verse tracking PDA
    /// 3. `[writable]` Correlation matrix PDA
    /// 4+. `[]` Market price history PDAs (variable count)
    CalculateCorrelations {
        verse_id: [u8; 16],
    },
    
    /// Update tail loss with correlation
    /// Accounts:
    /// 0. `[signer]` Authority
    /// 1. `[]` Correlation engine PDA
    /// 2. `[]` Verse tracking PDA
    /// 3. `[]` Correlation matrix PDA
    /// 4. `[writable]` Tail loss PDA
    UpdateTailLoss {
        verse_id: [u8; 16],
        outcome_count: u32,
    },
    
    /// Update market weights
    /// Accounts:
    /// 0. `[signer]` Authority
    /// 1. `[]` Correlation engine PDA
    /// 2. `[writable]` Verse tracking PDA
    UpdateMarketWeights {
        verse_id: [u8; 16],
        market_weights: Vec<(u16, u64)>, // (market_index, weight)
    },
}

impl CorrelationInstruction {
    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        let (&variant, rest) = input.split_first().ok_or(ProgramError::InvalidInstructionData)?;
        
        match variant {
            0 => Ok(Self::InitializeEngine),
            1 => Self::try_from_slice(rest).map_err(|_| ProgramError::InvalidInstructionData),
            2 => Self::try_from_slice(rest).map_err(|_| ProgramError::InvalidInstructionData),
            3 => Self::try_from_slice(rest).map_err(|_| ProgramError::InvalidInstructionData),
            4 => Self::try_from_slice(rest).map_err(|_| ProgramError::InvalidInstructionData),
            5 => Self::try_from_slice(rest).map_err(|_| ProgramError::InvalidInstructionData),
            _ => Err(ProgramError::InvalidInstructionData),
        }
    }
    
    pub fn pack(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(256);
        match self {
            Self::InitializeEngine => {
                buf.push(0);
            }
            Self::InitializeVerseTracking { .. } => {
                buf.push(1);
                buf.extend_from_slice(&self.try_to_vec().unwrap());
            }
            Self::UpdatePriceHistory { .. } => {
                buf.push(2);
                buf.extend_from_slice(&self.try_to_vec().unwrap());
            }
            Self::CalculateCorrelations { .. } => {
                buf.push(3);
                buf.extend_from_slice(&self.try_to_vec().unwrap());
            }
            Self::UpdateTailLoss { .. } => {
                buf.push(4);
                buf.extend_from_slice(&self.try_to_vec().unwrap());
            }
            Self::UpdateMarketWeights { .. } => {
                buf.push(5);
                buf.extend_from_slice(&self.try_to_vec().unwrap());
            }
        }
        buf
    }
}

// Helper functions to create instructions
pub fn initialize_engine(
    program_id: &Pubkey,
    authority: &Pubkey,
    engine_pda: &Pubkey,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new(*authority, true),
        AccountMeta::new(*engine_pda, false),
        AccountMeta::new_readonly(solana_program::system_program::id(), false),
        AccountMeta::new_readonly(solana_program::sysvar::rent::id(), false),
    ];
    
    let data = CorrelationInstruction::InitializeEngine.pack();
    
    Instruction {
        program_id: *program_id,
        accounts,
        data,
    }
}

pub fn initialize_verse_tracking(
    program_id: &Pubkey,
    authority: &Pubkey,
    engine_pda: &Pubkey,
    verse_tracking_pda: &Pubkey,
    correlation_matrix_pda: &Pubkey,
    tail_loss_pda: &Pubkey,
    verse_id: [u8; 16],
) -> Instruction {
    let accounts = vec![
        AccountMeta::new(*authority, true),
        AccountMeta::new_readonly(*engine_pda, false),
        AccountMeta::new(*verse_tracking_pda, false),
        AccountMeta::new(*correlation_matrix_pda, false),
        AccountMeta::new(*tail_loss_pda, false),
        AccountMeta::new_readonly(solana_program::system_program::id(), false),
        AccountMeta::new_readonly(solana_program::sysvar::rent::id(), false),
    ];
    
    let data = CorrelationInstruction::InitializeVerseTracking { verse_id }.pack();
    
    Instruction {
        program_id: *program_id,
        accounts,
        data,
    }
}

pub fn update_price_history(
    program_id: &Pubkey,
    keeper: &Pubkey,
    price_history_pda: &Pubkey,
    market_id: [u8; 16],
    price: u64,
    volume: u64,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new(*keeper, true),
        AccountMeta::new(*price_history_pda, false),
        AccountMeta::new_readonly(solana_program::system_program::id(), false),
    ];

    let data = CorrelationInstruction::UpdatePriceHistory {
        market_id,
        price,
        volume,
    }
    .pack();

    Instruction {
        program_id: *program_id,
        accounts,
        data,
    }
}

pub fn calculate_correlations(
    program_id: &Pubkey,
    authority: &Pubkey,
    engine_pda: &Pubkey,
    verse_tracking_pda: &Pubkey,
    correlation_matrix_pda: &Pubkey,
    verse_id: [u8; 16],
    market_price_history_pdas: &[Pubkey],
) -> Instruction {
    let mut accounts = vec![
        AccountMeta::new(*authority, true),
        AccountMeta::new_readonly(*engine_pda, false),
        AccountMeta::new_readonly(*verse_tracking_pda, false),
        AccountMeta::new(*correlation_matrix_pda, false),
    ];

    accounts.extend(
        market_price_history_pdas
            .iter()
            .map(|pda| AccountMeta::new_readonly(*pda, false)),
    );

    let data = CorrelationInstruction::CalculateCorrelations { verse_id }.pack();

    Instruction {
        program_id: *program_id,
        accounts,
        data,
    }
}

pub fn update_tail_loss(
    program_id: &Pubkey,
    authority: &Pubkey,
    engine_pda: &Pubkey,
    verse_tracking_pda: &Pubkey,
    correlation_matrix_pda: &Pubkey,
    tail_loss_pda: &Pubkey,
    verse_id: [u8; 16],
    outcome_count: u32,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new(*authority, true),
        AccountMeta::new_readonly(*engine_pda, false),
        AccountMeta::new_readonly(*verse_tracking_pda, false),
        AccountMeta::new_readonly(*correlation_matrix_pda, false),
        AccountMeta::new(*tail_loss_pda, false),
    ];

    let data = CorrelationInstruction::UpdateTailLoss {
        verse_id,
        outcome_count,
    }
    .pack();

    Instruction {
        program_id: *program_id,
        accounts,
        data,
    }
}
