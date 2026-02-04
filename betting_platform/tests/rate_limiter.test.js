"use strict";
var __awaiter = (this && this.__awaiter) || function (thisArg, _arguments, P, generator) {
    function adopt(value) { return value instanceof P ? value : new P(function (resolve) { resolve(value); }); }
    return new (P || (P = Promise))(function (resolve, reject) {
        function fulfilled(value) { try { step(generator.next(value)); } catch (e) { reject(e); } }
        function rejected(value) { try { step(generator["throw"](value)); } catch (e) { reject(e); } }
        function step(result) { result.done ? resolve(result.value) : adopt(result.value).then(fulfilled, rejected); }
        step((generator = generator.apply(thisArg, _arguments || [])).next());
    });
};
Object.defineProperty(exports, "__esModule", { value: true });
const globals_1 = require("@jest/globals");
const rate_limiter_1 = require("../src/rate_limiter");
(0, globals_1.describe)('TokenBucket', () => {
    let bucket;
    (0, globals_1.beforeEach)(() => {
        bucket = new rate_limiter_1.TokenBucket({
            rate: 10,
            per: 1000, // 10 tokens per second
            burst: 5,
        });
    });
    (0, globals_1.it)('should allow burst requests', () => {
        // Should be able to consume all burst tokens immediately
        for (let i = 0; i < 5; i++) {
            (0, globals_1.expect)(bucket.tryConsume(1)).toBe(true);
        }
        // 6th request should fail
        (0, globals_1.expect)(bucket.tryConsume(1)).toBe(false);
    });
    (0, globals_1.it)('should refill tokens over time', () => __awaiter(void 0, void 0, void 0, function* () {
        // Consume all tokens
        for (let i = 0; i < 5; i++) {
            bucket.tryConsume(1);
        }
        // Wait for refill
        yield new Promise(resolve => setTimeout(resolve, 200)); // 0.2 seconds = 2 tokens
        // Should be able to consume 2 tokens
        (0, globals_1.expect)(bucket.tryConsume(1)).toBe(true);
        (0, globals_1.expect)(bucket.tryConsume(1)).toBe(true);
        (0, globals_1.expect)(bucket.tryConsume(1)).toBe(false);
    }));
    (0, globals_1.it)('should wait for tokens when needed', () => __awaiter(void 0, void 0, void 0, function* () {
        // Consume all tokens
        for (let i = 0; i < 5; i++) {
            bucket.tryConsume(1);
        }
        const startTime = Date.now();
        yield bucket.waitForTokens(2);
        const elapsed = Date.now() - startTime;
        // Should wait approximately 200ms for 2 tokens (10 tokens/sec)
        (0, globals_1.expect)(elapsed).toBeGreaterThanOrEqual(150);
        (0, globals_1.expect)(elapsed).toBeLessThan(250);
    }));
    (0, globals_1.it)('should not exceed burst capacity', () => __awaiter(void 0, void 0, void 0, function* () {
        // Wait for full refill
        yield new Promise(resolve => setTimeout(resolve, 1000));
        // Available tokens should not exceed burst
        (0, globals_1.expect)(bucket.getAvailableTokens()).toBeLessThanOrEqual(5);
    }));
});
(0, globals_1.describe)('TieredRateLimiter', () => {
    let limiter;
    (0, globals_1.beforeEach)(() => {
        limiter = new rate_limiter_1.TieredRateLimiter('free');
    });
    (0, globals_1.afterEach)(() => {
        // Clean up any pending requests
        limiter.removeAllListeners();
    });
    (0, globals_1.it)('should respect rate limits', () => __awaiter(void 0, void 0, void 0, function* () {
        const requests = [];
        // Try to make 60 requests (should hit limit at 50)
        for (let i = 0; i < 60; i++) {
            requests.push(limiter.executeRequest('/markets', () => __awaiter(void 0, void 0, void 0, function* () { return ({ id: i }); }), 5));
        }
        const startTime = Date.now();
        const results = yield Promise.all(requests);
        const duration = Date.now() - startTime;
        // Should take > 10 seconds due to rate limiting (50 per 10s)
        (0, globals_1.expect)(duration).toBeGreaterThan(10000);
        (0, globals_1.expect)(results).toHaveLength(60);
    }));
    (0, globals_1.it)('should handle bursts correctly', () => __awaiter(void 0, void 0, void 0, function* () {
        const requests = [];
        // Burst of 10 should be immediate (free tier burst = 10)
        for (let i = 0; i < 10; i++) {
            requests.push(limiter.executeRequest('/markets', () => __awaiter(void 0, void 0, void 0, function* () { return ({ id: i }); }), 10));
        }
        const startTime = Date.now();
        yield Promise.all(requests);
        const duration = Date.now() - startTime;
        // Burst should complete quickly
        (0, globals_1.expect)(duration).toBeLessThan(1000);
    }));
    (0, globals_1.it)('should prioritize high priority requests', () => __awaiter(void 0, void 0, void 0, function* () {
        const results = [];
        const requests = [];
        // Queue multiple requests with different priorities
        for (let i = 0; i < 5; i++) {
            requests.push(limiter.executeRequest('/markets', () => __awaiter(void 0, void 0, void 0, function* () {
                results.push(i);
                return { id: i };
            }), i // Priority = i
            ));
        }
        // Add high priority request after others
        requests.push(limiter.executeRequest('/markets', () => __awaiter(void 0, void 0, void 0, function* () {
            results.push(99);
            return { id: 99 };
        }), rate_limiter_1.TieredRateLimiter.HIGH_PRIORITY));
        yield Promise.all(requests);
        // High priority request (99) should be processed early
        const highPriorityIndex = results.indexOf(99);
        (0, globals_1.expect)(highPriorityIndex).toBeLessThan(3);
    }));
    (0, globals_1.it)('should handle different tier limits', () => {
        const freeLimiter = new rate_limiter_1.TieredRateLimiter('free');
        const basicLimiter = new rate_limiter_1.TieredRateLimiter('basic');
        const premiumLimiter = new rate_limiter_1.TieredRateLimiter('premium');
        const freeLimits = freeLimiter.getTierLimits();
        const basicLimits = basicLimiter.getTierLimits();
        const premiumLimits = premiumLimiter.getTierLimits();
        // Verify tier differences
        (0, globals_1.expect)(freeLimits.markets.rate).toBe(50);
        (0, globals_1.expect)(basicLimits.markets.rate).toBe(500);
        (0, globals_1.expect)(premiumLimits.markets.rate).toBe(2000);
    });
    (0, globals_1.it)('should retry on 429 errors', () => __awaiter(void 0, void 0, void 0, function* () {
        let attempts = 0;
        const result = yield limiter.executeRequest('/markets', () => __awaiter(void 0, void 0, void 0, function* () {
            attempts++;
            if (attempts < 3) {
                const error = new Error('Rate limited');
                error.response = { status: 429 };
                throw error;
            }
            return { success: true };
        }), 5);
        (0, globals_1.expect)(attempts).toBe(3);
        (0, globals_1.expect)(result).toEqual({ success: true });
    }));
    (0, globals_1.it)('should handle emergency mode', () => __awaiter(void 0, void 0, void 0, function* () {
        const status1 = limiter.getStatus();
        limiter.enableEmergencyMode();
        const status2 = limiter.getStatus();
        // Emergency mode should reduce limits
        (0, globals_1.expect)(status2.buckets.markets.capacity).toBe(Math.floor(status1.buckets.markets.capacity / 2));
    }));
    (0, globals_1.it)('should track request metrics', () => __awaiter(void 0, void 0, void 0, function* () {
        // Make some successful requests
        for (let i = 0; i < 5; i++) {
            yield limiter.executeRequest('/markets', () => __awaiter(void 0, void 0, void 0, function* () { return ({ id: i }); }), 5);
        }
        // Make a failing request
        try {
            yield limiter.executeRequest('/markets', () => __awaiter(void 0, void 0, void 0, function* () {
                throw new Error('Test error');
            }), 5);
        }
        catch (e) {
            // Expected
        }
        const status = limiter.getStatus();
        (0, globals_1.expect)(status.recentRequests).toBeGreaterThanOrEqual(6);
        (0, globals_1.expect)(status.failureRate).toBeGreaterThan(0);
        (0, globals_1.expect)(status.failureRate).toBeLessThan(0.2);
    }));
    (0, globals_1.it)('should execute batch requests', () => __awaiter(void 0, void 0, void 0, function* () {
        const requests = [
            { endpoint: '/markets', execute: () => __awaiter(void 0, void 0, void 0, function* () { return ({ id: 1 }); }), priority: 5 },
            { endpoint: '/markets', execute: () => __awaiter(void 0, void 0, void 0, function* () { return ({ id: 2 }); }), priority: 10 },
            { endpoint: '/orders', execute: () => __awaiter(void 0, void 0, void 0, function* () { return ({ id: 3 }); }), priority: 5 },
        ];
        const results = yield limiter.executeBatch(requests);
        (0, globals_1.expect)(results).toHaveLength(3);
        (0, globals_1.expect)(results[0]).toEqual({ id: 1 });
        (0, globals_1.expect)(results[1]).toEqual({ id: 2 });
        (0, globals_1.expect)(results[2]).toEqual({ id: 3 });
    }));
    (0, globals_1.it)('should handle adaptive backoff', () => __awaiter(void 0, void 0, void 0, function* () {
        // Simulate multiple failures
        for (let i = 0; i < 5; i++) {
            try {
                yield limiter.executeRequest('/markets', () => __awaiter(void 0, void 0, void 0, function* () {
                    throw new Error('Test failure');
                }), 5);
            }
            catch (e) {
                // Expected
            }
        }
        const backoff = limiter.getAdaptiveBackoff('/markets');
        // High failure rate should result in longer backoff
        (0, globals_1.expect)(backoff).toBeGreaterThan(2000);
    }));
});
(0, globals_1.describe)('PriorityQueue', () => {
    (0, globals_1.it)('should dequeue items by priority', () => {
        var _a, _b, _c;
        const queue = new rate_limiter_1.PriorityQueue((a, b) => b.priority - a.priority);
        queue.enqueue({ value: 1, priority: 5 });
        queue.enqueue({ value: 2, priority: 10 });
        queue.enqueue({ value: 3, priority: 3 });
        (0, globals_1.expect)((_a = queue.dequeue()) === null || _a === void 0 ? void 0 : _a.value).toBe(2); // Highest priority
        (0, globals_1.expect)((_b = queue.dequeue()) === null || _b === void 0 ? void 0 : _b.value).toBe(1);
        (0, globals_1.expect)((_c = queue.dequeue()) === null || _c === void 0 ? void 0 : _c.value).toBe(3); // Lowest priority
    });
    (0, globals_1.it)('should handle empty queue', () => {
        const queue = new rate_limiter_1.PriorityQueue((a, b) => b - a);
        (0, globals_1.expect)(queue.isEmpty()).toBe(true);
        (0, globals_1.expect)(queue.dequeue()).toBeUndefined();
        (0, globals_1.expect)(queue.peek()).toBeUndefined();
    });
    (0, globals_1.it)('should maintain correct size', () => {
        const queue = new rate_limiter_1.PriorityQueue((a, b) => a - b);
        (0, globals_1.expect)(queue.size()).toBe(0);
        queue.enqueue(1);
        queue.enqueue(2);
        queue.enqueue(3);
        (0, globals_1.expect)(queue.size()).toBe(3);
        queue.dequeue();
        (0, globals_1.expect)(queue.size()).toBe(2);
    });
});
(0, globals_1.describe)('Rate Limit Compliance', () => {
    (0, globals_1.it)('should not exceed configured limits over time windows', () => __awaiter(void 0, void 0, void 0, function* () {
        const limiter = new rate_limiter_1.TieredRateLimiter('free');
        const startTime = Date.now();
        const requestLog = [];
        // Make continuous requests for 30 seconds
        const testDuration = 30000;
        const requests = [];
        while (Date.now() - startTime < testDuration) {
            requests.push(limiter.executeRequest('/markets', () => __awaiter(void 0, void 0, void 0, function* () {
                requestLog.push(Date.now() - startTime);
                return { timestamp: Date.now() };
            }), 5));
            // Small delay to not overwhelm
            yield new Promise(resolve => setTimeout(resolve, 50));
        }
        yield Promise.all(requests);
        // Analyze compliance - should not exceed 50 requests per 10 seconds
        for (let window = 0; window < testDuration - 10000; window += 1000) {
            const windowEnd = window + 10000;
            const requestsInWindow = requestLog.filter(t => t >= window && t < windowEnd).length;
            (0, globals_1.expect)(requestsInWindow).toBeLessThanOrEqual(50);
        }
    }));
});
