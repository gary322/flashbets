use anchor_lang::prelude::*;
use fixed::types::{U64F64 as FixedU64F64, I64F64 as FixedI64F64};
use std::fmt;

/// Wrapper for U64F64 that implements AnchorSerialize/AnchorDeserialize
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct U64F64(pub FixedU64F64);

impl U64F64 {
    pub fn from_num<Src: fixed::traits::ToFixed>(val: Src) -> Self {
        Self(FixedU64F64::from_num(val))
    }
    
    pub fn to_num<Dst: fixed::traits::FromFixed>(self) -> Dst {
        self.0.to_num()
    }
    
    pub fn one() -> Self {
        Self(FixedU64F64::ONE)
    }
    
    pub fn zero() -> Self {
        Self(FixedU64F64::ZERO)
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

impl fmt::Debug for U64F64 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::Display for U64F64 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

// Implement arithmetic operations
impl std::ops::Add for U64F64 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl std::ops::Sub for U64F64 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl std::ops::Mul for U64F64 {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self::Output {
        Self(self.0 * rhs.0)
    }
}

impl std::ops::Div for U64F64 {
    type Output = Self;
    fn div(self, rhs: Self) -> Self::Output {
        Self(self.0 / rhs.0)
    }
}

impl std::iter::Sum for U64F64 {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::zero(), |acc, x| acc + x)
    }
}

/// Wrapper for I64F64 that implements AnchorSerialize/AnchorDeserialize
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct I64F64(pub FixedI64F64);

impl I64F64 {
    pub fn from_num<Src: fixed::traits::ToFixed>(val: Src) -> Self {
        Self(FixedI64F64::from_num(val))
    }
    
    pub fn to_num<Dst: fixed::traits::FromFixed>(self) -> Dst {
        self.0.to_num()
    }
    
    pub fn one() -> Self {
        Self(FixedI64F64::ONE)
    }
    
    pub fn zero() -> Self {
        Self(FixedI64F64::ZERO)
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

impl fmt::Debug for I64F64 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::Display for I64F64 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

// Implement arithmetic operations
impl std::ops::Add for I64F64 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl std::ops::Sub for I64F64 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl std::ops::Mul for I64F64 {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self::Output {
        Self(self.0 * rhs.0)
    }
}

impl std::ops::Div for I64F64 {
    type Output = Self;
    fn div(self, rhs: Self) -> Self::Output {
        Self(self.0 / rhs.0)
    }
}

impl std::ops::Neg for I64F64 {
    type Output = Self;
    fn neg(self) -> Self::Output {
        Self(-self.0)
    }
}