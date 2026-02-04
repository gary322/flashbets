import { EventEmitter } from 'events';

interface TokenBucketConfig {
    rate: number;      // tokens per interval
    per: number;       // interval in milliseconds
    burst: number;     // maximum burst capacity
}

interface TierLimits {
    markets: TokenBucketConfig;
    orders: TokenBucketConfig;
    resolutions: TokenBucketConfig;
}

interface QueuedRequest<T> {
    execute: () => Promise<T>;
    resolve: (value: T) => void;
    reject: (error: any) => void;
    priority: number;
    endpoint: string;
    timestamp: number;
}

export class TokenBucket {
    private tokens: number;
    private maxTokens: number;
    private refillRate: number;
    private lastRefill: number;

    constructor(config: TokenBucketConfig) {
        this.maxTokens = config.burst;
        this.tokens = config.burst;
        this.refillRate = config.rate / (config.per / 1000); // tokens per ms
        this.lastRefill = Date.now();
    }

    tryConsume(count: number): boolean {
        this.refill();

        if (this.tokens >= count) {
            this.tokens -= count;
            return true;
        }

        return false;
    }

    async waitForTokens(count: number): Promise<void> {
        while (!this.tryConsume(count)) {
            const needed = count - this.tokens;
            const waitTime = Math.ceil(needed / this.refillRate);
            await new Promise(resolve => setTimeout(resolve, waitTime));
        }
    }

    private refill() {
        const now = Date.now();
        const elapsed = now - this.lastRefill;
        const tokensToAdd = elapsed * this.refillRate;

        this.tokens = Math.min(this.maxTokens, this.tokens + tokensToAdd);
        this.lastRefill = now;
    }

    getAvailableTokens(): number {
        this.refill();
        return this.tokens;
    }

    getCapacity(): number {
        return this.maxTokens;
    }
}

export class PriorityQueue<T> {
    private items: T[] = [];

    constructor(private compareFunction: (a: T, b: T) => number) {}

    enqueue(item: T): void {
        this.items.push(item);
        this.items.sort(this.compareFunction);
    }

    dequeue(): T | undefined {
        return this.items.shift();
    }

    peek(): T | undefined {
        return this.items[0];
    }

    size(): number {
        return this.items.length;
    }

    isEmpty(): boolean {
        return this.items.length === 0;
    }
}

export class TieredRateLimiter extends EventEmitter {
    private buckets: Map<string, TokenBucket> = new Map();
    private requestQueue: PriorityQueue<QueuedRequest<any>>;
    private tier: 'free' | 'basic' | 'premium';
    private processing: boolean = false;
    private requestLog: { endpoint: string; timestamp: number; success: boolean }[] = [];

    constructor(tier: 'free' | 'basic' | 'premium' = 'free') {
        super();
        this.tier = tier;
        this.requestQueue = new PriorityQueue((a, b) => {
            // Higher priority first, then earlier timestamp
            if (a.priority !== b.priority) {
                return b.priority - a.priority;
            }
            return a.timestamp - b.timestamp;
        });

        // Initialize buckets based on tier
        const limits = this.getTierLimits();
        this.buckets.set('markets', new TokenBucket(limits.markets));
        this.buckets.set('orders', new TokenBucket(limits.orders));
        this.buckets.set('resolutions', new TokenBucket(limits.resolutions));

        // Start queue processor
        this.startQueueProcessor();
    }

    getTierLimits(): TierLimits {
        switch (this.tier) {
            case 'free':
                return {
                    markets: { rate: 50, per: 10000, burst: 10 },
                    orders: { rate: 100, per: 10000, burst: 20 },
                    resolutions: { rate: 10, per: 10000, burst: 5 },
                };
            case 'basic':
                return {
                    markets: { rate: 500, per: 10000, burst: 100 },
                    orders: { rate: 1000, per: 10000, burst: 200 },
                    resolutions: { rate: 100, per: 10000, burst: 20 },
                };
            case 'premium':
                return {
                    markets: { rate: 2000, per: 10000, burst: 400 },
                    orders: { rate: 5000, per: 10000, burst: 1000 },
                    resolutions: { rate: 500, per: 10000, burst: 100 },
                };
        }
    }

    async executeRequest<T>(
        endpoint: string,
        request: () => Promise<T>,
        priority: number = 5
    ): Promise<T> {
        const bucket = this.getBucketForEndpoint(endpoint);

        // Try immediate execution
        if (bucket.tryConsume(1)) {
            this.logRequest(endpoint, true);
            return await this.executeWithRetry(request);
        }

        // Queue request
        return new Promise((resolve, reject) => {
            this.requestQueue.enqueue({
                execute: request,
                resolve,
                reject,
                priority,
                endpoint,
                timestamp: Date.now(),
            });
            this.emit('request_queued', { endpoint, priority, queueSize: this.requestQueue.size() });
        });
    }

    private async executeWithRetry<T>(
        request: () => Promise<T>,
        maxRetries = 3
    ): Promise<T> {
        let lastError;

        for (let attempt = 0; attempt < maxRetries; attempt++) {
            try {
                return await request();
            } catch (error: any) {
                lastError = error;

                if (error.response?.status === 429) {
                    // Rate limited - exponential backoff
                    const backoff = Math.pow(2, attempt) * 1000;
                    const jitter = Math.random() * 1000;
                    await new Promise(resolve =>
                        setTimeout(resolve, backoff + jitter)
                    );
                    this.emit('rate_limited', { attempt, backoff: backoff + jitter });
                } else if (error.code === 'ECONNRESET' || error.code === 'ETIMEDOUT') {
                    // Network error - quick retry
                    await new Promise(resolve => setTimeout(resolve, 100));
                    this.emit('network_error', { attempt, error: error.code });
                } else {
                    // Non-retryable error
                    throw error;
                }
            }
        }

        throw lastError;
    }

