//! Error types for the betting platform
//! 
//! Complete migration of all 89 Anchor error codes to native Solana

use num_derive::FromPrimitive;
use solana_program::{
    decode_error::DecodeError,
    program_error::{ProgramError, PrintProgramError},
    msg,
};
use thiserror::Error;

/// Custom error type for the betting platform
#[derive(Clone, Debug, Eq, Error, FromPrimitive, PartialEq)]
pub enum BettingPlatformError {
    #[error("Demo account reset is on cooldown")]
    DemoResetCooldown = 5998,
    
    #[error("Numerical overflow occurred")]
    NumericalOverflow = 5999,
    
    // Coverage and leverage errors (6000-6009)
    #[error("Insufficient coverage for leverage")]
    InsufficientCoverage = 6000,
    
    #[error("Maximum leverage exceeded")]
    MaxLeverageExceeded = 6001,
    
    #[error("System halted")]
    SystemHalted = 6002,
    
    #[error("Invalid verse hierarchy")]
    InvalidVerseHierarchy = 6003,
    
    #[error("Arithmetic overflow")]
    ArithmeticOverflow = 6004,
    
    #[error("Arithmetic underflow")]
    ArithmeticUnderflow = 6005,
    
    #[error("Division by zero")]
    DivisionByZero = 6006,
    
    #[error("Invalid input")]
    InvalidInput = 6007,
    
    #[error("Emergency halt expired")]
    EmergencyHaltExpired = 6008,
    
    #[error("Invalid coverage")]
    InvalidCoverage = 6009,
    
    #[error("Insufficient funds")]
    InsufficientFunds = 6334,
    
    #[error("Oracle spread too high")]
    OracleSpreadTooHigh = 6335,
    
    #[error("Leverage too high")]
    LeverageTooHigh = 6338,
    
    // Vault and fee errors (6010-6019)
    #[error("Invalid vault decrease")]
    InvalidVaultDecrease = 6010,
    
    #[error("Invalid fee rate")]
    InvalidFeeRate = 6011,
    
    #[error("Insufficient vault balance")]
    InsufficientVaultBalance = 6012,
    
    // Verse and proposal errors (6020-6029)
    #[error("Verse not found")]
    VerseNotFound = 6020,
    
    #[error("Proposal not found")]
    ProposalNotFound = 6021,
    
    #[error("Invalid proposal status")]
    InvalidProposalStatus = 6022,
    
    #[error("Proposal expired")]
    ProposalExpired = 6023,
    
    #[error("Verse not active")]
    VerseNotActive = 6024,
    
    #[error("Verse already settled")]
    VerseSettled = 6025,
    
    #[error("Invalid verse hierarchy")]
    CircularHierarchy = 6026,
    
    #[error("Maximum depth exceeded")]
    MaxDepthExceeded = 6027,
    
    #[error("Invalid proposal")]
    InvalidProposal = 6028,
    
    #[error("Oracle cache miss")]
    OracleCacheMiss = 6029,
    
    // Authorization and access errors (6030-6039)
    #[error("Unauthorized")]
    Unauthorized = 6030,
    
    #[error("Already resolved")]
    AlreadyResolved = 6031,
    
    // Trading errors (6040-6049)
    #[error("Invalid outcome")]
    InvalidOutcome = 6040,
    
    #[error("Insufficient balance")]
    InsufficientBalance = 6041,
    
    #[error("Invalid leverage tier")]
    InvalidLeverageTier = 6042,
    
    #[error("Position not found")]
    PositionNotFound = 6043,
    
    #[error("Invalid position")]
    InvalidPosition = 6044,
    
    #[error("Invalid position index")]
    InvalidPositionIndex = 6045,
    
    #[error("Excessive leverage")]
    ExcessiveLeverage = 6046,
    
    // Oracle and price errors (6050-6059)
    #[error("Invalid oracle feed")]
    InvalidOracleFeed = 6050,
    
    #[error("Stale price data")]
    StalePrice = 6051,
    
    #[error("Excessive price movement detected")]
    ExcessivePriceMovement = 6052,
    
    #[error("Stale price update")]
    StalePriceUpdate = 6053,
    
    // MMT and token errors (6060-6069)
    #[error("Mint authority not burned")]
    MintAuthorityNotBurned = 6060,
    
    #[error("Invalid MMT supply")]
    InvalidMMTSupply = 6061,
    
    // Math and calculation errors (6070-6079)
    #[error("Math overflow")]
    MathOverflow = 6070,
    
    #[error("Price clamp exceeded")]
    PriceClampExceeded = 6071,
    
    #[error("Invalid distribution")]
    InvalidDistribution = 6072,
    
    // Liquidation errors (6080-6089)
    #[error("Position healthy - cannot liquidate")]
    PositionHealthy = 6080,
    
    #[error("Insufficient liquidation buffer")]
    InsufficientLiquidationBuffer = 6081,
    
    #[error("Position not at risk")]
    PositionNotAtRisk = 6082,
    
    // Verse and hierarchy errors (6090-6099)
    #[error("Verse capacity exceeded")]
    VerseCapacityExceeded = 6090,
    
    #[error("Single parent invariant violated")]
    SingleParentInvariant = 6091,
    
    #[error("Parent verse not found")]
    ParentVerseNotFound = 6092,
    
    
    #[error("Max children exceeded")]
    MaxChildrenExceeded = 6094,
    
    // Oracle coordination errors (6100-6109)
    #[error("No healthy oracles available")]
    NoHealthyOracles = 6100,
    
    #[error("External API error")]
    ExternalApiError = 6101,
    
    #[error("Mirror not active")]
    MirrorNotActive = 6102,
    
    #[error("Invalid bundle size")]
    InvalidBundleSize = 6103,
    
    // Chain execution errors (6110-6119)
    #[error("Invalid chain steps")]
    InvalidChainSteps = 6110,
    
    #[error("Chain not found")]
    ChainNotFound = 6111,
    
    #[error("Circular dependency detected")]
    CircularDependency = 6112,
    
    #[error("Self dependency not allowed")]
    SelfDependency = 6113,
    
    #[error("Cross-verse not allowed")]
    CrossVerseNotAllowed = 6114,
    
    // Circuit breaker errors (6095-6099)
    #[error("Circuit breaker triggered")]
    CircuitBreakerTriggered = 6095,
    
    #[error("Low coverage")]
    LowCoverage = 6096,
    
