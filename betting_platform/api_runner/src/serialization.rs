//! Type-safe serialization utilities for JavaScript compatibility

use serde::{Deserialize, Serialize, Serializer, Deserializer};
use serde::de::{self, Visitor};
use std::fmt;
use std::str::FromStr;
use solana_sdk::pubkey::Pubkey;

/// Wrapper for u128 that serializes as string to preserve precision in JavaScript
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SafeU128(pub u128);

impl Serialize for SafeU128 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for SafeU128 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct SafeU128Visitor;

        impl<'de> Visitor<'de> for SafeU128Visitor {
            type Value = SafeU128;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a string containing a u128 number")
            }

            fn visit_str<E>(self, value: &str) -> Result<SafeU128, E>
            where
                E: de::Error,
            {
                let num = value.parse::<u128>()
                    .map_err(|_| E::custom(format!("invalid u128: {}", value)))?;
                Ok(SafeU128(num))
            }

            fn visit_u64<E>(self, value: u64) -> Result<SafeU128, E>
            where
                E: de::Error,
            {
                Ok(SafeU128(value as u128))
            }
        }

        deserializer.deserialize_any(SafeU128Visitor)
    }
}

/// Wrapper for u64 that serializes as string for amounts to preserve precision
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SafeU64(pub u64);

impl Serialize for SafeU64 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for SafeU64 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct SafeU64Visitor;

        impl<'de> Visitor<'de> for SafeU64Visitor {
            type Value = SafeU64;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a string containing a u64 number")
            }

            fn visit_str<E>(self, value: &str) -> Result<SafeU64, E>
            where
                E: de::Error,
            {
                let num = value.parse::<u64>()
                    .map_err(|_| E::custom(format!("invalid u64: {}", value)))?;
                Ok(SafeU64(num))
            }

            fn visit_u64<E>(self, value: u64) -> Result<SafeU64, E>
            where
                E: de::Error,
            {
                Ok(SafeU64(value))
            }
        }

        deserializer.deserialize_any(SafeU64Visitor)
    }
}

// Conversion implementations
impl From<u128> for SafeU128 {
    fn from(value: u128) -> Self {
        SafeU128(value)
    }
}

impl From<SafeU128> for u128 {
    fn from(value: SafeU128) -> Self {
        value.0
    }
}

impl From<u64> for SafeU64 {
    fn from(value: u64) -> Self {
        SafeU64(value)
    }
}

impl From<SafeU64> for u64 {
    fn from(value: SafeU64) -> Self {
        value.0
    }
}

/// Serialize a Pubkey as a base58 string
pub fn serialize_pubkey<S>(pubkey: &Pubkey, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&pubkey.to_string())
}

/// Deserialize a Pubkey from a base58 string
pub fn deserialize_pubkey<'de, D>(deserializer: D) -> Result<Pubkey, D::Error>
where
    D: Deserializer<'de>,
{
    struct PubkeyVisitor;

    impl<'de> Visitor<'de> for PubkeyVisitor {
        type Value = Pubkey;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a base58-encoded public key")
        }

        fn visit_str<E>(self, value: &str) -> Result<Pubkey, E>
        where
            E: de::Error,
        {
            Pubkey::from_str(value)
                .map_err(|_| E::custom(format!("invalid public key: {}", value)))
        }
    }

    deserializer.deserialize_str(PubkeyVisitor)
}

#[allow(dead_code)]#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_safe_u128_serialization() {
        let value = SafeU128(u128::MAX);
        let json = serde_json::to_string(&value).unwrap();
        assert_eq!(json, "\"340282366920938463463374607431768211455\"");
        
        let deserialized: SafeU128 = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, value);
    }

    #[test]
    fn test_safe_u64_serialization() {
        let value = SafeU64(u64::MAX);
        let json = serde_json::to_string(&value).unwrap();
        assert_eq!(json, "\"18446744073709551615\"");
        
        let deserialized: SafeU64 = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, value);
    }
}