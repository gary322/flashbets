//! Platform abstraction layer for cross-platform compatibility
//! Handles differences in file paths, timestamps, and system-specific types

use std::{
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Platform-agnostic timestamp type (always uses i64 for consistency)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Timestamp(i64);

impl Timestamp {
    /// Create a new timestamp from the current time
    pub fn now() -> Self {
        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards");
        Self(duration.as_secs() as i64)
    }
    
    /// Create from Unix timestamp (seconds since epoch)
    pub fn from_unix(secs: i64) -> Self {
        Self(secs)
    }
    
    /// Create from milliseconds since epoch
    pub fn from_millis(millis: i64) -> Self {
        Self(millis / 1000)
    }
    
    /// Get as Unix timestamp
    pub fn as_unix(&self) -> i64 {
        self.0
    }
    
    /// Get as milliseconds since epoch
    pub fn as_millis(&self) -> i64 {
        self.0 * 1000
    }
    
    /// Convert to chrono DateTime
    pub fn to_datetime(&self) -> DateTime<Utc> {
        DateTime::from_timestamp(self.0, 0).unwrap_or_else(|| Utc::now())
    }
    
    /// Create from chrono DateTime
    pub fn from_datetime(dt: DateTime<Utc>) -> Self {
        Self(dt.timestamp())
    }
    
    /// Add duration
    pub fn add_duration(&self, duration: Duration) -> Self {
        Self(self.0 + duration.as_secs() as i64)
    }
    
    /// Subtract duration
    pub fn sub_duration(&self, duration: Duration) -> Self {
        Self(self.0 - duration.as_secs() as i64)
    }
    
    /// Duration since this timestamp
    pub fn elapsed(&self) -> Duration {
        let now = Self::now();
        if now.0 >= self.0 {
            Duration::from_secs((now.0 - self.0) as u64)
        } else {
            Duration::from_secs(0)
        }
    }
}

/// Platform-agnostic file path handling
pub struct PlatformPath;

impl PlatformPath {
    /// Get the platform-specific temporary directory
    pub fn temp_dir() -> PathBuf {
        #[cfg(target_os = "windows")]
        {
            std::env::temp_dir()
        }
        #[cfg(not(target_os = "windows"))]
        {
            PathBuf::from("/tmp")
        }
    }
    
    /// Normalize path separators for the current platform
    pub fn normalize(path: &str) -> PathBuf {
        #[cfg(target_os = "windows")]
        {
            PathBuf::from(path.replace('/', "\\"))
        }
        #[cfg(not(target_os = "windows"))]
        {
            PathBuf::from(path.replace('\\', "/"))
        }
    }
    
    /// Join paths in a platform-agnostic way
    pub fn join<P: AsRef<Path>>(base: &Path, path: P) -> PathBuf {
        base.join(path)
    }
    
    /// Get home directory
    pub fn home_dir() -> Option<PathBuf> {
        #[cfg(target_os = "windows")]
        {
            std::env::var("USERPROFILE").ok().map(PathBuf::from)
        }
        #[cfg(not(target_os = "windows"))]
        {
            std::env::var("HOME").ok().map(PathBuf::from)
        }
    }
    
    /// Get config directory
    pub fn config_dir(app_name: &str) -> PathBuf {
        #[cfg(target_os = "windows")]
        {
            if let Ok(appdata) = std::env::var("APPDATA") {
                PathBuf::from(appdata).join(app_name)
            } else {
                Self::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(format!(".{}", app_name))
            }
        }
        #[cfg(target_os = "macos")]
        {
            Self::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("Library")
                .join("Application Support")
                .join(app_name)
        }
        #[cfg(target_os = "linux")]
        {
            if let Ok(xdg_config) = std::env::var("XDG_CONFIG_HOME") {
                PathBuf::from(xdg_config).join(app_name)
            } else {
                Self::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".config")
                    .join(app_name)
            }
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        {
            PathBuf::from(".").join(app_name)
        }
    }
    
    /// Ensure a path is absolute
    pub fn ensure_absolute(path: &Path) -> std::io::Result<PathBuf> {
        if path.is_absolute() {
            Ok(path.to_path_buf())
        } else {
            std::env::current_dir().map(|cwd| cwd.join(path))
        }
    }
}

/// Platform-agnostic integer types
pub mod integers {
    /// Platform-agnostic size type (always 64-bit)
    pub type PlatformSize = u64;
    
    /// Platform-agnostic signed size type (always 64-bit)
    pub type PlatformSSize = i64;
    
    /// Convert usize to platform-agnostic size
    pub fn to_platform_size(size: usize) -> PlatformSize {
        size as PlatformSize
    }
    
    /// Convert from platform-agnostic size to usize
    pub fn from_platform_size(size: PlatformSize) -> usize {
        size as usize
    }
    
    /// Safe conversion from i32 to i64
    pub fn i32_to_i64(value: i32) -> i64 {
        value as i64
    }
    
    /// Safe conversion from i64 to i32 with bounds checking
    pub fn i64_to_i32_safe(value: i64) -> Option<i32> {
        if value >= i32::MIN as i64 && value <= i32::MAX as i64 {
            Some(value as i32)
        } else {
            None
        }
    }
}

/// Platform-specific network configuration
pub struct NetworkConfig;

impl NetworkConfig {
    /// Get platform-specific localhost address
    pub fn localhost() -> &'static str {
        "127.0.0.1"
    }
    
    /// Get platform-specific any address
    pub fn any_addr() -> &'static str {
        "0.0.0.0"
    }
    
    /// Check if an address is loopback
    pub fn is_loopback(addr: &str) -> bool {
        addr == "127.0.0.1" || addr == "localhost" || addr == "::1"
    }
}

