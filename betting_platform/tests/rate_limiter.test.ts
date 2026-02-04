import { describe, it, expect, beforeEach, afterEach } from '@jest/globals';
import { TieredRateLimiter, TokenBucket, PriorityQueue } from '../src/rate_limiter';

describe('TokenBucket', () => {
    let bucket: TokenBucket;

    beforeEach(() => {
        bucket = new TokenBucket({
            rate: 10,
            per: 1000, // 10 tokens per second
            burst: 5,
        });
    });

    it('should allow burst requests', () => {
        // Should be able to consume all burst tokens immediately
        for (let i = 0; i < 5; i++) {
            expect(bucket.tryConsume(1)).toBe(true);
        }
        
        // 6th request should fail
        expect(bucket.tryConsume(1)).toBe(false);
    });

    it('should refill tokens over time', async () => {
        // Consume all tokens
        for (let i = 0; i < 5; i++) {
            bucket.tryConsume(1);
        }
        
        // Wait for refill
        await new Promise(resolve => setTimeout(resolve, 200)); // 0.2 seconds = 2 tokens
        
        // Should be able to consume 2 tokens
        expect(bucket.tryConsume(1)).toBe(true);
        expect(bucket.tryConsume(1)).toBe(true);
        expect(bucket.tryConsume(1)).toBe(false);
    });

    it('should wait for tokens when needed', async () => {
        // Consume all tokens
        for (let i = 0; i < 5; i++) {
            bucket.tryConsume(1);
        }
        
        const startTime = Date.now();
        await bucket.waitForTokens(2);
        const elapsed = Date.now() - startTime;
        
        // Should wait approximately 200ms for 2 tokens (10 tokens/sec)
        expect(elapsed).toBeGreaterThanOrEqual(150);
        expect(elapsed).toBeLessThan(250);
    });

    it('should not exceed burst capacity', async () => {
        // Wait for full refill
        await new Promise(resolve => setTimeout(resolve, 1000));
        
        // Available tokens should not exceed burst
        expect(bucket.getAvailableTokens()).toBeLessThanOrEqual(5);
    });
});

describe('TieredRateLimiter', () => {
    let limiter: TieredRateLimiter;

    beforeEach(() => {
        limiter = new TieredRateLimiter('free');
    });

    afterEach(() => {
        // Clean up any pending requests
        limiter.removeAllListeners();
    });

    it('should respect rate limits', async () => {
        const requests = [];
        
        // Try to make 60 requests (should hit limit at 50)
        for (let i = 0; i < 60; i++) {
            requests.push(
                limiter.executeRequest(
                    '/markets',
                    async () => ({ id: i }),
                    5
                )
            );
        }
        
        const startTime = Date.now();
        const results = await Promise.all(requests);
        const duration = Date.now() - startTime;
        
        // Should take > 10 seconds due to rate limiting (50 per 10s)
        expect(duration).toBeGreaterThan(10000);
        expect(results).toHaveLength(60);
    });

    it('should handle bursts correctly', async () => {
        const requests = [];
        
        // Burst of 10 should be immediate (free tier burst = 10)
        for (let i = 0; i < 10; i++) {
            requests.push(
                limiter.executeRequest(
                    '/markets',
                    async () => ({ id: i }),
                    10
                )
            );
        }
        
        const startTime = Date.now();
        await Promise.all(requests);
        const duration = Date.now() - startTime;
        
        // Burst should complete quickly
        expect(duration).toBeLessThan(1000);
    });

    it('should prioritize high priority requests', async () => {
        const results: number[] = [];
        const requests = [];
        
        // Queue multiple requests with different priorities
        for (let i = 0; i < 5; i++) {
            requests.push(
                limiter.executeRequest(
                    '/markets',
                    async () => {
                        results.push(i);
                        return { id: i };
                    },
                    i // Priority = i
                )
            );
        }
        
        // Add high priority request after others
        requests.push(
            limiter.executeRequest(
                '/markets',
                async () => {
                    results.push(99);
                    return { id: 99 };
                },
                TieredRateLimiter.HIGH_PRIORITY
            )
        );
        
        await Promise.all(requests);
        
        // High priority request (99) should be processed early
        const highPriorityIndex = results.indexOf(99);
        expect(highPriorityIndex).toBeLessThan(3);
    });

    it('should handle different tier limits', () => {
        const freeLimiter = new TieredRateLimiter('free');
        const basicLimiter = new TieredRateLimiter('basic');
        const premiumLimiter = new TieredRateLimiter('premium');
        
        const freeLimits = freeLimiter.getTierLimits();
        const basicLimits = basicLimiter.getTierLimits();
        const premiumLimits = premiumLimiter.getTierLimits();
        
        // Verify tier differences
        expect(freeLimits.markets.rate).toBe(50);
        expect(basicLimits.markets.rate).toBe(500);
        expect(premiumLimits.markets.rate).toBe(2000);
    });

    it('should retry on 429 errors', async () => {
        let attempts = 0;
        
        const result = await limiter.executeRequest(
            '/markets',
            async () => {
                attempts++;
                if (attempts < 3) {
                    const error: any = new Error('Rate limited');
                    error.response = { status: 429 };
                    throw error;
                }
                return { success: true };
            },
            5
        );
        
        expect(attempts).toBe(3);
        expect(result).toEqual({ success: true });
    });

    it('should handle emergency mode', async () => {
        const status1 = limiter.getStatus();
        
        limiter.enableEmergencyMode();
        
        const status2 = limiter.getStatus();
        
        // Emergency mode should reduce limits
        expect(status2.buckets.markets.capacity).toBe(
            Math.floor(status1.buckets.markets.capacity / 2)
        );
    });

    it('should track request metrics', async () => {
        // Make some successful requests
        for (let i = 0; i < 5; i++) {
            await limiter.executeRequest(
                '/markets',
                async () => ({ id: i }),
                5
            );
        }
        
        // Make a failing request
        try {
            await limiter.executeRequest(
                '/markets',
                async () => {
                    throw new Error('Test error');
                },
                5
            );
        } catch (e) {
            // Expected
        }
        
        const status = limiter.getStatus();
        
        expect(status.recentRequests).toBeGreaterThanOrEqual(6);
        expect(status.failureRate).toBeGreaterThan(0);
        expect(status.failureRate).toBeLessThan(0.2);
    });

    it('should execute batch requests', async () => {
        const requests = [
            { endpoint: '/markets', execute: async () => ({ id: 1 }), priority: 5 },
            { endpoint: '/markets', execute: async () => ({ id: 2 }), priority: 10 },
            { endpoint: '/orders', execute: async () => ({ id: 3 }), priority: 5 },
        ];
        
        const results = await limiter.executeBatch(requests);
        
        expect(results).toHaveLength(3);
        expect(results[0]).toEqual({ id: 1 });
        expect(results[1]).toEqual({ id: 2 });
        expect(results[2]).toEqual({ id: 3 });
    });

    it('should handle adaptive backoff', async () => {
        // Simulate multiple failures
        for (let i = 0; i < 5; i++) {
            try {
                await limiter.executeRequest(
                    '/markets',
                    async () => {
                        throw new Error('Test failure');
                    },
                    5
                );
            } catch (e) {
                // Expected
            }
        }
        
        const backoff = limiter.getAdaptiveBackoff('/markets');
        
        // High failure rate should result in longer backoff
        expect(backoff).toBeGreaterThan(2000);
    });
});