    #[error("Inconsistent coverage")]
    InconsistentCoverage = 6097,
    
    #[error("Unauthorized emergency shutdown attempt")]
    UnauthorizedEmergency = 6098,
    
    #[error("Attack detected")]
    AttackDetected = 6099,
    
    // Chain execution errors (6102-6109)
    #[error("Invalid deposit amount")]
    InvalidDeposit = 6202,
    
    #[error("Too many chain steps")]
    TooManySteps = 6203,
    
    #[error("No chain steps provided")]
    NoSteps = 6104,
    
    #[error("Inactive verse")]
    InactiveVerse = 6105,
    
    #[error("Wrong verse")]
    WrongVerse = 6106,
    
    #[error("Invalid chain status")]
    InvalidChainStatus = 6107,
    
    #[error("Chain cycle detected")]
    ChainCycle = 6108,
    
    #[error("Exceeds verse limit")]
    ExceedsVerseLimit = 6109,
    
    // Keeper network errors (6110-6119)
    #[error("No rewards to claim")]
    NoRewardsToClaim = 6122,
    
    #[error("Insufficient stake")]
    InsufficientStake = 6123,
    
    #[error("Queue full")]
    QueueFull = 6124,
    
    #[error("Entry not found")]
    EntryNotFound = 6437,
    
    #[error("No work available")]
    NoWorkAvailable = 6125,
    
    #[error("No active keepers available")]
    NoActiveKeepers = 6126,
    
    #[error("Stop condition not met")]
    StopConditionNotMet = 6115,
    
    #[error("Insufficient prepaid bounty")]
    InsufficientPrepaidBounty = 6116,
    
    #[error("No backup keeper available")]
    NoBackupKeeperAvailable = 6117,
    
    #[error("Market not found")]
    MarketNotFound = 6118,
    
    #[error("Conflicting resolution")]
    ConflictingResolution = 6119,
    
    // AMM errors (6120-6129)
    #[error("Invalid shares amount")]
    InvalidShares = 6120,
    
    // Advanced order errors (6130-6149)
    #[error("Invalid randomization value")]
    InvalidRandomization = 6130,
    
    #[error("Invalid order type")]
    InvalidOrderType = 6131,
    
    #[error("Invalid slice count")]
    InvalidSliceCount = 6172,
    
    #[error("TWAP order complete")]
    TWAPComplete = 6173,
    
    #[error("TWAP execution too early")]
    TWAPTooEarly = 6174,
    
    #[error("Slice too small")]
    SliceTooSmall = 6175,
    
    #[error("No verse probability available")]
    NoVerseProbability = 6176,
    
    #[error("Invalid VRF output")]
    InvalidVRFOutput = 6177,
    
    #[error("Stale VRF output")]
    StaleVRFOutput = 6178,
    
    #[error("Invalid IPFS hash")]
    InvalidIPFSHash = 6179,
    
    #[error("Feature not enabled")]
    FeatureNotEnabled = 6180,
    
    #[error("Dark pool not active")]
    DarkPoolNotActive = 6170,
    
    #[error("Below minimum size")]
    BelowMinimumSize = 6171,
    
    #[error("Too early to settle")]
    TooEarlyToSettle = 6199,
    
    // Monitoring errors (6140-6149)
    #[error("Invalid alert index")]
    InvalidAlertIndex = 6303,
    
    #[error("Invalid operation")]
    InvalidOperation = 6141,
    
    #[error("Unauthorized recovery action")]
    UnauthorizedRecoveryAction = 6181,
    
    #[error("Invalid checkpoint")]
    InvalidCheckpoint = 6182,
    
    #[error("Order not active")]
    OrderNotActive = 6183,
    
    #[error("Order conditions not met")]
    OrderConditionsNotMet = 6184,
    
    #[error("Recovery incomplete")]
    RecoveryIncomplete = 6144,
    
    #[error("Unverified checkpoint")]
    UnverifiedCheckpoint = 6145,
    
    #[error("Invalid Polymarket program")]
    InvalidPolymarketProgram = 6146,
    
    #[error("Unsupported order type")]
    UnsupportedOrderType = 6147,
    
    #[error("Polymarket routing failed")]
    PolymarketRoutingFailed = 6148,
    
    #[error("Order expired")]
    OrderExpired = 6149,
    
    #[error("Price sum does not equal 1")]
    PriceSumError = 6121,
    
    #[error("Convergence failed in numerical solver")]
    ConvergenceFailed = 6127,
    
    #[error("Insufficient points for integration")]
    InsufficientPoints = 6128,
    
    #[error("Unsupported distribution type")]
    UnsupportedDistribution = 6129,
    
    // Advanced trading errors (6130-6139)
    #[error("Invalid visible size for iceberg order")]
    InvalidVisibleSize = 6201,
    
    #[error("Invalid total size for iceberg order")]
    InvalidTotalSize = 6302,
    
    #[error("Visible size too large for iceberg order")]
    VisibleSizeTooLarge = 6132,
    
    #[error("Exceeds visible size")]
    ExceedsVisibleSize = 6133,
    
    #[error("Not an iceberg order")]
    NotIcebergOrder = 6134,
    
    #[error("Invalid intervals for TWAP order")]
    InvalidIntervals = 6135,
    
    #[error("Duration too short for TWAP order")]
    DurationTooShort = 6136,
    
    #[error("Size too small for order")]
    SizeTooSmall = 6137,
    
    #[error("Invalid TWAP state")]
    InvalidTWAPState = 6138,
    
    #[error("Too early for TWAP execution")]
    TooEarlyForTWAP = 6139,
    
    // Rollback protection errors (6510-6519)
    #[error("Invalid state version")]
    InvalidStateVersion = 6510,
    
    #[error("State frozen for migration")]
    StateFrozen = 6511,
    
    #[error("Invalid slot progression")]
    InvalidSlotProgression = 6512,
    
    #[error("Hash chain broken")]
    HashChainBroken = 6513,
    
    #[error("Nonce reused")]
    NonceReused = 6514,
    
    #[error("Nonce too high")]
    NonceTooHigh = 6515,
    
    #[error("Migration in progress")]
    MigrationInProgress = 6516,
    
    #[error("Invalid migration target")]
    InvalidMigrationTarget = 6517,
    
    #[error("No migration in progress")]
    NoMigrationInProgress = 6518,
    
    #[error("Not a TWAP order")]
    NotTWAPOrder = 6140,
    
    // Validation errors (6141-6149)
    