/// Platform-specific process utilities
pub struct ProcessUtils;

impl ProcessUtils {
    /// Get current process ID in a platform-agnostic way
    pub fn current_pid() -> u32 {
        std::process::id()
    }
    
    /// Check if running with elevated privileges
    pub fn is_elevated() -> bool {
        #[cfg(target_os = "windows")]
        {
            // Windows-specific check would go here
            false
        }
        #[cfg(unix)]
        {
            unsafe { libc::geteuid() == 0 }
        }
        #[cfg(not(any(target_os = "windows", unix)))]
        {
            false
        }
    }
}

/// Platform-specific file permissions
pub struct FilePermissions;

impl FilePermissions {
    /// Set file as executable (Unix-specific, no-op on Windows)
    pub fn set_executable(path: &Path) -> std::io::Result<()> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(path)?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(path, perms)?;
        }
        Ok(())
    }
    
    /// Set file as read-only
    pub fn set_readonly(path: &Path) -> std::io::Result<()> {
        let mut perms = std::fs::metadata(path)?.permissions();
        perms.set_readonly(true);
        std::fs::set_permissions(path, perms)
    }
}

/// Platform-specific environment utilities
pub struct EnvUtils;

impl EnvUtils {
    /// Get platform-specific line ending
    pub fn line_ending() -> &'static str {
        #[cfg(target_os = "windows")]
        {
            "\r\n"
        }
        #[cfg(not(target_os = "windows"))]
        {
            "\n"
        }
    }
    
    /// Get platform name
    pub fn platform_name() -> &'static str {
        #[cfg(target_os = "windows")]
        {
            "windows"
        }
        #[cfg(target_os = "macos")]
        {
            "macos"
        }
        #[cfg(target_os = "linux")]
        {
            "linux"
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        {
            "unknown"
        }
    }
    
    /// Check if running in CI environment
    pub fn is_ci() -> bool {
        std::env::var("CI").is_ok() || 
        std::env::var("CONTINUOUS_INTEGRATION").is_ok() ||
        std::env::var("GITHUB_ACTIONS").is_ok() ||
        std::env::var("TRAVIS").is_ok() ||
        std::env::var("CIRCLECI").is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_timestamp_conversions() {
        let ts = Timestamp::from_unix(1704067200); // 2024-01-01 00:00:00 UTC
        assert_eq!(ts.as_unix(), 1704067200);
        assert_eq!(ts.as_millis(), 1704067200000);
        
        let ts2 = Timestamp::from_millis(1704067200000);
        assert_eq!(ts2.as_unix(), 1704067200);
    }
    
    #[test]
    fn test_platform_path() {
        let temp = PlatformPath::temp_dir();
        assert!(temp.exists());
        
        let normalized = PlatformPath::normalize("foo/bar/baz");
        #[cfg(target_os = "windows")]
        assert_eq!(normalized.to_str().unwrap(), "foo\\bar\\baz");
        #[cfg(not(target_os = "windows"))]
        assert_eq!(normalized.to_str().unwrap(), "foo/bar/baz");
    }
    
    #[test]
    fn test_integer_conversions() {
        use integers::*;
        
        let size: usize = 42;
        let platform_size = to_platform_size(size);
        assert_eq!(from_platform_size(platform_size), size);
        
        assert_eq!(i32_to_i64(42), 42i64);
        assert_eq!(i64_to_i32_safe(42), Some(42i32));
        assert_eq!(i64_to_i32_safe(i64::MAX), None);
    }
}