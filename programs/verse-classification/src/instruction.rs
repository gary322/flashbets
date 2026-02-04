use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    instruction::{AccountMeta, Instruction},
    program_error::ProgramError,
    pubkey::Pubkey,
};

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub enum ClassificationInstruction {
    /// Initialize the classification engine
    /// Accounts:
    /// 0. `[signer]` Authority
    /// 1. `[writable]` Classification engine PDA
    /// 2. `[writable]` Verse registry PDA
    /// 3. `[]` System program
    /// 4. `[]` Rent sysvar
    InitializeEngine,
    
    /// Classify a new market
    /// Accounts:
    /// 0. `[signer]` Authority
    /// 1. `[]` Classification engine PDA
    /// 2. `[writable]` Verse registry PDA
    /// 3. `[writable]` Verse account PDA (may be new)
    /// 4. `[]` System program
    /// 5. `[]` Rent sysvar
    ClassifyMarket {
        market_title: String,
        market_id: String,
    },
    
    /// Update verse hierarchy
    /// Accounts:
    /// 0. `[signer]` Authority
    /// 1. `[]` Classification engine PDA
    /// 2. `[writable]` Verse account PDA
    /// 3. `[writable]` Parent verse account PDA (optional)
    UpdateVerseHierarchy {
        verse_id: [u8; 16],
        parent_id: Option<[u8; 16]>,
    },
    
    /// Search verses
    /// Accounts:
    /// 0. `[]` Verse registry PDA
    SearchVerses {
        keywords: Vec<String>,
        category: Option<String>,
    },
}

impl ClassificationInstruction {
    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        let (&variant, rest) = input.split_first().ok_or(ProgramError::InvalidInstructionData)?;
        
        match variant {
            0 => Ok(Self::InitializeEngine),
            1 => Self::try_from_slice(rest).map_err(|_| ProgramError::InvalidInstructionData),
            2 => Self::try_from_slice(rest).map_err(|_| ProgramError::InvalidInstructionData),
            3 => Self::try_from_slice(rest).map_err(|_| ProgramError::InvalidInstructionData),
            _ => Err(ProgramError::InvalidInstructionData),
        }
    }
    
    pub fn pack(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(256);
        match self {
            Self::InitializeEngine => {
                buf.push(0);
            }
            Self::ClassifyMarket { .. } => {
                buf.push(1);
                buf.extend_from_slice(&self.try_to_vec().unwrap());
            }
            Self::UpdateVerseHierarchy { .. } => {
                buf.push(2);
                buf.extend_from_slice(&self.try_to_vec().unwrap());
            }
            Self::SearchVerses { .. } => {
                buf.push(3);
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
    registry_pda: &Pubkey,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new(*authority, true),
        AccountMeta::new(*engine_pda, false),
        AccountMeta::new(*registry_pda, false),
        AccountMeta::new_readonly(solana_program::system_program::id(), false),
        AccountMeta::new_readonly(solana_program::sysvar::rent::id(), false),
    ];
    
    let data = ClassificationInstruction::InitializeEngine.pack();
    
    Instruction {
        program_id: *program_id,
        accounts,
        data,
    }
}