    #[error("Invalid global config")]
    InvalidGlobalConfig = 6142,
    
    #[error("Invalid user map")]
    InvalidUserMap = 6143,
    
    // State compression errors (6150-6159)
    #[error("Invalid compression proof")]
    InvalidCompressionProof = 6150,
    
    #[error("Decompression failed")]
    DecompressionFailed = 6151,
    
    #[error("Invalid proof type")]
    InvalidProofType = 6152,
    
    // Implementation errors (6160+)
    
    /// Invalid account owner
    #[error("Invalid account owner")]
    InvalidAccountOwner = 6161,
    
    /// Invalid account data
    #[error("Invalid account data")]
    InvalidAccountData = 6162,
    
    /// Slippage exceeded
    #[error("Slippage exceeded")]
    SlippageExceeded = 6163,
    
    /// Overflow
    #[error("Overflow")]
    Overflow = 6164,
    
    /// Market not active
    #[error("Market not active")]
    MarketNotActive = 6165,
    
    /// Invalid trade amount
    #[error("Invalid trade amount")]
    InvalidTradeAmount = 6166,
    
    /// Insufficient shares
    #[error("Insufficient shares")]
    InsufficientShares = 6167,
    
    /// Insufficient liquidity
    #[error("Insufficient liquidity")]
    InsufficientLiquidity = 6168,
    
    /// Invalid market state
    #[error("Invalid market state")]
    InvalidMarketState = 6169,
    
    /// Trade too large
    #[error("Trade too large")]
    TradeTooLarge = 6195,
    
    /// Market already resolved
    #[error("Market already resolved")]
    MarketAlreadyResolved = 6196,
    
    /// Invalid oracle
    #[error("Invalid oracle")]
    InvalidOracle = 6197,
    
    /// Fee too high
    #[error("Fee too high")]
    FeeTooHigh = 6198,
    
    /// Update too frequent
    #[error("Update too frequent")]
    UpdateTooFrequent = 6321,
    
    /// Fee increase too large
    #[error("Fee increase too large")]
    FeeIncreaseTooLarge = 6200,
    
    /// Liquidity too high
    #[error("Liquidity too high")]
    LiquidityTooHigh = 6216,
    
    /// Liquidity change too large
    #[error("Liquidity change too large")]
    LiquidityChangeTooLarge = 6217,
    
    /// Market not resolved
    #[error("Market not resolved")]
    MarketNotResolved = 6218,
    
    /// Settlement period not complete
    #[error("Settlement period not complete")]
    SettlementPeriodNotComplete = 6219,
    
    /// Invalid range
    #[error("Invalid range")]
    InvalidRange = 6220,
    
    /// Invalid leverage
    #[error("Invalid leverage")]
    InvalidLeverage = 6221,
    
    /// Invalid time range
    #[error("Invalid time range")]
    InvalidTimeRange = 6559,
    
    /// Invalid conversion
    #[error("Invalid conversion")]
    InvalidConversion = 6222,
    
    /// Too many positions
    #[error("Too many positions")]
    TooManyPositions = 6300,
    
    /// Price out of bounds
    #[error("Price out of bounds")]
    PriceOutOfBounds = 6301,
    
    /// Price manipulation detected
    #[error("Price manipulation detected")]
    PriceManipulation = 6402,
    
    /// Invalid probability sum
    #[error("Invalid probability sum")]
    InvalidProbabilitySum = 6403,
    
    /// Cross verse violation
    #[error("Cross verse violation")]
    CrossVerseViolation = 6304,
    
    /// Value leakage detected
    #[error("Value leakage detected")]
    ValueLeakage = 6305,
    
    /// Invalid quantum state
    #[error("Invalid quantum state")]
    InvalidQuantumState = 6306,
    
    /// Position already closed
    #[error("Position already closed")]
    PositionAlreadyClosed = 6185,
    
    /// No claimable amount
    #[error("No claimable amount")]
    NoClaimableAmount = 6186,
    
    /// Invalid AMM type
    #[error("Invalid AMM type")]
    InvalidAMMType = 6187,
    
    /// Invalid size
    #[error("Invalid size")]
    InvalidSize = 6188,
    
    /// Insufficient margin
    #[error("Insufficient margin")]
    InsufficientMargin = 6189,
    
    /// Invalid mint
    #[error("Invalid mint")]
    InvalidMint = 6190,
    
    /// Insufficient collateral
    #[error("Insufficient collateral")]
    InsufficientCollateral = 6191,
    
    /// Underflow
    #[error("Underflow")]
    Underflow = 6192,
    
    /// Invalid proof
    #[error("Invalid proof")]
    InvalidProof = 6193,
    
    /// Rate limited
    #[error("Rate limited - too many requests")]
    RateLimited = 6194,
    
    /// Security check failed
    #[error("Security check failed")]
    SecurityCheckFailed = 6223,
    
    // Resolution errors (6224-6239)
    #[error("Already signed")]
    AlreadySigned = 6224,
    
    #[error("Dispute window active")]
    DisputeWindowActive = 6225,
    
    #[error("Market disputed")]
    MarketDisputed = 6226,
    
    #[error("Resolution cancelled")]
    ResolutionCancelled = 6227,
    
    #[error("Invalid resolution")]
    InvalidResolution = 6228,
    
    #[error("Dispute window closed")]
    DisputeWindowClosed = 6229,
    
    #[error("Already voted")]
    AlreadyVoted = 6230,
    
    #[error("Duplicate entry")]
    DuplicateEntry = 6231,
    
    // Synthetic wrapper errors (6232-6249)
    #[error("Too many markets for synthetic wrapper")]
    TooManyMarkets = 6232,
    
    #[error("No markets provided for wrapper")]
    NoMarketsProvided = 6233,
    
    #[error("Weight mismatch for synthetic markets")]
    WeightMismatch = 6234,
    
    #[error("Wrapper not found")]
    WrapperNotFound = 6235,
    
    #[error("Pool not active")]
    PoolNotActive = 6236,
    
    #[error("Wrapper not active")]
    WrapperNotActive = 6237,
    
    #[error("Order not found")]
    OrderNotFound = 6238,
    
    #[error("Unregistered keeper")]
    UnregisteredKeeper = 6239,
    
    #[error("Polymarket API error")]
    PolymarketApiError = 6240,
    
    #[error("Data mismatch")]
    DataMismatch = 6241,
    
    #[error("No price history")]
    NoPriceHistory = 6242,
    