describe('PriorityQueue', () => {
    it('should dequeue items by priority', () => {
        const queue = new PriorityQueue<{ value: number; priority: number }>(
            (a, b) => b.priority - a.priority
        );
        
        queue.enqueue({ value: 1, priority: 5 });
        queue.enqueue({ value: 2, priority: 10 });
        queue.enqueue({ value: 3, priority: 3 });
        
        expect(queue.dequeue()?.value).toBe(2); // Highest priority
        expect(queue.dequeue()?.value).toBe(1);
        expect(queue.dequeue()?.value).toBe(3); // Lowest priority
    });

    it('should handle empty queue', () => {
        const queue = new PriorityQueue<number>((a, b) => b - a);
        
        expect(queue.isEmpty()).toBe(true);
        expect(queue.dequeue()).toBeUndefined();
        expect(queue.peek()).toBeUndefined();
    });

    it('should maintain correct size', () => {
        const queue = new PriorityQueue<number>((a, b) => a - b);
        
        expect(queue.size()).toBe(0);
        
        queue.enqueue(1);
        queue.enqueue(2);
        queue.enqueue(3);
        
        expect(queue.size()).toBe(3);
        
        queue.dequeue();
        
        expect(queue.size()).toBe(2);
    });
});

describe('Rate Limit Compliance', () => {
    it('should not exceed configured limits over time windows', async () => {
        const limiter = new TieredRateLimiter('free');
        const startTime = Date.now();
        const requestLog: number[] = [];
        
        // Make continuous requests for 30 seconds
        const testDuration = 30000;
        const requests = [];
        
        while (Date.now() - startTime < testDuration) {
            requests.push(
                limiter.executeRequest(
                    '/markets',
                    async () => {
                        requestLog.push(Date.now() - startTime);
                        return { timestamp: Date.now() };
                    },
                    5
                )
            );
            
            // Small delay to not overwhelm
            await new Promise(resolve => setTimeout(resolve, 50));
        }
        
        await Promise.all(requests);
        
        // Analyze compliance - should not exceed 50 requests per 10 seconds
        for (let window = 0; window < testDuration - 10000; window += 1000) {
            const windowEnd = window + 10000;
            const requestsInWindow = requestLog.filter(
                t => t >= window && t < windowEnd
            ).length;
            
            expect(requestsInWindow).toBeLessThanOrEqual(50);
        }
    });
});