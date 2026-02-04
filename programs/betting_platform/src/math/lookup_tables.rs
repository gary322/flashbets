// Precomputed lookup tables for fast approximations
// Native Solana implementation - NO ANCHOR

use solana_program::{
    program_error::ProgramError,
    account_info::AccountInfo,
    pubkey::Pubkey,
    msg,
};
use crate::math::fixed_point::{U64F64, MathError, ONE};
use crate::math::trigonometry::TrigFunctions;
use crate::math::functions::MathFunctions;

// Size of lookup tables (256 points as specified in CLAUDE.md)
pub const TABLE_SIZE: usize = 256;
pub const TABLE_MIN: i64 = -4;  // -4 standard deviations
pub const TABLE_MAX: i64 = 4;   // +4 standard deviations

/// Account layout for storing precomputed tables on-chain
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PrecomputedTables {
    pub discriminator: [u8; 8],
    pub is_initialized: bool,
    pub normal_cdf_table: [u128; TABLE_SIZE],
    pub normal_pdf_table: [u128; TABLE_SIZE],
    pub exp_table: [u128; TABLE_SIZE],
    pub ln_table: [u128; TABLE_SIZE],
    pub sqrt_table: [u128; TABLE_SIZE],
}

impl PrecomputedTables {
    pub const DISCRIMINATOR: [u8; 8] = [0x71, 0x14, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD];
    pub const LEN: usize = 8 + 1 + (5 * TABLE_SIZE * 16); // discriminator + init + 5 tables
    
    /// Initialize all lookup tables
    pub fn initialize(&mut self) -> Result<(), MathError> {
        self.discriminator = Self::DISCRIMINATOR;
        self.is_initialized = true;
        
        // Precompute normal distribution tables
        for i in 0..TABLE_SIZE {
            let x = Self::index_to_value(i)?;
            
            // Normal CDF and PDF
            self.normal_cdf_table[i] = TrigFunctions::normal_cdf(x)?.0;
            self.normal_pdf_table[i] = TrigFunctions::normal_pdf(x)?.0;
            
            // Exponential (0 to 8)
            let exp_x = U64F64::from_num((i * 8) as u64)
                .checked_div(U64F64::from_num(TABLE_SIZE as u64))
                .ok_or(MathError::DivisionByZero)?;
            self.exp_table[i] = MathFunctions::exp(exp_x)?.0;
            
            // Natural log (0.01 to 10)
            if i > 0 {
                let ln_x = U64F64::from_num(i as u64)
                    .checked_mul(U64F64::from_num(10))
                    .ok_or(MathError::Overflow)?
                    .checked_div(U64F64::from_num(TABLE_SIZE as u64))
                    .ok_or(MathError::DivisionByZero)?;
                self.ln_table[i] = MathFunctions::ln(ln_x)?.0;
            } else {
                self.ln_table[i] = 0; // ln(0) undefined, store 0
            }
            
            // Square root (0 to 256)
            let sqrt_x = U64F64::from_num(i as u64);
            self.sqrt_table[i] = MathFunctions::sqrt(sqrt_x)?.0;
        }
        
        Ok(())
    }
    
    /// Convert table index to value in [-4, 4] range
    fn index_to_value(index: usize) -> Result<U64F64, MathError> {
        let range = (TABLE_MAX - TABLE_MIN) as u64;
        let step = U64F64::from_num(range)
            .checked_div(U64F64::from_num((TABLE_SIZE - 1) as u64))
            .ok_or(MathError::DivisionByZero)?;
        
        let offset = U64F64::from_num(TABLE_MIN.abs() as u64);
        let index_value = U64F64::from_num(index as u64)
            .checked_mul(step)
            .ok_or(MathError::Overflow)?;
        
        // x = -4 + index * step
        index_value.checked_sub(offset).ok_or(MathError::Underflow)
    }
    
    /// Convert value to table index and fractional part for interpolation
    fn value_to_index_and_frac(x: U64F64) -> Result<(usize, U64F64), MathError> {
        // Clamp to table range
        let min = U64F64::from_num(TABLE_MIN.abs() as u64);
        let max = U64F64::from_num(TABLE_MAX as u64);
        let range = U64F64::from_num((TABLE_MAX - TABLE_MIN) as u64);
        
        // Shift x to [0, range]
        let shifted = x.checked_add(min).ok_or(MathError::Overflow)?;
        let clamped = shifted.min(range).max(U64F64::ZERO);
        
        // Scale to table index
        let scaled = clamped
            .checked_mul(U64F64::from_num((TABLE_SIZE - 1) as u64))
            .ok_or(MathError::Overflow)?
            .checked_div(range)
            .ok_or(MathError::DivisionByZero)?;
        
        let index = (scaled.0 >> 64) as usize;
        let frac = U64F64::from_raw(scaled.0 & ((1u128 << 64) - 1));
        
        Ok((index.min(TABLE_SIZE - 2), frac))
    }
    