    #[error("Invalid liquidity source")]
    InvalidLiquiditySource = 6243,
    
    #[error("Must route to Polymarket")]
    MustRouteToPolymarket = 6244,
    
    #[error("No internal liquidity allowed")]
    NoInternalLiquidity = 6245,
    
    #[error("Order ID mismatch")]
    OrderIdMismatch = 6246,
    
    #[error("Invalid program")]
    InvalidProgram = 6247,
    
    // Priority queue errors (6250-6269)
    #[error("Duplicate commitment")]
    DuplicateCommitment = 6250,
    
    #[error("Invalid commitment")]
    InvalidCommitment = 6251,
    
    #[error("Too early to reveal")]
    TooEarlyToReveal = 6252,
    
    #[error("Reveal deadline passed")]
    RevealDeadlinePassed = 6253,
    
    #[error("No price band")]
    NoPriceBand = 6254,
    
    #[error("Price outside bands")]
    PriceOutsideBands = 6255,
    
    #[error("Too frequent submission")]
    TooFrequentSubmission = 6256,
    
    #[error("No submission record")]
    NoSubmissionRecord = 6257,
    
    #[error("Order not in queue")]
    OrderNotInQueue = 6258,
    
    #[error("Invalid health status")]
    InvalidHealthStatus = 6314,
    
    #[error("Bootstrap already complete")]
    BootstrapAlreadyComplete = 6315,
    
    #[error("System not initialized")]
    SystemNotInitialized = 6316,
    
    #[error("Bootstrap not complete")]
    BootstrapNotComplete = 6317,
    
    #[error("Insufficient accounts")]
    InsufficientAccounts = 6318,
    
    #[error("Unauthorized admin")]
    UnauthorizedAdmin = 6319,
    
    #[error("Deposit too small")]
    DepositTooSmall = 6320,
    
    #[error("Update too frequent for recovery")]
    RecoveryUpdateTooFrequent = 6322,
    
    #[error("Recovery not needed")]
    RecoveryNotNeeded = 6323,
    
    // MathOverflow already defined at 6070
    
    #[error("Invalid component")]
    InvalidComponent = 6680,
    
    #[error("Coverage ratio below minimum")]
    CoverageRatioBelowMinimum = 6324,
    
    #[error("Bootstrap not active")]
    BootstrapNotActive = 6619,
    
    #[error("Ineligible for rewards")]
    IneligibleForRewards = 6620,
    
    #[error("Vampire attack detected")]
    VampireAttackDetected = 6621,
    
    #[error("Vampire attack cooldown")]
    VampireAttackCooldown = 6622,
    
    #[error("Suspicious withdrawal")]
    SuspiciousWithdrawal = 6623,
    
    #[error("Rapid withdrawals detected")]
    RapidWithdrawalsDetected = 6624,
    
    #[error("Suspicious address")]
    SuspiciousAddress = 6625,
    
    #[error("Invalid price feed")]
    InvalidPriceFeed = 6325,
    
    #[error("Invalid oracle signature")]
    InvalidOracleSignature = 6326,
    
    #[error("Invalid price sum")]
    InvalidPriceSum = 6327,
    
    #[error("No fallback price")]
    NoFallbackPrice = 6328,
    
    #[error("Fallback expired")]
    FallbackExpired = 6329,
    
    #[error("Insufficient confidence")]
    InsufficientConfidence = 6330,
    
    #[error("Stale price feed")]
    StalePriceFeed = 6331,
    
    #[error("Authority already burned")]
    AuthorityAlreadyBurned = 6332,
    
    #[error("Burn not scheduled")]
    BurnNotScheduled = 6333,
    
    #[error("Burn delay not met")]
    BurnDelayNotMet = 6336,
    
    #[error("Critical functions locked")]
    CriticalFunctionsLocked = 6469,
    
    #[error("No admin actions remaining")]
    NoAdminActionsRemaining = 6337,
    
    #[error("Transfer delay not met")]
    TransferDelayNotMet = 6457,
    
    #[error("No transfer pending")]
    NoTransferPending = 6458,
    
    #[error("Too many emergency contacts")]
    TooManyEmergencyContacts = 6459,
    
    #[error("Insufficient emergency signers")]
    InsufficientEmergencySigners = 6339,
    
    #[error("Upgrades exhausted")]
    UpgradesExhausted = 6340,
    
    #[error("Liquidation too small")]
    LiquidationTooSmall = 6341,
    
    #[error("Partial liquidation disabled")]
    PartialLiquidationDisabled = 6342,
    
    #[error("Test assertion failed")]
    TestAssertionFailed = 6343,
    
    
    #[error("Not implemented")]
    NotImplemented = 6345,
    
    #[error("Invalid amount")]
    InvalidAmount = 6346,
    
    #[error("Verse mismatch")]
    VerseMismatch = 6347,
    
    #[error("Insufficient oracle sources")]
    InsufficientOracleSources = 6626,
    
    #[error("Invalid Pyth account")]
    InvalidPythAccount = 6348,
    
    #[error("Invalid Pyth account type")]
    InvalidPythAccountType = 6349,
    
    #[error("Pyth price not trading")]
    PythPriceNotTrading = 6350,
    
    #[error("Invalid Pyth price")]
    InvalidPythPrice = 6351,
    
    #[error("Stale Pyth price")]
    StalePythPrice = 6352,
    
    #[error("Pyth mapping not found")]
    PythMappingNotFound = 6353,
    
    #[error("Invalid Chainlink version")]
    InvalidChainlinkVersion = 6354,
    
    #[error("Invalid Chainlink decimals")]
    InvalidChainlinkDecimals = 6355,
    
    #[error("Invalid Chainlink price")]
    InvalidChainlinkPrice = 6356,
    
    #[error("Chainlink price out of bounds")]
    ChainlinkPriceOutOfBounds = 6357,
    
    #[error("Stale Chainlink price")]
    StaleChainlinkPrice = 6358,
    
    #[error("Chainlink mapping not found")]
    ChainlinkMappingNotFound = 6359,
    
    #[error("Invalid Chainlink feed")]
    InvalidChainlinkFeed = 6360,
    
    #[error("Price feed already exists")]
    PriceFeedAlreadyExists = 6361,
    
    #[error("Unsupported market type")]
    UnsupportedMarketType = 6362,
    
    
    #[error("Unauthorized oracle update")]
    UnauthorizedOracleUpdate = 6371,
    
