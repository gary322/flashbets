pub mod classification_engine;
pub mod verse_registry;
pub mod verse_metadata;

pub use classification_engine::*;
pub use verse_registry::*;
pub use verse_metadata::*;

// Constants
pub const MAX_TITLE_LENGTH: usize = 256;
pub const MAX_KEYWORDS: usize = 50;
pub const MAX_VERSE_DEPTH: u8 = 32;
pub const LEVENSHTEIN_THRESHOLD: u8 = 5;