    /// Linear interpolation between two table values
    fn linear_interpolate(y0: U64F64, y1: U64F64, frac: U64F64) -> Result<U64F64, MathError> {
        // y = y0 + frac * (y1 - y0)
        let diff = if y1 >= y0 {
            y1.checked_sub(y0).ok_or(MathError::Underflow)?
        } else {
            // Handle negative difference
            let abs_diff = y0.checked_sub(y1).ok_or(MathError::Underflow)?;
            let frac_diff = frac.checked_mul(abs_diff).ok_or(MathError::Overflow)?;
            return y0.checked_sub(frac_diff).ok_or(MathError::Underflow);
        };
        
        let frac_diff = frac.checked_mul(diff).ok_or(MathError::Overflow)?;
        y0.checked_add(frac_diff).ok_or(MathError::Overflow)
    }
    
    /// Lookup normal CDF with interpolation
    pub fn lookup_normal_cdf(&self, x: U64F64) -> Result<U64F64, MathError> {
        let (index, frac) = Self::value_to_index_and_frac(x)?;
        let y0 = U64F64::from_raw(self.normal_cdf_table[index]);
        let y1 = U64F64::from_raw(self.normal_cdf_table[index + 1]);
        Self::linear_interpolate(y0, y1, frac)
    }
    
    /// Lookup normal PDF with interpolation
    pub fn lookup_normal_pdf(&self, x: U64F64) -> Result<U64F64, MathError> {
        let (index, frac) = Self::value_to_index_and_frac(x)?;
        let y0 = U64F64::from_raw(self.normal_pdf_table[index]);
        let y1 = U64F64::from_raw(self.normal_pdf_table[index + 1]);
        Self::linear_interpolate(y0, y1, frac)
    }
    
    /// Lookup exponential with interpolation
    pub fn lookup_exp(&self, x: U64F64) -> Result<U64F64, MathError> {
        // Map x from [0, 8] to table index
        if x > U64F64::from_num(8) {
            // For x > 8, use the fact that exp(x) = exp(8) * exp(x-8)
            let exp_8 = U64F64::from_raw(self.exp_table[TABLE_SIZE - 1]);
            let remainder = x.checked_sub(U64F64::from_num(8)).ok_or(MathError::Underflow)?;
            let exp_remainder = MathFunctions::exp(remainder)?;
            return exp_8.checked_mul(exp_remainder).ok_or(MathError::Overflow);
        }
        
        let scaled = x.checked_mul(U64F64::from_num((TABLE_SIZE - 1) as u64))
            .ok_or(MathError::Overflow)?
            .checked_div(U64F64::from_num(8))
            .ok_or(MathError::DivisionByZero)?;
        
        let index = (scaled.0 >> 64) as usize;
        let frac = U64F64::from_raw(scaled.0 & ((1u128 << 64) - 1));
        
        if index >= TABLE_SIZE - 1 {
            return Ok(U64F64::from_raw(self.exp_table[TABLE_SIZE - 1]));
        }
        
        let y0 = U64F64::from_raw(self.exp_table[index]);
        let y1 = U64F64::from_raw(self.exp_table[index + 1]);
        Self::linear_interpolate(y0, y1, frac)
    }
    
    /// Lookup natural logarithm with interpolation
    pub fn lookup_ln(&self, x: U64F64) -> Result<U64F64, MathError> {
        if x <= U64F64::ZERO {
            return Err(MathError::InvalidInput);
        }
        
        // Map x from [0.01, 10] to table index
        if x < U64F64::from_raw(ONE / 100) {  // x < 0.01
            // Use ln(x) = ln(0.01) + ln(x/0.01)
            let ln_001 = U64F64::from_raw(self.ln_table[1]); // Approximation
            let ratio = x.checked_mul(U64F64::from_num(100))
                .ok_or(MathError::Overflow)?;
            let ln_ratio = MathFunctions::ln(ratio)?;
            return ln_001.checked_add(ln_ratio).ok_or(MathError::Overflow);
        }
        
        if x > U64F64::from_num(10) {
            // Use ln(x) = ln(10) + ln(x/10)
            let ln_10 = U64F64::from_raw(self.ln_table[TABLE_SIZE - 1]);
            let ratio = x.checked_div(U64F64::from_num(10))
                .ok_or(MathError::DivisionByZero)?;
            let ln_ratio = MathFunctions::ln(ratio)?;
            return ln_10.checked_add(ln_ratio).ok_or(MathError::Overflow);
        }
        
        // Scale to table index
        let scaled = x.checked_mul(U64F64::from_num((TABLE_SIZE - 1) as u64))
            .ok_or(MathError::Overflow)?
            .checked_div(U64F64::from_num(10))
            .ok_or(MathError::DivisionByZero)?;
        
        let index = ((scaled.0 >> 64) as usize).max(1);
        let frac = U64F64::from_raw(scaled.0 & ((1u128 << 64) - 1));
        
        if index >= TABLE_SIZE - 1 {
            return Ok(U64F64::from_raw(self.ln_table[TABLE_SIZE - 1]));
        }
        
        let y0 = U64F64::from_raw(self.ln_table[index]);
        let y1 = U64F64::from_raw(self.ln_table[index + 1]);
        Self::linear_interpolate(y0, y1, frac)
    }
    