    #[error("Invalid account size")]
    InvalidAccountSize = 6372,
    
    #[error("Account data too large")]
    AccountDataTooLarge = 6373,
    
    #[error("Compute unit limit exceeded")]
    ComputeUnitLimitExceeded = 6374,
    
    #[error("Operation too complex")]
    OperationTooComplex = 6375,
    
    #[error("Market already sharded")]
    MarketAlreadySharded = 6376,
    
    #[error("Market not sharded")]
    MarketNotSharded = 6377,
    
    // Block trading errors (6410-6419)
    #[error("Invalid trade status")]
    InvalidTradeStatus = 6410,
    
    #[error("Trade expired")]
    TradeExpired = 6411,
    
    #[error("Cannot accept own price")]
    CannotAcceptOwnPrice = 6412,
    
    #[error("Insufficient price improvement")]
    InsufficientPriceImprovement = 6413,
    
    #[error("Trade not agreed")]
    TradeNotAgreed = 6414,
    
    #[error("Trade already executed")]
    TradeAlreadyExecuted = 6415,
    
    #[error("Position mismatch")]
    PositionMismatch = 6416,
    
    #[error("Proposal mismatch")]
    ProposalMismatch = 6417,
    
    #[error("Invalid negotiation window")]
    InvalidNegotiationWindow = 6418,
    
    #[error("Invalid execution window")]
    InvalidExecutionWindow = 6419,
    
    // Market ingestion errors (6378-6399)
    #[error("Too early for ingestion")]
    TooEarly = 6378,
    
    #[error("Market ingestion halted")]
    IngestionHalted = 6379,
    
    #[error("Invalid market data")]
    InvalidMarketData = 6390,
    
    #[error("Request queue full")]
    RequestQueueFull = 6391,
    
    #[error("Rate limit exceeded")]
    RateLimitExceeded = 6392,
    
    #[error("Request not found")]
    RequestNotFound = 6393,
    
    #[error("WebSocket connection failed")]
    WebSocketConnectionFailed = 6394,
    
    #[error("Too many outcomes")]
    TooManyOutcomes = 6395,
    
    #[error("Title too long")]
    TitleTooLong = 6396,
    
    #[error("Missing description")]
    MissingDescription = 6397,
    
    #[error("Too many normalization errors")]
    TooManyNormalizationErrors = 6398,
    
    #[error("Too many subscriptions")]
    TooManySubscriptions = 6399,
    
    #[error("Subscription not found")]
    SubscriptionNotFound = 6600,
    
    #[error("Below target TPS")]
    BelowTargetTPS = 6601,
    
    #[error("All shards unavailable")]
    AllShardsUnavailable = 6602,
    
    #[error("External data required")]
    ExternalDataRequired = 6603,
    
    #[error("Below target performance")]
    BelowTargetPerformance = 6604,
    
    #[error("Payload too large for cross-shard message")]
    TooLargePayload = 6455,
    
    #[error("Emergency halt triggered across shards")]
    EmergencyHaltTriggered = 6456,
    
    // Chain liquidation errors (6395-6399)
    #[error("Chain not active")]
    ChainNotActive = 6605,
    
    #[error("No open positions")]
    NoOpenPositions = 6606,
    
    #[error("No liquidatable positions")]
    NoLiquidatablePositions = 6607,
    
    #[error("Price not found")]
    PriceNotFound = 6608,
    
    #[error("CPI depth exceeded")]
    CPIDepthExceeded = 6609,
    
    // Flash loan errors (6400-6404)
    #[error("Insufficient flash loan repayment")]
    InsufficientFlashLoanRepayment = 6400,
    
    // AMM errors (6405-6409)
    #[error("Invalid outcome count")]
    InvalidOutcomeCount = 6405,
    
    // These are already defined above, removed duplicates
    
    #[error("Polymarket oracle unavailable")]
    PolymarketOracleUnavailable = 6408,
    
    #[error("Stale oracle data")]
    StaleOracleData = 6409,
    
    #[error("Invalid chain ID")]
    InvalidChainId = 6430,
    
    #[error("Invalid matrix index")]
    InvalidMatrixIndex = 6434,
    
    #[error("Market not in matrix")]
    MarketNotInMatrix = 6435,
    
    #[error("Matrix capacity exceeded")]
    MatrixCapacityExceeded = 6436,
    
    // Vesting errors (6610-6619)
    #[error("Vesting schedule inactive")]
    VestingInactive = 6610,
    
    #[error("Nothing to claim")]
    NothingToClaim = 6611,
    
    #[error("Chain cycle detected")]
    ChainCycleDetected = 6612,
    
    #[error("Market capacity exceeded")]
    MarketCapacityExceeded = 6431,
    
    
    #[error("Merkle tree not found")]
    MerkleTreeNotFound = 6432,
    
    #[error("Invalid probabilities")]
    InvalidProbabilities = 6433,
    
    #[error("No valid path")]
    NoValidPath = 6613,
    
    #[error("Invalid market")]
    InvalidMarket = 6614,
    
    #[error("Market halted")]
    MarketHalted = 6615,
    
    #[error("Already initialized")]
    AlreadyInitialized = 6616,
    
    // Credits system errors (6434-6439)
    #[error("Not eligible for refund")]
    NotEligibleForRefund = 6617,
    
    #[error("Active positions exist")]
    ActivePositionsExist = 6451,
    
    #[error("Too early for refund")]
    TooEarlyForRefund = 6452,
    
    #[error("No credits to refund")]
    NoCreditsToRefund = 6453,
    
    #[error("Verse not halted")]
    VerseNotHalted = 6454,
    
    #[error("In grace period")]
    InGracePeriod = 6940,
    
    #[error("Too many oracle sources")]
    TooManyOracleSources = 6941,
    
    #[error("Keeper not active")]
    KeeperNotActive = 6942,
    
    #[error("Stake locked")]
    StakeLocked = 6943,
    
    // MEV Protection errors (6443-6449)
    #[error("Already revealed")]
    AlreadyRevealed = 6443,
    
    #[error("Commitment expired")]
    CommitmentExpired = 6444,
    
    #[error("Invalid reveal")]
    InvalidReveal = 6445,
    
    #[error("Order not revealed")]
    OrderNotRevealed = 6446,
    
    #[error("Already executed")]
    AlreadyExecuted = 6447,
    
    // Security audit errors (6448-6455)
    #[error("Duplicate signature")]
    DuplicateSignature = 6448,
    
