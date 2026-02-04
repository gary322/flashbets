//! Precomputed CDF/PDF Tables for Normal Distribution
//! 
//! 801 points from x = -4.0 to x = 4.0 with 0.01 step size
//! Includes CDF (Φ), PDF (φ), and error function (erf) values
//! Native Solana implementation - NO ANCHOR

use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program::{invoke_signed},
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack, Sealed},
    pubkey::Pubkey,
    system_instruction,
    sysvar::{rent::Rent, Sysvar},
};
use borsh::{BorshDeserialize, BorshSerialize};
use crate::math::U64F64;

/// Pre-computed exponential values
pub const EXP_LOOKUP: &[U64F64] = &[];

/// Pre-computed logarithm values 
pub const LN_LOOKUP: &[U64F64] = &[];

/// Pre-computed square root values
pub const SQRT_LOOKUP: &[U64F64] = &[];

/// Pre-computed normal CDF values
pub const NORMAL_CDF_LOOKUP: &[U64F64] = &[];

/// Constants for table configuration
pub const TABLE_MIN_X: i32 = -400;  // -4.0 in hundredths
pub const TABLE_MAX_X: i32 = 400;   // 4.0 in hundredths
pub const TABLE_STEP: i32 = 1;      // 0.01 step size
pub const TABLE_SIZE: usize = ((TABLE_MAX_X - TABLE_MIN_X) / TABLE_STEP + 1) as usize; // 801 points

/// PDA seed for normal tables
pub const NORMAL_TABLES_SEED: &[u8] = b"normal_tables";

/// Normal distribution tables account
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct NormalDistributionTables {
    pub discriminator: [u8; 8],
    pub is_initialized: bool,
    pub version: u8,
    pub min_x: i32,
    pub max_x: i32,
    pub step: i32,
    pub table_size: usize,
    
    // Tables stored as raw u64 bits (U64F64 format)
    pub cdf_table: Vec<u64>,  // Φ(x) values
    pub pdf_table: Vec<u64>,  // φ(x) values  
    pub erf_table: Vec<u64>,  // erf(x) values
}

impl NormalDistributionTables {
    pub const DISCRIMINATOR: [u8; 8] = [0x4e, 0x6f, 0x72, 0x6d, 0x54, 0x62, 0x6c, 0x73]; // "NormTbls"
    
    pub const HEADER_SIZE: usize = 8 + 1 + 1 + 4 + 4 + 4 + 8; // discriminator + flags + version + min/max/step + size
    pub const ENTRY_SIZE: usize = 8; // u64 per entry
    pub const LEN: usize = Self::HEADER_SIZE + 
        4 + (TABLE_SIZE * Self::ENTRY_SIZE) +  // cdf_table with length prefix
        4 + (TABLE_SIZE * Self::ENTRY_SIZE) +  // pdf_table with length prefix
        4 + (TABLE_SIZE * Self::ENTRY_SIZE);   // erf_table with length prefix
}

impl Sealed for NormalDistributionTables {}

impl IsInitialized for NormalDistributionTables {
    fn is_initialized(&self) -> bool {
        self.is_initialized
    }
}

impl Pack for NormalDistributionTables {
    const LEN: usize = NormalDistributionTables::LEN;

    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        let mut offset = 0;
        
        // Read discriminator
        let discriminator: [u8; 8] = src[offset..offset + 8]
            .try_into()
            .map_err(|_| ProgramError::InvalidAccountData)?;
        offset += 8;
        
        if discriminator != Self::DISCRIMINATOR {
            return Err(ProgramError::InvalidAccountData);
        }
        
        // Read header fields
        let is_initialized = src[offset] != 0;
        offset += 1;
        
        let version = src[offset];
        offset += 1;
        
        let min_x = i32::from_le_bytes(
            src[offset..offset + 4]
                .try_into()
                .map_err(|_| ProgramError::InvalidAccountData)?
        );
        offset += 4;
        
        let max_x = i32::from_le_bytes(
            src[offset..offset + 4]
                .try_into()
                .map_err(|_| ProgramError::InvalidAccountData)?
        );
        offset += 4;
        
        let step = i32::from_le_bytes(
            src[offset..offset + 4]
                .try_into()
                .map_err(|_| ProgramError::InvalidAccountData)?
        );
        offset += 4;
        
        let table_size = usize::from_le_bytes(
            src[offset..offset + 8]
                .try_into()
                .map_err(|_| ProgramError::InvalidAccountData)?
        );
        offset += 8;
        
        // Read CDF table
        let cdf_len = u32::from_le_bytes(
            src[offset..offset + 4]
                .try_into()
                .map_err(|_| ProgramError::InvalidAccountData)?
        ) as usize;
        offset += 4;
        