    /// Lookup square root with interpolation
    pub fn lookup_sqrt(&self, x: U64F64) -> Result<U64F64, MathError> {
        if x > U64F64::from_num(256) {
            // For x > 256, use sqrt(x) = sqrt(256) * sqrt(x/256)
            let sqrt_256 = U64F64::from_raw(self.sqrt_table[TABLE_SIZE - 1]);
            let ratio = x.checked_div(U64F64::from_num(256))
                .ok_or(MathError::DivisionByZero)?;
            let sqrt_ratio = MathFunctions::sqrt(ratio)?;
            return sqrt_256.checked_mul(sqrt_ratio).ok_or(MathError::Overflow);
        }
        
        let index = (x.0 >> 64) as usize;
        if index >= TABLE_SIZE - 1 {
            return Ok(U64F64::from_raw(self.sqrt_table[TABLE_SIZE - 1]));
        }
        
        let frac = U64F64::from_raw(x.0 & ((1u128 << 64) - 1));
        let y0 = U64F64::from_raw(self.sqrt_table[index]);
        let y1 = U64F64::from_raw(self.sqrt_table[index + 1]);
        Self::linear_interpolate(y0, y1, frac)
    }
}

/// Load precomputed tables from a PDA
pub fn load_tables_from_pda<'a>(
    tables_account: &'a AccountInfo,
    expected_program_id: &Pubkey,
) -> Result<&'a PrecomputedTables, ProgramError> {
    // Verify account ownership
    if tables_account.owner != expected_program_id {
        msg!("Invalid tables account owner");
        return Err(ProgramError::InvalidAccountOwner);
    }
    
    // Verify account size
    if tables_account.data_len() < PrecomputedTables::LEN {
        msg!("Tables account too small");
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Cast account data to PrecomputedTables
    let tables = unsafe {
        &*(tables_account.data.borrow().as_ptr() as *const PrecomputedTables)
    };
    
    // Verify discriminator
    if tables.discriminator != PrecomputedTables::DISCRIMINATOR {
        msg!("Invalid tables discriminator");
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Verify initialization
    if !tables.is_initialized {
        msg!("Tables not initialized");
        return Err(ProgramError::UninitializedAccount);
    }
    
    Ok(tables)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_table_initialization() {
        let mut tables = PrecomputedTables {
            discriminator: [0; 8],
            is_initialized: false,
            normal_cdf_table: [0; TABLE_SIZE],
            normal_pdf_table: [0; TABLE_SIZE],
            exp_table: [0; TABLE_SIZE],
            ln_table: [0; TABLE_SIZE],
            sqrt_table: [0; TABLE_SIZE],
        };
        
        assert!(tables.initialize().is_ok());
        assert!(tables.is_initialized);
        assert_eq!(tables.discriminator, PrecomputedTables::DISCRIMINATOR);
        
        // Verify some known values
        // CDF(0) ≈ 0.5
        let mid_index = TABLE_SIZE / 2;
        let cdf_0 = U64F64::from_raw(tables.normal_cdf_table[mid_index]);
        let half = U64F64::from_raw(ONE >> 1);
        assert!(cdf_0.abs_diff(half).0 < (ONE >> 10));
        
        // sqrt(4) = 2
        let sqrt_4 = U64F64::from_raw(tables.sqrt_table[4]);
        assert_eq!(sqrt_4, U64F64::from_num(2));
    }
    
    #[test]
    fn test_interpolation() {
        let mut tables = PrecomputedTables {
            discriminator: [0; 8],
            is_initialized: false,
            normal_cdf_table: [0; TABLE_SIZE],
            normal_pdf_table: [0; TABLE_SIZE],
            exp_table: [0; TABLE_SIZE],
            ln_table: [0; TABLE_SIZE],
            sqrt_table: [0; TABLE_SIZE],
        };
        
        tables.initialize().unwrap();
        
        // Test CDF lookup with interpolation
        let x = U64F64::from_raw(HALF); // 0.5
        let cdf_lookup = tables.lookup_normal_cdf(x).unwrap();
        let cdf_computed = TrigFunctions::normal_cdf(x).unwrap();
        
        // Should be close (within 0.1%)
        let diff = cdf_lookup.abs_diff(cdf_computed);
        let tolerance = U64F64::from_num(1).checked_div(U64F64::from_num(1000)).unwrap();
        assert!(diff < tolerance);
    }
}