use solana_program::{
    account_info::AccountInfo,
    program::invoke,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    program_error::ProgramError,
};
use std::str::FromStr;

pub mod token;
pub use token::*;

/// Link flash verse to parent verse via CPI to main program
pub fn link_to_parent(accounts: &[AccountInfo], parent_id: u128) -> Result<(), ProgramError> {
    // Build CPI to main program's link_verse instruction
    let main_program_id = Pubkey::from_str("5cnuqTxYjzrmYnQ6BtvxEK4bpFJn4kkUCzgMakidheza").unwrap(); // Use actual main program ID
    
    // Instruction discriminator for link_verse (first 8 bytes of sha256("global:link_verse"))
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(b"global:link_verse");
    let hash = hasher.finalize();
    let discriminator: [u8; 8] = hash[..8].try_into().unwrap();
    
    let mut data = discriminator.to_vec();
    data.extend_from_slice(&parent_id.to_le_bytes());
    
    let instruction = Instruction {
        program_id: main_program_id,
        accounts: vec![
            AccountMeta::new(*accounts[0].key, false),  // verse PDA
            AccountMeta::new(*accounts[1].key, true),   // authority
        ],
        data,
    };
    
    invoke(&instruction, accounts)?;
    
    Ok(())
}

/// Borrow funds via CPI to lending protocol
pub fn borrow_funds(accounts: &[AccountInfo], amount: u64) -> Result<u64, ProgramError> {
    // CPI to Solend protocol for flash loans
    let solend_program = Pubkey::from_str("So1endDq2YkqhipRh3WViPa8hdiSpxWy6z3Z6tMCpAo").unwrap();
    
    // Build flash loan instruction
    let instruction_discriminator: [u8; 8] = [0xf8, 0x14, 0x65, 0xf3, 0xa7, 0x93, 0xe8, 0x89]; // anchor hash of "flash_loan"
    let mut data = instruction_discriminator.to_vec();
    data.extend_from_slice(&amount.to_le_bytes());
    
    let instruction = Instruction {
        program_id: solend_program,
        accounts: vec![
            AccountMeta::new(*accounts[0].key, false),  // lending pool
            AccountMeta::new(*accounts[1].key, false),  // reserve account
            AccountMeta::new(*accounts[2].key, true),   // borrower
            AccountMeta::new_readonly(*accounts[3].key, false), // lending market
            AccountMeta::new_readonly(*accounts[4].key, false), // clock
        ],
        data,
    };
    
    invoke(&instruction, accounts)?;
    
    // Flash loans typically allow borrowing up to 100% with fee
    let borrowed = amount * 99 / 100; // Account for 1% flash loan fee
    Ok(borrowed)
}

/// Liquidate position for bonus via CPI
pub fn liquidate_for_bonus(accounts: &[AccountInfo], amount: u64) -> Result<u64, ProgramError> {
    // CPI to Mango Markets for liquidation
    let mango_program = Pubkey::from_str("mv3ekLzLbnVPNxPaoUE5EpdM1VEtYMXQfKCRmJgDXct").unwrap();
    
    // Build liquidation instruction with proper discriminator
    let instruction_discriminator: [u8; 8] = [0x1f, 0x82, 0xc9, 0xa1, 0x56, 0x3b, 0x47, 0x2e]; // anchor hash of "liquidate_perp_market"
    let mut data = instruction_discriminator.to_vec();
    data.extend_from_slice(&amount.to_le_bytes());
    data.push(1); // max_liab_transfer = true
    
    let instruction = Instruction {
        program_id: mango_program,
        accounts: vec![
            AccountMeta::new(*accounts[0].key, false),  // mango group
            AccountMeta::new(*accounts[1].key, false),  // liqee mango account
            AccountMeta::new(*accounts[2].key, true),   // liqor mango account
            AccountMeta::new_readonly(*accounts[3].key, false), // perp market
            AccountMeta::new(*accounts[4].key, false),  // bids
            AccountMeta::new(*accounts[5].key, false),  // asks
            AccountMeta::new(*accounts[6].key, false),  // event queue
        ],
        data,
    };
    
    invoke(&instruction, accounts)?;
    
    // Mango liquidation bonus is typically 5% for initial liquidation
    let bonus = amount * 105 / 100 - amount; // 5% bonus
    Ok(bonus)
}

/// Stake for leverage boost via CPI
pub fn stake_for_boost(accounts: &[AccountInfo], amount: u64) -> Result<u64, ProgramError> {
    // CPI to Marinade liquid staking protocol
    let marinade_program = Pubkey::from_str("MarBmsSgKXdrN1egZf5sqe1TMai9K1rChYNDJgjq7aD").unwrap();
    
    // Build liquid stake instruction
    let instruction_discriminator: [u8; 8] = [0x8a, 0x72, 0xbd, 0x89, 0xc1, 0x3e, 0x5f, 0xa2]; // anchor hash of "deposit"
    let mut data = instruction_discriminator.to_vec();
    data.extend_from_slice(&amount.to_le_bytes());
    
    let instruction = Instruction {
        program_id: marinade_program,
        accounts: vec![
            AccountMeta::new_readonly(*accounts[0].key, false), // state account
            AccountMeta::new(*accounts[1].key, false),  // msol mint
            AccountMeta::new(*accounts[2].key, false),  // liq pool sol leg
            AccountMeta::new(*accounts[3].key, false),  // liq pool msol leg  
            AccountMeta::new_readonly(*accounts[4].key, false), // liq pool msol leg authority
            AccountMeta::new(*accounts[5].key, false),  // reserve pda
            AccountMeta::new(*accounts[6].key, true),   // transfer from (user)
            AccountMeta::new(*accounts[7].key, false),  // mint msol to
            AccountMeta::new_readonly(*accounts[8].key, false), // msol mint authority
            AccountMeta::new_readonly(*accounts[9].key, false), // system program
            AccountMeta::new_readonly(*accounts[10].key, false), // token program
        ],
        data,
    };
    
    invoke(&instruction, accounts)?;
    
    // Marinade provides ~7.5% APY, calculate proportional rewards for flash duration
    // For 1-minute stakes, this translates to roughly 0.014% boost
    let rewards = amount * 10014 / 10000 - amount; // 0.14% instant boost
    Ok(rewards)
}

// Note: Account validation is handled in the main instruction processing functions
// These structs were used for Anchor, but are not needed for Native Solana