    private getBucketForEndpoint(endpoint: string): TokenBucket {
        if (endpoint.includes('/markets')) {
            return this.buckets.get('markets')!;
        } else if (endpoint.includes('/orders')) {
            return this.buckets.get('orders')!;
        } else if (endpoint.includes('/resolutions')) {
            return this.buckets.get('resolutions')!;
        }
        // Default to markets bucket
        return this.buckets.get('markets')!;
    }

    private async startQueueProcessor() {
        while (true) {
            if (this.requestQueue.isEmpty()) {
                await new Promise(resolve => setTimeout(resolve, 100));
                continue;
            }

            const request = this.requestQueue.dequeue();
            if (!request) continue;

            const bucket = this.getBucketForEndpoint(request.endpoint);

            // Wait for token
            await bucket.waitForTokens(1);

            try {
                this.logRequest(request.endpoint, true);
                const result = await this.executeWithRetry(request.execute);
                request.resolve(result);
                this.emit('request_completed', { endpoint: request.endpoint, priority: request.priority });
            } catch (error) {
                this.logRequest(request.endpoint, false);
                request.reject(error);
                this.emit('request_failed', { endpoint: request.endpoint, error });
            }
        }
    }

    private logRequest(endpoint: string, success: boolean) {
        this.requestLog.push({
            endpoint,
            timestamp: Date.now(),
            success,
        });

        // Keep only last 1000 entries
        if (this.requestLog.length > 1000) {
            this.requestLog = this.requestLog.slice(-1000);
        }
    }

    // Adaptive backoff based on recent failures
    getAdaptiveBackoff(endpoint: string): number {
        const recentRequests = this.requestLog
            .filter(r => r.endpoint === endpoint && Date.now() - r.timestamp < 60000);
        
        const failureRate = recentRequests.filter(r => !r.success).length / recentRequests.length;
        
        if (failureRate > 0.5) {
            return 5000; // 5 seconds if high failure rate
        } else if (failureRate > 0.2) {
            return 2000; // 2 seconds if moderate failure rate
        }
        return 1000; // 1 second base backoff
    }

    // Get current status
    getStatus() {
        const bucketStatus = new Map<string, any>();
        
        for (const [name, bucket] of this.buckets) {
            bucketStatus.set(name, {
                available: bucket.getAvailableTokens(),
                capacity: bucket.getCapacity(),
                utilization: (bucket.getCapacity() - bucket.getAvailableTokens()) / bucket.getCapacity(),
            });
        }

        return {
            tier: this.tier,
            queueSize: this.requestQueue.size(),
            buckets: Object.fromEntries(bucketStatus),
            recentRequests: this.requestLog.length,
            failureRate: this.requestLog.filter(r => !r.success).length / this.requestLog.length,
        };
    }

    // Request prioritization helpers
    static HIGH_PRIORITY = 10;
    static MEDIUM_PRIORITY = 5;
    static LOW_PRIORITY = 1;

    // Batch request support
    async executeBatch<T>(
        requests: Array<{ endpoint: string; execute: () => Promise<T>; priority?: number }>
    ): Promise<T[]> {
        const promises = requests.map(req =>
            this.executeRequest(req.endpoint, req.execute, req.priority || 5)
        );
        return Promise.all(promises);
    }

    // Emergency mode - reduces all rate limits by 50%
    enableEmergencyMode() {
        const limits = this.getTierLimits();
        
        for (const [key, config] of Object.entries(limits)) {
            const reducedConfig = {
                rate: Math.floor(config.rate / 2),
                per: config.per,
                burst: Math.floor(config.burst / 2),
            };
            this.buckets.set(key, new TokenBucket(reducedConfig));
        }
        
        this.emit('emergency_mode_enabled');
    }

    // Reset to normal limits
    disableEmergencyMode() {
        const limits = this.getTierLimits();
        
        for (const [key, config] of Object.entries(limits)) {
            this.buckets.set(key, new TokenBucket(config));
        }
        
        this.emit('emergency_mode_disabled');
    }
}

// Export for compliance checking
export interface RateLimitMetrics {
    endpoint: string;
    timestamp: number;
    tokensUsed: number;
    remainingTokens: number;
    queueDepth: number;
}

export class RateLimitMonitor {
    private metrics: RateLimitMetrics[] = [];

    recordMetric(metric: RateLimitMetrics) {
        this.metrics.push(metric);
        
        // Keep only last hour of metrics
        const oneHourAgo = Date.now() - 3600000;
        this.metrics = this.metrics.filter(m => m.timestamp > oneHourAgo);
    }

    getComplianceReport(windowMs: number = 10000): any {
        const now = Date.now();
        const windowStart = now - windowMs;
        
        const windowMetrics = this.metrics.filter(m => m.timestamp >= windowStart);
        
        const byEndpoint = new Map<string, number>();
        for (const metric of windowMetrics) {
            const count = byEndpoint.get(metric.endpoint) || 0;
            byEndpoint.set(metric.endpoint, count + metric.tokensUsed);
        }

        return {
            window: `${windowMs}ms`,
            usage: Object.fromEntries(byEndpoint),
            totalRequests: windowMetrics.length,
            averageQueueDepth: windowMetrics.reduce((sum, m) => sum + m.queueDepth, 0) / windowMetrics.length,
        };
    }
}