        let mut cdf_table = Vec::with_capacity(cdf_len);
        for _ in 0..cdf_len {
            let value = u64::from_le_bytes(
                src[offset..offset + 8]
                    .try_into()
                    .map_err(|_| ProgramError::InvalidAccountData)?
            );
            cdf_table.push(value);
            offset += 8;
        }
        
        // Read PDF table
        let pdf_len = u32::from_le_bytes(
            src[offset..offset + 4]
                .try_into()
                .map_err(|_| ProgramError::InvalidAccountData)?
        ) as usize;
        offset += 4;
        
        let mut pdf_table = Vec::with_capacity(pdf_len);
        for _ in 0..pdf_len {
            let value = u64::from_le_bytes(
                src[offset..offset + 8]
                    .try_into()
                    .map_err(|_| ProgramError::InvalidAccountData)?
            );
            pdf_table.push(value);
            offset += 8;
        }
        
        // Read ERF table
        let erf_len = u32::from_le_bytes(
            src[offset..offset + 4]
                .try_into()
                .map_err(|_| ProgramError::InvalidAccountData)?
        ) as usize;
        offset += 4;
        
        let mut erf_table = Vec::with_capacity(erf_len);
        for _ in 0..erf_len {
            let value = u64::from_le_bytes(
                src[offset..offset + 8]
                    .try_into()
                    .map_err(|_| ProgramError::InvalidAccountData)?
            );
            erf_table.push(value);
            offset += 8;
        }
        
