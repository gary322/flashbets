use anchor_lang::prelude::*;
use fixed::types::{U64F64 as FixedU64F64, I64F64 as FixedI64F64};
use std::ops::{Add, Sub, Mul, Div};

// Wrapper for U64F64 to implement Anchor traits
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct U64F64(pub FixedU64F64);

impl U64F64 {
    pub fn from_num<T: Into<FixedU64F64>>(val: T) -> Self {
        Self(val.into())
    }
    
    pub fn to_num<T: fixed::traits::FromFixed>(self) -> T {
        T::from_fixed(self.0)
    }
    
    pub fn to_bits(self) -> u128 {
        self.0.to_bits()
    }
    
    pub fn from_bits(bits: u128) -> Self {
        Self(FixedU64F64::from_bits(bits))
    }
    
    pub fn min(self, other: Self) -> Self {
        Self(self.0.min(other.0))
    }
    
    pub fn max(self, other: Self) -> Self {
        Self(self.0.max(other.0))
    }
    
    pub fn saturating_sub(self, other: Self) -> Self {
        Self(self.0.saturating_sub(other.0))
    }
    
    pub fn zero() -> Self {
        Self::from_num(0u32)
    }
    
    pub fn one() -> Self {
        Self::from_num(1u32)
    }
}

impl AnchorSerialize for U64F64 {
    fn serialize<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        let bits = self.0.to_bits();
        bits.serialize(writer)
    }
}

impl AnchorDeserialize for U64F64 {
    fn deserialize_reader<R: std::io::Read>(reader: &mut R) -> std::io::Result<Self> {
        let bits = u128::deserialize_reader(reader)?;
        Ok(Self(FixedU64F64::from_bits(bits)))
    }
}

// Implement arithmetic operations
impl Add for U64F64 {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self(self.0 + other.0)
    }
}

impl Sub for U64F64 {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Self(self.0 - other.0)
    }
}

impl Mul for U64F64 {
    type Output = Self;
    fn mul(self, other: Self) -> Self {
        Self(self.0 * other.0)
    }
}

impl Div for U64F64 {
    type Output = Self;
    fn div(self, other: Self) -> Self {
        Self(self.0 / other.0)
    }
}

impl std::iter::Sum for U64F64 {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::from_num(0u32), |a, b| a + b)
    }
}

// PartialEq and Ord are already derived

impl std::fmt::Display for U64F64 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(feature = "idl-build")]
impl anchor_lang::IdlBuild for U64F64 {
    fn get_full_path() -> String {
        "u128".to_string()
    }

    fn create_type() -> Option<anchor_lang::idl::IdlTypeDefinition> {
        None
    }

    fn insert_types(_types: &mut std::collections::BTreeMap<String, anchor_lang::idl::IdlTypeDefinition>) {
        // U64F64 is treated as u128 in IDL
    }
}

// Wrapper for I64F64 to implement Anchor traits
#[derive(Clone, Copy, Debug, Default)]
pub struct I64F64(pub FixedI64F64);

impl I64F64 {
    pub fn from_num<T: Into<FixedI64F64>>(val: T) -> Self {
        Self(val.into())
    }
    
    pub fn to_num<T: fixed::traits::FromFixed>(self) -> T {
        T::from_fixed(self.0)
    }
}

impl AnchorSerialize for I64F64 {
    fn serialize<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        let bits = self.0.to_bits();
        bits.serialize(writer)
    }
}

impl AnchorDeserialize for I64F64 {
    fn deserialize_reader<R: std::io::Read>(reader: &mut R) -> std::io::Result<Self> {
        let bits = i128::deserialize_reader(reader)?;
        Ok(Self(FixedI64F64::from_bits(bits)))
    }
}

impl PartialEq for I64F64 {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for I64F64 {}

impl PartialOrd for I64F64 {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for I64F64 {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

#[cfg(feature = "idl-build")]
impl anchor_lang::IdlBuild for I64F64 {
    fn get_full_path() -> String {
        "i128".to_string()
    }

    fn create_type() -> Option<anchor_lang::idl::IdlTypeDefinition> {
        None
    }

    fn insert_types(_types: &mut std::collections::BTreeMap<String, anchor_lang::idl::IdlTypeDefinition>) {
        // I64F64 is treated as i128 in IDL
    }
}