// Fixed-point math module
// Native Solana implementation - NO ANCHOR

pub mod fixed_point;
pub mod functions;
pub mod trigonometry;
pub mod lookup_tables;
pub mod utils;

// Re-export commonly used types and functions
pub use fixed_point::{U64F64, U128F128, U256, MathError};
pub use functions::MathFunctions;
pub use trigonometry::TrigFunctions;
pub use lookup_tables::{PrecomputedTables, TABLE_SIZE};
pub use utils::{MathUtils, LeverageUtils, FeeUtils};

// Re-export constants
pub use fixed_point::{ONE, HALF, E, PI, SQRT2, LN2};