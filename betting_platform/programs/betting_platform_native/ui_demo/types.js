/**
 * Type definitions and validators for API responses
 * Ensures type safety between Rust backend and JavaScript frontend
 */

// Verse type definition
const VerseType = {
    id: 'string',
    name: 'string',
    description: 'string',
    level: 'number',
    multiplier: 'number',
    category: 'string',
    risk_tier: 'string',
    parent_id: 'string?',
    market_count: 'number'
};

// Market type definition - flexible for both our format and Polymarket
const MarketType = {
    pubkey: 'string?',
    id: 'string?',  // u128 from Rust, handled as string
    title: 'string?',  // Either title or question must exist
    description: 'string?',
    question: 'string?',  // Polymarket uses question instead of title
    creator: 'string?',
    outcomes: 'array?',  // Can be array or tokens array
    tokens: 'array?',    // Polymarket format
    total_liquidity: 'string?',  // u64 as string
    total_volume: 'string?',     // u64 as string
    resolution_time: 'number?',
    resolved: 'boolean?',
    winning_outcome: 'number?',
    verses: 'array?',
    category: 'string?',
    // Polymarket specific fields
    volume: 'string?',
    volume24hr: 'number?',
    liquidity: 'string?',
    outcomePrices: 'array?',
    condition_id: 'string?',
    question_id: 'string?',
    market_slug: 'string?'
};

// Position type definition
const PositionType = {
    pubkey: 'string',
    market_id: 'string',    // u128 as string
    trader: 'string',
    size: 'string',         // u64 as string
    side: 'string',
    entry_price: 'string',  // u64 as string
    liquidation_price: 'string', // u64 as string
    margin_used: 'number',
    collateral: 'string',   // u64 as string
    pnl: 'number',
    closed: 'boolean'
};

// Trade request type
const TradeRequestType = {
    market_id: 'string',    // u128 as string
    amount: 'string',       // u64 as string
    leverage: 'number',
    side: 'string',
    wallet: 'string'
};

// Type validator
class TypeValidator {
    /**
     * Validate an object against a type definition
     * @param {object} obj - Object to validate
     * @param {object} typeDef - Type definition
     * @returns {object} Validation result with isValid and errors
     */
    static validate(obj, typeDef) {
        const errors = [];
        
        for (const [key, expectedType] of Object.entries(typeDef)) {
            const value = obj[key];
            const isOptional = expectedType.endsWith('?');
            const type = isOptional ? expectedType.slice(0, -1) : expectedType;
            
            if (value === undefined || value === null) {
                if (!isOptional) {
                    errors.push(`Missing required field: ${key}`);
                }
                continue;
            }
            
            if (!this.checkType(value, type)) {
                errors.push(`Invalid type for ${key}: expected ${type}, got ${typeof value}`);
            }
        }
        
        return {
            isValid: errors.length === 0,
            errors
        };
    }
    
    /**
     * Check if a value matches expected type
     * @param {any} value - Value to check
     * @param {string} type - Expected type
     * @returns {boolean} True if type matches
     */
    static checkType(value, type) {
        switch (type) {
            case 'string':
                return typeof value === 'string';
            case 'number':
                return typeof value === 'number' && !isNaN(value);
            case 'boolean':
                return typeof value === 'boolean';
            case 'array':
                return Array.isArray(value);
            case 'object':
                return typeof value === 'object' && value !== null && !Array.isArray(value);
            default:
                return true;
        }
    }
    
    /**
     * Validate Polymarket-specific market format
     * @param {object} data - Raw market data
     * @returns {object} Validation result
     */
    static validatePolymarket(data) {
        const errors = [];
        
        // Must have question or title
        if (!data.question && !data.title) {
            errors.push('Missing title/question');
        }
        
        // Must have some form of outcomes
        if (!data.outcomes && !data.tokens) {
            errors.push('Missing outcomes/tokens');
        }
        
        // Generate ID from condition_id or question_id if missing
        if (!data.id && !data.condition_id && !data.question_id) {
            errors.push('Missing identifier');
        }
        
        return {
            isValid: errors.length === 0,
            errors
        };
    }
    
    /**
     * Create a validated market object
     * @param {object} data - Raw market data
     * @returns {object|null} Validated market or null if invalid
     */
    static createMarket(data) {
        // For Polymarket data, use special validation
        if (data.condition_id || data.question_id || data.tokens) {
            const polyValidation = this.validatePolymarket(data);
            if (!polyValidation.isValid) {
                console.error('Invalid Polymarket data:', polyValidation.errors);
                return null;
            }
            
            // Normalize Polymarket data
            return {
                ...data,
                id: data.id || data.condition_id || data.question_id || 'unknown',
                title: data.title || data.question || 'Unknown Market',
                outcomes: data.outcomes || (data.tokens ? data.tokens.map(t => t.outcome) : ['Yes', 'No']),
                total_liquidity: '0',
                total_volume: '0'
            };
        }
        
        // Standard validation
        const validation = this.validate(data, MarketType);
        if (!validation.isValid) {
            console.error('Invalid market data:', validation.errors);
            return null;
        }
        
        // Ensure numeric string fields are valid
        if (data.id && !SafeNumbers.isValidBigInt(data.id)) {
            console.error('Invalid market ID:', data.id);
            return null;
        }
        
        return {
            ...data,
            id: SafeNumbers.createMarketId(data.id),
            total_liquidity: data.total_liquidity ? SafeNumbers.parseBigInt(data.total_liquidity) : 0n,
            total_volume: data.total_volume ? SafeNumbers.parseBigInt(data.total_volume) : 0n
        };
    }
    
    /**
     * Create a validated position object
     * @param {object} data - Raw position data
     * @returns {object|null} Validated position or null if invalid
     */
    static createPosition(data) {
        const validation = this.validate(data, PositionType);
        if (!validation.isValid) {
            console.error('Invalid position data:', validation.errors);
            return null;
        }
        
        return {
            ...data,
            market_id: SafeNumbers.createMarketId(data.market_id),
            size: SafeNumbers.parseBigInt(data.size),
            entry_price: SafeNumbers.parseBigInt(data.entry_price),
            liquidation_price: SafeNumbers.parseBigInt(data.liquidation_price),
            collateral: SafeNumbers.parseBigInt(data.collateral)
        };
    }
    
    /**
     * Create a validated trade request
     * @param {object} data - Trade request data
     * @returns {object|null} Validated request or null if invalid
     */
    static createTradeRequest(data) {
        const validation = this.validate(data, TradeRequestType);
        if (!validation.isValid) {
            console.error('Invalid trade request:', validation.errors);
            return null;
        }
        
        // Ensure leverage is within valid range
        if (data.leverage < 1 || data.leverage > 100) {
            console.error('Invalid leverage:', data.leverage);
            return null;
        }
        
        // Ensure side is valid
        if (!['long', 'short'].includes(data.side.toLowerCase())) {
            console.error('Invalid side:', data.side);
            return null;
        }
        
        return {
            market_id: data.market_id,
            amount: data.amount,
            leverage: data.leverage,
            side: data.side.toLowerCase(),
            wallet: data.wallet
        };
    }
}

// Export for use
if (typeof module !== 'undefined' && module.exports) {
    module.exports = { TypeValidator, MarketType, PositionType, TradeRequestType, VerseType };
} else {
    window.TypeValidator = TypeValidator;
    window.APITypes = { MarketType, PositionType, TradeRequestType, VerseType };
}