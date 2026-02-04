# Cross-Platform Compatibility Documentation

## Overview

The betting platform API runner is designed to work consistently across Windows, macOS, and Linux. This document describes the platform abstraction layer and best practices for maintaining cross-platform compatibility.

## Platform Abstraction Layer

### Core Modules

1. **`platform.rs`** - Core platform abstractions
2. **`platform_fixes.rs`** - Helper functions for common conversions

### Key Components

#### 1. Timestamp Handling

The `Timestamp` type provides consistent timestamp handling across platforms:

```rust
use crate::platform::Timestamp;

// Create timestamp
let now = Timestamp::now();
let from_unix = Timestamp::from_unix(1704067200);
let from_millis = Timestamp::from_millis(1704067200000);

// Convert to/from database
let db_value = timestamp.as_unix(); // Always i64
let timestamp = Timestamp::from_unix(db_value);

// Convert to/from chrono
let datetime = timestamp.to_datetime();
let timestamp = Timestamp::from_datetime(datetime);
```

#### 2. File Path Handling

Platform-agnostic file path operations:

```rust
use crate::platform::PlatformPath;

// Get platform-specific directories
let temp_dir = PlatformPath::temp_dir();
let config_dir = PlatformPath::config_dir("betting_platform");

// Normalize paths
let path = PlatformPath::normalize("config/settings.toml");

// Ensure absolute paths
let abs_path = PlatformPath::ensure_absolute(&path)?;
```

#### 3. Integer Type Safety

Consistent integer handling across platforms:

```rust
use crate::platform::integers::*;

// Platform-agnostic size types
let size: PlatformSize = to_platform_size(usize_value);
let usize_value = from_platform_size(size);

// Safe conversions
let i64_val = i32_to_i64(i32_value);
let i32_val = i64_to_i32_safe(i64_value)?; // Returns Option
```

## Common Patterns

### Database Type Conversions

```rust
use crate::platform_fixes::db_types::*;

// u64 <-> PostgreSQL BIGINT
let db_val = u64_to_db(rust_u64); // Returns i64
let rust_u64 = db_to_u64(db_i64);

// u128 <-> PostgreSQL NUMERIC
let db_str = u128_to_db(rust_u128); // Returns String
let rust_u128 = db_to_u128(&db_str)?;

// Count conversions
let count = db_count_to_usize(row.get::<_, i64>(0));
let db_index = index_to_db(vec.len());
```

### Timestamp Conversions

```rust
use crate::platform_fixes::*;

// Database timestamps
let platform_ts = db_timestamp_to_platform(row.get::<_, i64>(0));
let db_ts = platform_timestamp_to_db(platform_ts);

// Duration conversions
let seconds = duration_to_seconds(std::time::Duration::from_secs(30));
let duration = seconds_to_duration(seconds);
```

### File Path Best Practices

```rust
use crate::platform_fixes::*;

// Get configuration file path
let config_path = get_config_path("settings.toml");

// Get temporary file path
let temp_path = get_temp_path("upload_12345.tmp");

// Ensure absolute path
let abs_path = ensure_absolute_path("./data/markets.db")?;
```

## Platform-Specific Considerations

### Windows

- File paths use backslashes (`\`)
- Temporary directory: `%TEMP%`
- Config directory: `%APPDATA%\betting_platform`
- Line endings: `\r\n`
- No file execute permissions

### macOS

- File paths use forward slashes (`/`)
- Temporary directory: `/tmp`
- Config directory: `~/Library/Application Support/betting_platform`
- Line endings: `\n`
- File permissions via Unix mode

### Linux

- File paths use forward slashes (`/`)
- Temporary directory: `/tmp`
- Config directory: `$XDG_CONFIG_HOME/betting_platform` or `~/.config/betting_platform`
- Line endings: `\n`
- File permissions via Unix mode

## Migration Guide

### Updating Existing Code

1. **Replace hardcoded paths:**
```rust
// Before
let path = "/tmp/upload.tmp";

// After
let path = PlatformPath::temp_dir().join("upload.tmp");
```

2. **Fix timestamp handling:**
```rust
// Before
let timestamp = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .unwrap()
    .as_secs() as i64;

// After
let timestamp = Timestamp::now().as_unix();
```

3. **Safe integer conversions:**
```rust
// Before
let count = row.get::<_, i64>(0) as usize;

// After
let count = db_count_to_usize(row.get::<_, i64>(0));
```

## Testing Cross-Platform Code

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_cross_platform_paths() {
        let path = PlatformPath::normalize("foo/bar");
        
        #[cfg(windows)]
        assert_eq!(path.to_str().unwrap(), "foo\\bar");
        
        #[cfg(not(windows))]
        assert_eq!(path.to_str().unwrap(), "foo/bar");
    }
}
```

### CI Testing

Ensure CI tests run on:
- Ubuntu (Linux)
- macOS
- Windows

## Common Pitfalls

1. **Hardcoded file separators** - Use `Path::join()` instead
2. **Assuming `/tmp` exists** - Use `PlatformPath::temp_dir()`
3. **Integer overflow** - Use safe conversion functions
4. **Timestamp precision** - Always use `Timestamp` type
5. **Line endings in files** - Use `EnvUtils::line_ending()`

## Performance Considerations

The platform abstraction layer adds minimal overhead:
- Type conversions are compile-time where possible
- Runtime checks only for safety-critical operations
- No dynamic dispatch or boxing

## Future Enhancements

1. **ARM64 support** - Test on Apple Silicon and ARM Linux
2. **WASM support** - For browser-based deployment
3. **Mobile platforms** - iOS and Android via FFI
4. **Embedded systems** - Lightweight builds for IoT

## Troubleshooting

### "Time went backwards" panic
```rust
// Use safe timestamp creation
let ts = Timestamp::now(); // Won't panic
```

### Path not found errors
```rust
// Always ensure paths are absolute
let path = PlatformPath::ensure_absolute(&relative_path)?;
```

### Integer overflow in database
```rust
// Use safe conversions
if let Some(i32_val) = i64_to_i32_safe(large_value) {
    // Safe to use
} else {
    // Handle overflow
}
```