    #[error("Insufficient signatures")]
    InsufficientSignatures = 6449,
    
    #[error("PDA collision detected")]
    PDACollision = 6627,
    
    // Dispute evidence errors (6451-6455)
    #[error("Stale evidence")]
    StaleEvidence = 6628,
    
    #[error("Invalid evidence type")]
    InvalidEvidenceType = 6629,
    
    #[error("Invalid evidence order")]
    InvalidEvidenceOrder = 6630,
    
    #[error("Duplicate evidence")]
    DuplicateEvidence = 6631,
    
    /// Position closed
    #[error("Position is already closed")]
    PositionClosed = 6460,
    
    /// Invalid price
    #[error("Invalid price")]
    InvalidPrice = 6461,
    
    /// Internal error
    #[error("Internal error occurred")]
    InternalError = 6462,
    
    /// Proposal not active
    #[error("Proposal is not active")]
    ProposalNotActive = 6463,
    
    /// Invalid PDA
    #[error("Invalid PDA")]
    InvalidPDA = 6464,
    
    /// Oracle not active
    #[error("Oracle is not active")]
    OracleNotActive = 6465,
    
    /// Chain position not found
    #[error("Chain position not found")]
    ChainPositionNotFound = 6466,
    
    /// Stale oracle data
    #[error("Stale oracle data")]
    StaleOracle = 6467,
    
    /// Invalid oracle state
    #[error("Invalid oracle state")]
    InvalidOracleState = 6468,
    
    /// Invalid chain configuration
    #[error("Invalid chain configuration")]
    InvalidChainConfiguration = 6470,
    
    /// Rate limit error
    #[error("Rate limit exceeded")]
    RateLimitError = 6471,
    
    /// Insufficient tokens
    #[error("Insufficient tokens")]
    InsufficientTokens = 6472,
    
    /// Liquidation missed
    #[error("Liquidation missed")]
    LiquidationMissed = 6473,
    
    /// Position limit exceeded
    #[error("Position limit exceeded")]
    PositionLimitExceeded = 6474,
    
    /// AMM invariant violation
    #[error("AMM invariant violation")]
    AMMInvariantViolation = 6475,
    
    /// Keeper not found
    #[error("Keeper not found")]
    KeeperNotFound = 6476,
    
    /// Keeper not staking
    #[error("Keeper not staking")]
    KeeperNotStaking = 6477,
    
    /// No return data from CPI
    #[error("No return data from CPI")]
    NoReturnData = 6478,
    
    /// Invalid return data from CPI
    #[error("Invalid return data from CPI")]
    InvalidReturnData = 6479,
    
    /// Invalid signature
    #[error("Invalid signature")]
    InvalidSignature = 6480,
    
    /// Invalid parameter
    #[error("Invalid parameter")]
    InvalidParameter = 6481,
    
    /// Invalid bounds
    #[error("Invalid bounds")]
    InvalidBounds = 6482,
    
    /// Unauthorized access
    #[error("Unauthorized access")]
    UnauthorizedAccess = 6483,
    
    /// Invalid receipt
    #[error("Invalid receipt")]
    InvalidReceipt = 6484,
    
    /// Flash loan detected
    #[error("Flash loan detected")]
    FlashLoanDetected = 6485,
    
    /// Invalid receipt status
    #[error("Invalid receipt status")]
    InvalidReceiptStatus = 6486,
    
    /// Unauthorized keeper
    #[error("Unauthorized keeper")]
    UnauthorizedKeeper = 6487,
    
    /// Unauthorized governance
    #[error("Unauthorized governance")]
    UnauthorizedGovernance = 6488,
    
    /// Position not open
    #[error("Position not open")]
    PositionNotOpen = 6489,
    
    /// Order size too large
    #[error("Order size too large")]
    OrderSizeTooLarge = 6490,
    
    /// Opportunity expired
    #[error("Opportunity expired")]
    OpportunityExpired = 6491,
    
    /// Market mismatch
    #[error("Market mismatch")]
    MarketMismatch = 6492,
    
    /// Invalid order status
    #[error("Invalid order status")]
    InvalidOrderStatus = 6493,
    
    /// Invalid order size
    #[error("Invalid order size")]
    InvalidOrderSize = 6494,
    
    /// Invalid execution data
    #[error("Invalid execution data")]
    InvalidExecutionData = 6495,
    
    /// Invalid dispute
    #[error("Invalid dispute")]
    InvalidDispute = 6496,
    
    /// Invalid dispute reason
    #[error("Invalid dispute reason")]
    InvalidDisputeReason = 6497,
    
    /// Insufficient profit
    #[error("Insufficient profit")]
    InsufficientProfit = 6498,
    
    /// Excessive price deviation
    #[error("Excessive price deviation")]
    ExcessivePriceDeviation = 6499,
    
    /// Dispute already resolved
    #[error("Dispute already resolved")]
    DisputeAlreadyResolved = 6500,
    
    /// Cancellation window expired
    #[error("Cancellation window expired")]
    CancellationWindowExpired = 6501,
    
    /// Invalid owner
    #[error("Invalid owner")]
    InvalidOwner = 6502,
    
    /// Invalid status
    #[error("Invalid status")]
    InvalidStatus = 6503,
    
    /// Migration timeout
    #[error("Migration timeout")]
    MigrationTimeout = 6504,
    
    /// Migration not active
    #[error("Migration not active")]
    MigrationNotActive = 6563,
    
    /// Migration expired
    #[error("Migration period has expired")]
    MigrationExpired = 6564,
    
    /// Migration not expired
    #[error("Migration period not yet expired")]
    MigrationNotExpired = 6565,
    
    /// Liquidation halted
    #[error("Liquidation halted")]
    LiquidationHalted = 6505,
    
    /// Invalid AMM state
    #[error("Invalid AMM state")]
    InvalidAMMState = 6506,
    
    /// Compute budget exceeded
    #[error("Compute budget exceeded")]
    ComputeBudgetExceeded = 6507,
    
    /// Registry full
    #[error("Registry full")]
    RegistryFull = 6508,
    
    /// Invalid index
    #[error("Invalid index")]
    InvalidIndex = 6509,
    
    /// Empty price series
    #[error("Empty price series")]
    EmptyPriceSeries = 6519,
    
    // Security module errors (6520-6559)
    /// Reentrancy detected
    #[error("Reentrancy detected")]
    ReentrancyDetected = 6520,
    
