/**
 * Type-safe number handling utilities for large integers
 * Handles u64 and u128 values from Rust backend safely
 */

class SafeNumbers {
    /**
     * Parse a string or number to BigInt safely
     * @param {string|number|bigint} value - The value to parse
     * @returns {bigint} The parsed BigInt value
     */
    static parseBigInt(value) {
        if (typeof value === 'bigint') {
            return value;
        }
        if (typeof value === 'string') {
            return BigInt(value);
        }
        if (typeof value === 'number') {
            if (!Number.isSafeInteger(value)) {
                console.warn('Number exceeds safe integer range:', value);
            }
            return BigInt(Math.floor(value));
        }
        throw new Error(`Cannot parse BigInt from ${typeof value}: ${value}`);
    }

    /**
     * Convert BigInt to string for display
     * @param {bigint} value - The BigInt value
     * @returns {string} String representation
     */
    static toString(value) {
        return value.toString();
    }

    /**
     * Format a BigInt amount with decimals (for SOL, USDC, etc)
     * @param {bigint|string} amount - The amount in smallest units
     * @param {number} decimals - Number of decimal places (9 for SOL, 6 for USDC)
     * @returns {string} Formatted amount
     */
    static formatAmount(amount, decimals = 9) {
        const value = this.parseBigInt(amount);
        const divisor = BigInt(10 ** decimals);
        const whole = value / divisor;
        const remainder = value % divisor;
        
        if (remainder === 0n) {
            return whole.toString();
        }
        
        const remainderStr = remainder.toString().padStart(decimals, '0');
        const trimmed = remainderStr.replace(/0+$/, '');
        return `${whole}.${trimmed}`;
    }

    /**
     * Parse a decimal amount to smallest units
     * @param {string|number} amount - The decimal amount
     * @param {number} decimals - Number of decimal places
     * @returns {bigint} Amount in smallest units
     */
    static parseAmount(amount, decimals = 9) {
        const amountStr = amount.toString();
        const parts = amountStr.split('.');
        const whole = BigInt(parts[0] || '0');
        const fractional = parts[1] || '';
        
        const paddedFractional = fractional.padEnd(decimals, '0').slice(0, decimals);
        const fractionalBigInt = BigInt(paddedFractional);
        
        return whole * BigInt(10 ** decimals) + fractionalBigInt;
    }

    /**
     * Compare two BigInt values
     * @param {bigint|string} a - First value
     * @param {bigint|string} b - Second value
     * @returns {number} -1 if a < b, 0 if a == b, 1 if a > b
     */
    static compare(a, b) {
        const aBig = this.parseBigInt(a);
        const bBig = this.parseBigInt(b);
        
        if (aBig < bBig) return -1;
        if (aBig > bBig) return 1;
        return 0;
    }

    /**
     * Check if a value is zero
     * @param {bigint|string} value - The value to check
     * @returns {boolean} True if zero
     */
    static isZero(value) {
        return this.parseBigInt(value) === 0n;
    }

    /**
     * Calculate percentage of two BigInt values
     * @param {bigint|string} value - The value
     * @param {bigint|string} total - The total
     * @param {number} precision - Decimal places for percentage
     * @returns {string} Percentage as string
     */
    static percentage(value, total, precision = 2) {
        const valueBig = this.parseBigInt(value);
        const totalBig = this.parseBigInt(total);
        
        if (totalBig === 0n) return '0';
        
        const factor = BigInt(10 ** (precision + 2));
        const percentage = (valueBig * factor) / totalBig;
        const percentageStr = percentage.toString();
        
        if (percentageStr.length <= 2) {
            return '0.' + percentageStr.padStart(2, '0');
        }
        
        const whole = percentageStr.slice(0, -2);
        const decimal = percentageStr.slice(-2);
        return `${whole}.${decimal}`;
    }

    /**
     * Validate that a string can be parsed as a valid BigInt
     * @param {string} value - The value to validate
     * @returns {boolean} True if valid
     */
    static isValidBigInt(value) {
        try {
            BigInt(value);
            return true;
        } catch {
            return false;
        }
    }

    /**
     * Create type-safe market ID from string
     * @param {string} id - Market ID as string
     * @returns {object} Object with string and bigint representations
     */
    static createMarketId(id) {
        return {
            string: id,
            bigint: this.parseBigInt(id),
            toString() { return this.string; },
            toBigInt() { return this.bigint; }
        };
    }
}

// Export for use in other scripts
if (typeof module !== 'undefined' && module.exports) {
    module.exports = SafeNumbers;
} else {
    window.SafeNumbers = SafeNumbers;
}