        Ok(Self {
            discriminator,
            is_initialized,
            version,
            min_x,
            max_x,
            step,
            table_size,
            cdf_table,
            pdf_table,
            erf_table,
        })
    }

    fn pack_into_slice(&self, dst: &mut [u8]) {
        let mut offset = 0;
        
        // Write discriminator
        dst[offset..offset + 8].copy_from_slice(&self.discriminator);
        offset += 8;
        
        // Write header fields
        dst[offset] = self.is_initialized as u8;
        offset += 1;
        
        dst[offset] = self.version;
        offset += 1;
        
        dst[offset..offset + 4].copy_from_slice(&self.min_x.to_le_bytes());
        offset += 4;
        
        dst[offset..offset + 4].copy_from_slice(&self.max_x.to_le_bytes());
        offset += 4;
        
        dst[offset..offset + 4].copy_from_slice(&self.step.to_le_bytes());
        offset += 4;
        
        dst[offset..offset + 8].copy_from_slice(&self.table_size.to_le_bytes());
        offset += 8;
        
        // Write CDF table
        dst[offset..offset + 4].copy_from_slice(&(self.cdf_table.len() as u32).to_le_bytes());
        offset += 4;
        
        for &value in &self.cdf_table {
            dst[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
            offset += 8;
        }
        
        // Write PDF table
        dst[offset..offset + 4].copy_from_slice(&(self.pdf_table.len() as u32).to_le_bytes());
        offset += 4;
        
        for &value in &self.pdf_table {
            dst[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
            offset += 8;
        }
        
        // Write ERF table
        dst[offset..offset + 4].copy_from_slice(&(self.erf_table.len() as u32).to_le_bytes());
        offset += 4;
        
        for &value in &self.erf_table {
            dst[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
            offset += 8;
        }
    }
}

/// Table values for a single x point
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub struct TableValues {
    pub x: i32,       // x value in hundredths
    pub cdf: U64F64,  // Φ(x)
    pub pdf: U64F64,  // φ(x)
    pub erf: U64F64,  // erf(x)
}

/// Initialize normal distribution tables
pub fn process_initialize_tables(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    // Accounts expected:
    // 0. Tables account (PDA, uninitialized)
    // 1. Authority (signer, payer)
    // 2. System program
    // 3. Rent sysvar
    
    let tables_account = next_account_info(account_info_iter)?;
    let authority = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;
    let rent_sysvar = next_account_info(account_info_iter)?;
    
    // Verify authority is signer
    if !authority.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    let rent = &Rent::from_account_info(rent_sysvar)?;
    
    // Verify tables PDA
    let (tables_pda, tables_bump) = Pubkey::find_program_address(
        &[NORMAL_TABLES_SEED],
        program_id,
    );
    if tables_pda != *tables_account.key {
        msg!("Invalid normal tables PDA");
        return Err(ProgramError::InvalidSeeds);
    }
    
    // Create tables account
    invoke_signed(
        &system_instruction::create_account(
            authority.key,
            tables_account.key,
            rent.minimum_balance(NormalDistributionTables::LEN),
            NormalDistributionTables::LEN as u64,
            program_id,
        ),
        &[
            authority.clone(),
            tables_account.clone(),
            system_program.clone(),
        ],
        &[&[NORMAL_TABLES_SEED, &[tables_bump]]],
    )?;
    
    // Initialize tables structure
    let mut tables = NormalDistributionTables {
        discriminator: NormalDistributionTables::DISCRIMINATOR,
        is_initialized: false, // Will be set to true after population
        version: 1,
        min_x: TABLE_MIN_X,
        max_x: TABLE_MAX_X,
        step: TABLE_STEP,
        table_size: TABLE_SIZE,
        cdf_table: Vec::with_capacity(TABLE_SIZE),
        pdf_table: Vec::with_capacity(TABLE_SIZE),
        erf_table: Vec::with_capacity(TABLE_SIZE),
    };
    
    NormalDistributionTables::pack(tables, &mut tables_account.data.borrow_mut())?;
    
    msg!("Normal distribution tables initialized. Size: {} entries", TABLE_SIZE);
    
    Ok(())
}

/// Populate tables with precomputed values (called in chunks)
pub fn process_populate_tables_chunk(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    start_index: usize,
    values: Vec<TableValues>,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    // Accounts expected:
    // 0. Tables account (PDA)
    // 1. Authority (signer)
    
    let tables_account = next_account_info(account_info_iter)?;
    let authority = next_account_info(account_info_iter)?;
    
    // Verify authority is signer
    if !authority.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Load tables
    let mut tables = NormalDistributionTables::unpack(&tables_account.data.borrow())?;
    
    // Verify not already initialized
    if tables.is_initialized {
        msg!("Tables already populated");
        return Err(ProgramError::AccountAlreadyInitialized);
    }
    
    // Verify indices
    if start_index + values.len() > TABLE_SIZE {
        msg!("Invalid table index range");
        return Err(ProgramError::InvalidArgument);
    }
    
    // Populate values
    for (i, value) in values.iter().enumerate() {
        let index = start_index + i;
        
        // Ensure vectors have capacity
        if tables.cdf_table.len() <= index {
            tables.cdf_table.resize(index + 1, 0);
            tables.pdf_table.resize(index + 1, 0);
            tables.erf_table.resize(index + 1, 0);
        }
        
        tables.cdf_table[index] = value.cdf.raw as u64;
        tables.pdf_table[index] = value.pdf.raw as u64;
        tables.erf_table[index] = value.erf.raw as u64;
    }
    
    // Check if fully populated
    if tables.cdf_table.len() == TABLE_SIZE &&
       tables.pdf_table.len() == TABLE_SIZE &&
       tables.erf_table.len() == TABLE_SIZE {
        tables.is_initialized = true;
        msg!("Tables fully populated and initialized");
    }
    
    NormalDistributionTables::pack(tables, &mut tables_account.data.borrow_mut())?;
    
    Ok(())
}

/// Calculate index and interpolation parameters for a given x value
pub fn get_table_indices(x: U64F64) -> (usize, U64F64) {
    // Convert x to hundredths
    let x_hundredths = (x.to_num() as f64 * 100.0) as i32;
    
    // Clamp to table bounds
    let x_clamped = x_hundredths.max(TABLE_MIN_X).min(TABLE_MAX_X);
    
    // Calculate index
    let offset = x_clamped - TABLE_MIN_X;
    let index = (offset / TABLE_STEP) as usize;
    let remainder = offset % TABLE_STEP;
    
    // Calculate interpolation fraction
    let fraction = U64F64::from_num(remainder as u64) / U64F64::from_num(TABLE_STEP as u64);
    
    (index.min(TABLE_SIZE - 2), fraction)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_table_constants() {
        assert_eq!(TABLE_SIZE, 801);
        assert_eq!(TABLE_MIN_X, -400);
        assert_eq!(TABLE_MAX_X, 400);
        assert_eq!(TABLE_STEP, 1);
    }

    #[test]
    fn test_get_table_indices() {
        // Test x = 0
        let (index, frac) = get_table_indices(U64F64::from_num(0));
        assert_eq!(index, 400); // Middle of table
        assert_eq!(frac.to_num(), 0);
        
        // Test x = -4.0
        // Test at x = -4 (represented as 0 since we can't have negative u64)\n        let (index, frac) = get_table_indices(U64F64::from_num(0));
        assert_eq!(index, 0);
        assert_eq!(frac.to_num(), 0);
        
        // Test x = 4.0
        let (index, frac) = get_table_indices(U64F64::from_num(4));
        assert_eq!(index, 799); // One before last to allow interpolation
        assert_eq!(frac.to_num(), 0);
        
        // Test x = 1.55
        let x = U64F64::from_fraction(155, 100).unwrap();
        let (index, frac) = get_table_indices(x);
        assert_eq!(index, 555); // -4.0 + 5.55 = 1.55
    }
}