    /// Guard locked
    #[error("Guard locked")]
    GuardLocked = 6521,
    
    /// Invalid guard state
    #[error("Invalid guard state")]
    InvalidGuardState = 6522,
    
    /// Unauthorized CPI
    #[error("Unauthorized CPI")]
    UnauthorizedCPI = 6523,
    
    
    
    /// Math underflow
    #[error("Math underflow")]
    MathUnderflow = 6526,
    
    
    /// Cast overflow
    #[error("Cast overflow")]
    CastOverflow = 6528,
    
    /// Invalid cast
    #[error("Invalid cast")]
    InvalidCast = 6529,
    
    /// Index out of bounds
    #[error("Index out of bounds")]
    IndexOutOfBounds = 6530,
    
    /// Invalid percentage
    #[error("Invalid percentage")]
    InvalidPercentage = 6531,
    
    /// Below minimum
    #[error("Below minimum")]
    BelowMinimum = 6532,
    
    /// Above maximum
    #[error("Above maximum")]
    AboveMaximum = 6533,
    
    /// At maximum
    #[error("At maximum")]
    AtMaximum = 6534,
    
    /// At minimum
    #[error("At minimum")]
    AtMinimum = 6535,
    
    /// Permission denied
    #[error("Permission denied")]
    PermissionDenied = 6536,
    
    /// Invalid role
    #[error("Invalid role")]
    InvalidRole = 6537,
    
    /// Role expired
    #[error("Role expired")]
    RoleExpired = 6538,
    
    /// Role already assigned
    #[error("Role already assigned")]
    RoleAlreadyAssigned = 6539,
    
    /// Role member limit reached
    #[error("Role member limit reached")]
    RoleMemberLimitReached = 6540,
    
    /// Role not assigned
    #[error("Role not assigned")]
    RoleNotAssigned = 6541,
    
    /// User limit reached
    #[error("User limit reached")]
    UserLimitReached = 6542,
    
    /// User suspended
    #[error("User suspended")]
    UserSuspended = 6543,
    
    /// Global rate limit exceeded
    #[error("Global rate limit exceeded")]
    GlobalRateLimitExceeded = 6544,
    
    /// Circuit breaker open
    #[error("Circuit breaker open")]
    CircuitBreakerOpen = 6545,
    
    /// Unsupported signature type
    #[error("Unsupported signature type")]
    UnsupportedSignatureType = 6546,
    
    /// Signature mismatch
    #[error("Signature mismatch")]
    SignatureMismatch = 6547,
    
    /// Unauthorized signer
    #[error("Unauthorized signer")]
    UnauthorizedSigner = 6548,
    
    /// Duplicate signer
    #[error("Duplicate signer")]
    DuplicateSigner = 6549,
    
    /// Invalid nonce
    #[error("Invalid nonce")]
    InvalidNonce = 6550,
    
    
    /// Insufficient oracle confirmations
    #[error("Insufficient oracle confirmations")]
    InsufficientOracleConfirmations = 6552,
    
    /// Unauthorized oracle
    #[error("Unauthorized oracle")]
    UnauthorizedOracle = 6553,
    
    /// Oracle data mismatch
    #[error("Oracle data mismatch")]
    OracleDataMismatch = 6554,
    
    // Fused leverage oracle errors (6950-6969)
    #[error("Invalid probability value")]
    InvalidProbability = 6950,
    
    #[error("Invalid sigma value")]
    InvalidSigma = 6951,
    
    #[error("Oracle feed halted")]
    OracleHalted = 6952,
    
    #[error("TWAP deviation exceeded")]
    TwapDeviationExceeded = 6953,
    
    #[error("Insufficient oracle consensus")]
    InsufficientConsensus = 6954,
    
    #[error("Early resolution detected")]
    EarlyResolutionDetected = 6955,
    
    #[error("Scalar calculation failed")]
    ScalarCalculationFailed = 6956,
    
    #[error("High volatility detected")]
    HighVolatilityDetected = 6957,
    
    // Synthetic token errors (6960-6979)
    #[error("Invalid token")]
    InvalidToken = 6960,
    
    #[error("Supply exceeded")]
    SupplyExceeded = 6961,
    
    #[error("Minting disabled")]
    MintingDisabled = 6962,
    
    #[error("Account frozen")]
    AccountFrozen = 6963,
    
    #[error("Transfer restricted")]
    TransferRestricted = 6964,
    
    #[error("Unauthorized transfer")]
    UnauthorizedTransfer = 6965,
    
    #[error("Insufficient balance")]
    InsufficientBalance = 6966,
    
    #[error("Position liquidated")]
    PositionLiquidated = 6967,
    
    #[error("Position closed")]
    PositionClosed = 6968,
    
    #[error("Unhealthy position")]
    UnhealthyPosition = 6969,
    
    #[error("Max positions reached")]
    MaxPositionsReached = 6970,
    
    #[error("Position not found")]
    PositionNotFound = 6971,
    
    // CDP errors (6980-6999)
    #[error("Insufficient collateral")]
    InsufficientCollateral = 6980,
    
    #[error("Exceeds max LTV")]
    ExceedsMaxLTV = 6981,
    
    #[error("Exceeds max leverage")]
    ExceedsMaxLeverage = 6982,
    
    #[error("Would be undercollateralized")]
    WouldBeUndercollateralized = 6983,
    
    #[error("Exceeds debt")]
    ExceedsDebt = 6984,
    
    #[error("Not liquidatable")]
    NotLiquidatable = 6985,
    
    #[error("Unauthorized access")]
    UnauthorizedAccess = 6986,
    
    #[error("Vault paused")]
    VaultPaused = 6987,
    
    #[error("Insufficient shares")]
    InsufficientShares = 6988,
    
    #[error("Insufficient liquidity")]
    InsufficientLiquidity = 6989,
    
    #[error("Exceeds mint limit")]
    ExceedsMintLimit = 6990,
    
    #[error("Daily limit exceeded")]
    DailyLimitExceeded = 6991,
    
    #[error("Cooldown active")]
    CooldownActive = 6992,
    
    #[error("Burning disabled")]
    BurningDisabled = 6993,
    
    #[error("Below minimum")]
    BelowMinimum = 6994,
    
    #[error("Exceeds borrow limit")]
    ExceedsBorrowLimit = 6995,
    
    #[error("Interest rate too high")]
    InterestRateTooHigh = 6996,
    
    #[error("Auction not active")]
    AuctionNotActive = 6997,
    
