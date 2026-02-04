//! Platform-specific fixes and utilities for cross-platform compatibility

use crate::platform::{Timestamp, PlatformPath, integers::*};
use std::path::PathBuf;
use chrono::{DateTime, Utc};

/// Convert database timestamp (i64) to platform-agnostic Timestamp
pub fn db_timestamp_to_platform(db_ts: i64) -> Timestamp {
    Timestamp::from_unix(db_ts)
}

/// Convert platform Timestamp to database timestamp (i64)
pub fn platform_timestamp_to_db(ts: Timestamp) -> i64 {
    ts.as_unix()
}

/// Convert chrono DateTime to platform Timestamp
pub fn datetime_to_platform(dt: DateTime<Utc>) -> Timestamp {
    Timestamp::from_datetime(dt)
}

/// Convert platform Timestamp to chrono DateTime
pub fn platform_to_datetime(ts: Timestamp) -> DateTime<Utc> {
    ts.to_datetime()
}

/// Safe conversion for database row counts
pub fn db_count_to_usize(count: i64) -> usize {
    from_platform_size(count.max(0) as PlatformSize)
}

/// Safe conversion for array indices to database
pub fn index_to_db(index: usize) -> i64 {
    to_platform_size(index) as i64
}

/// Platform-safe file path for configuration
pub fn get_config_path(filename: &str) -> PathBuf {
    let config_dir = std::env::var("CONFIG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PlatformPath::config_dir("betting_platform"));
    
    config_dir.join(filename)
}

/// Platform-safe temporary file path
pub fn get_temp_path(filename: &str) -> PathBuf {
    PlatformPath::temp_dir().join(filename)
}

/// Ensure path is absolute and normalized
pub fn ensure_absolute_path(path: &str) -> std::io::Result<PathBuf> {
    let path = PlatformPath::normalize(path);
    PlatformPath::ensure_absolute(&path)
}

/// Convert system time duration to i64 safely
pub fn duration_to_seconds(duration: std::time::Duration) -> i64 {
    duration.as_secs() as i64
}

/// Convert i64 seconds to Duration safely
pub fn seconds_to_duration(seconds: i64) -> std::time::Duration {
    std::time::Duration::from_secs(seconds.max(0) as u64)
}

/// Platform-specific JSON timestamp serialization
pub mod json_timestamp {
    use super::*;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    
    pub fn serialize<S>(timestamp: &Timestamp, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        timestamp.as_unix().serialize(serializer)
    }
    
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Timestamp, D::Error>
    where
        D: Deserializer<'de>,
    {
        let unix_ts = i64::deserialize(deserializer)?;
        Ok(Timestamp::from_unix(unix_ts))
    }
}

/// Platform-specific database type conversions
pub mod db_types {
    use super::*;
    
    /// Convert Rust bool to PostgreSQL bool
    pub fn bool_to_db(value: bool) -> bool {
        value
    }
    
    /// Convert PostgreSQL bool to Rust bool
    pub fn db_to_bool(value: bool) -> bool {
        value
    }
    
    /// Convert u64 to PostgreSQL BIGINT
    pub fn u64_to_db(value: u64) -> i64 {
        value as i64
    }
    
    /// Convert PostgreSQL BIGINT to u64
    pub fn db_to_u64(value: i64) -> u64 {
        value.max(0) as u64
    }
    
    /// Convert u128 to PostgreSQL NUMERIC string
    pub fn u128_to_db(value: u128) -> String {
        value.to_string()
    }
    
    /// Convert PostgreSQL NUMERIC string to u128
    pub fn db_to_u128(value: &str) -> Result<u128, std::num::ParseIntError> {
        value.parse()
    }
}

/// Platform-specific network address handling
pub mod network {
    use super::*;
    
    /// Parse address with platform-specific defaults
    pub fn parse_addr(addr: &str) -> String {
        if addr == "localhost" {
            "127.0.0.1".to_string()
        } else {
            addr.to_string()
        }
    }
    
    /// Get platform-specific bind address
    pub fn get_bind_addr(host: Option<&str>, port: u16) -> String {
        let host = host.unwrap_or("0.0.0.0");
        format!("{}:{}", host, port)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_timestamp_conversions() {
        let db_ts = 1704067200i64;
        let platform_ts = db_timestamp_to_platform(db_ts);
        assert_eq!(platform_timestamp_to_db(platform_ts), db_ts);
    }
    
    #[test]
    fn test_count_conversions() {
        assert_eq!(db_count_to_usize(42), 42);
        assert_eq!(db_count_to_usize(-1), 0);
        
        assert_eq!(index_to_db(42), 42);
    }
    
    #[test]
    fn test_duration_conversions() {
        let duration = std::time::Duration::from_secs(42);
        assert_eq!(duration_to_seconds(duration), 42);
        assert_eq!(seconds_to_duration(42).as_secs(), 42);
        assert_eq!(seconds_to_duration(-1).as_secs(), 0);
    }
}