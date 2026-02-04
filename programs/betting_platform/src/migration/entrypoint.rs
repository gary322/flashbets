// Native Solana entrypoint for migration program
// NO ANCHOR

use solana_program::{
    account_info::AccountInfo,
    entrypoint,
    entrypoint::ProgramResult,
    pubkey::Pubkey,
    msg,
};

use crate::migration::instruction::process_instruction;

// Program entrypoint
entrypoint!(process);

pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    msg!("Migration program entrypoint");
    
    // Process the instruction
    process_instruction(program_id, accounts, instruction_data)
}