    #[error("Auction ended")]
    AuctionEnded = 6998,
    
    #[error("Bid below reserve")]
    BidBelowReserve = 6999,
    
    #[error("Bid too low")]
    BidTooLow = 7000,
    
    #[error("Auction still active")]
    AuctionStillActive = 7001,
    
    #[error("Insufficient repayment")]
    InsufficientRepayment = 7002,
    
    #[error("Auction not complete")]
    AuctionNotComplete = 7003,
    
    /// Invariant violation
    #[error("Invariant violation")]
    InvariantViolation = 6556,
    
    /// Protocol paused
    #[error("Protocol paused")]
    ProtocolPaused = 6557,
    
    /// Protocol frozen
    #[error("Protocol frozen")]
    ProtocolFrozen = 6558,
    
    /// Program not immutable
    #[error("Program not immutable")]
    NotImmutable = 6560,
    
    /// Governance not allowed
    #[error("Governance not allowed in immutable program")]
    GovernanceNotAllowed = 6561,
    
    /// Parameter mismatch
    #[error("Parameter mismatch - values differ from constants")]
    ParameterMismatch = 6562,
    
    // Risk quiz errors (6700-6709)
    /// Risk quiz required
    #[error("Risk quiz required for leverage > 10x")]
    RiskQuizRequired = 6700,
    
    /// Quiz already passed
    #[error("Quiz already passed")]
    QuizAlreadyPassed = 6701,
    
    /// Invalid risk hash
    #[error("Invalid risk disclosure hash")]
    InvalidRiskHash = 6702,
    
    /// Risk not acknowledged
    #[error("Risk disclosure not acknowledged")]
    RiskNotAcknowledged = 6703,
    
    /// Quiz cooldown active
    #[error("Quiz cooldown active - wait before retrying")]
    QuizCooldownActive = 6704,
    
    /// Quiz attempts exceeded
    #[error("Maximum quiz attempts exceeded")]
    QuizAttemptsExceeded = 6705,
    
    /// Invalid quiz answers
    #[error("Invalid quiz answers")]
    InvalidQuizAnswers = 6706,
    
    // Error handling and recovery errors (6800-6899)
    
    /// Recovery already active
    #[error("Recovery already active")]
    RecoveryAlreadyActive = 6800,
    
    /// Recovery not found
    #[error("Recovery not found")]
    RecoveryNotFound = 6801,
    
    /// Recovery type disabled
    #[error("Recovery type disabled")]
    RecoveryTypeDisabled = 6802,
    
    /// Max recovery attempts exceeded
    #[error("Max recovery attempts exceeded")]
    MaxRecoveryAttemptsExceeded = 6803,
    
    /// Too many pending transactions
    #[error("Too many pending transactions")]
    TooManyPendingTransactions = 6804,
    
    /// Undo window expired
    #[error("Undo window expired")]
    UndoWindowExpired = 6805,
    
    /// Undo window not expired
    #[error("Undo window not expired")]
    UndoWindowNotExpired = 6806,
    
    /// Transaction not cancellable
    #[error("Transaction not cancellable")]
    TransactionNotCancellable = 6807,
    
    /// Transaction cancelled
    #[error("Transaction cancelled")]
    TransactionCancelled = 6808,
    
    /// Transaction already executed
    #[error("Transaction already executed")]
    TransactionAlreadyExecuted = 6809,
    
    /// Invalid transaction status
    #[error("Invalid transaction status")]
    InvalidTransactionStatus = 6810,
    
    /// Too many revertible actions
    #[error("Too many revertible actions")]
    TooManyRevertibleActions = 6811,
    
    /// Action already reverted
    #[error("Action already reverted")]
    ActionAlreadyReverted = 6812,
    
    /// Action not found
    #[error("Action not found")]
    ActionNotFound = 6813,
    
    /// Revert window expired
    #[error("Revert window expired")]
    RevertWindowExpired = 6814,
    
    // Tour errors (6900-6909)
    #[error("Tour already in progress")]
    TourInProgress = 6900,
    
    #[error("No active tour")]
    NoActiveTour = 6901,
    
    // Migration errors (6910-6919)
    #[error("Migration already completed")]
    MigrationCompleted = 6910,
    
    // Airdrop errors (6920-6929)
    #[error("Airdrop not active")]
    AirdropNotActive = 6920,
    
    #[error("Airdrop cap reached")]
    AirdropCapReached = 6921,
    
    #[error("Insufficient followers")]
    InsufficientFollowers = 6922,
    
    #[error("Already registered")]
    AlreadyRegistered = 6923,
    
    #[error("Already claimed")]
    AlreadyClaimed = 6924,
    
    #[error("Claim not started")]
    ClaimNotStarted = 6925,
    
    #[error("Claim period ended")]
    ClaimPeriodEnded = 6926,
    
    #[error("Invalid influencer")]
    InvalidInfluencer = 6927,
    
    #[error("Invalid token account")]
    InvalidTokenAccount = 6928,
    
    // Halt mechanism errors (6930-6939)
    #[error("Already halted")]
    AlreadyHalted = 6930,
    
    #[error("Not halted")]
    NotHalted = 6931,
    
    #[error("Unauthorized halt operation")]
    UnauthorizedHaltOperation = 6932,
}

impl PrintProgramError for BettingPlatformError {
    fn print<E>(&self) {
        msg!("Betting Platform Error: {}", self);
    }
}

impl From<BettingPlatformError> for ProgramError {
    fn from(e: BettingPlatformError) -> Self {
        ProgramError::Custom(e as u32)
    }
}

impl<T> DecodeError<T> for BettingPlatformError {
    fn type_of() -> &'static str {
        "BettingPlatformError"
    }
}

/// Helper function to log and return errors
pub fn error_msg<T>(error: BettingPlatformError, message: &str) -> Result<T, ProgramError> {
    msg!("Error: {} - {}", error, message);
    Err(error.into())
}
impl TryFrom<ProgramError> for BettingPlatformError {
    type Error = ProgramError;
    
    fn try_from(error: ProgramError) -> Result<Self, Self::Error> {
        match error {
            ProgramError::Custom(code) => {
                // Map custom error codes to BettingPlatformError variants
                match code {
                    6001 => Ok(BettingPlatformError::InvalidAccountData),
                    6002 => Ok(BettingPlatformError::SystemHalted),
                    _ => Err(error),
                }
            }
            _ => Err(error),
        }
    }
}
