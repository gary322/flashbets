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
exports.RateLimitMonitor = exports.TieredRateLimiter = exports.PriorityQueue = exports.TokenBucket = void 0;
const events_1 = require("events");
class TokenBucket {
    constructor(config) {
        this.maxTokens = config.burst;
        this.tokens = config.burst;
        this.refillRate = config.rate / (config.per / 1000); // tokens per ms
        this.lastRefill = Date.now();
    }
    tryConsume(count) {
        this.refill();
        if (this.tokens >= count) {
            this.tokens -= count;
            return true;
        }
        return false;
    }
    waitForTokens(count) {
        return __awaiter(this, void 0, void 0, function* () {
            while (!this.tryConsume(count)) {
                const needed = count - this.tokens;
                const waitTime = Math.ceil(needed / this.refillRate);
                yield new Promise(resolve => setTimeout(resolve, waitTime));
            }
        });
    }
    refill() {
        const now = Date.now();
        const elapsed = now - this.lastRefill;
        const tokensToAdd = elapsed * this.refillRate;
        this.tokens = Math.min(this.maxTokens, this.tokens + tokensToAdd);
        this.lastRefill = now;
    }
    getAvailableTokens() {
        this.refill();
        return this.tokens;
    }
    getCapacity() {
        return this.maxTokens;
    }
}
exports.TokenBucket = TokenBucket;
class PriorityQueue {
    constructor(compareFunction) {
        this.compareFunction = compareFunction;
        this.items = [];
    }
    enqueue(item) {
        this.items.push(item);
        this.items.sort(this.compareFunction);
    }
    dequeue() {
        return this.items.shift();
    }
    peek() {
        return this.items[0];
    }
    size() {
        return this.items.length;
    }
    isEmpty() {
        return this.items.length === 0;
    }
}
exports.PriorityQueue = PriorityQueue;
class TieredRateLimiter extends events_1.EventEmitter {
    constructor(tier = 'free') {
        super();
        this.buckets = new Map();
        this.processing = false;
        this.requestLog = [];
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
    getTierLimits() {
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
    executeRequest(endpoint_1, request_1) {
        return __awaiter(this, arguments, void 0, function* (endpoint, request, priority = 5) {
            const bucket = this.getBucketForEndpoint(endpoint);
            // Try immediate execution
            if (bucket.tryConsume(1)) {
                this.logRequest(endpoint, true);
                return yield this.executeWithRetry(request);
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
        });
    }
    executeWithRetry(request_1) {
        return __awaiter(this, arguments, void 0, function* (request, maxRetries = 3) {
            var _a;
            let lastError;
            for (let attempt = 0; attempt < maxRetries; attempt++) {
                try {
                    return yield request();
                }
                catch (error) {
                    lastError = error;
                    if (((_a = error.response) === null || _a === void 0 ? void 0 : _a.status) === 429) {
                        // Rate limited - exponential backoff
                        const backoff = Math.pow(2, attempt) * 1000;
                        const jitter = Math.random() * 1000;
                        yield new Promise(resolve => setTimeout(resolve, backoff + jitter));
                        this.emit('rate_limited', { attempt, backoff: backoff + jitter });
                    }
                    else if (error.code === 'ECONNRESET' || error.code === 'ETIMEDOUT') {
                        // Network error - quick retry
                        yield new Promise(resolve => setTimeout(resolve, 100));
                        this.emit('network_error', { attempt, error: error.code });
                    }
                    else {
                        // Non-retryable error
                        throw error;
                    }
                }
            }
            throw lastError;
        });
    }
    getBucketForEndpoint(endpoint) {
        if (endpoint.includes('/markets')) {
            return this.buckets.get('markets');
        }
        else if (endpoint.includes('/orders')) {
            return this.buckets.get('orders');
        }
        else if (endpoint.includes('/resolutions')) {
            return this.buckets.get('resolutions');
        }
        // Default to markets bucket
        return this.buckets.get('markets');
    }
    startQueueProcessor() {
        return __awaiter(this, void 0, void 0, function* () {
            while (true) {
                if (this.requestQueue.isEmpty()) {
                    yield new Promise(resolve => setTimeout(resolve, 100));
                    continue;
                }
                const request = this.requestQueue.dequeue();
                if (!request)
                    continue;
                const bucket = this.getBucketForEndpoint(request.endpoint);
                // Wait for token
                yield bucket.waitForTokens(1);
                try {
                    this.logRequest(request.endpoint, true);
                    const result = yield this.executeWithRetry(request.execute);
                    request.resolve(result);
                    this.emit('request_completed', { endpoint: request.endpoint, priority: request.priority });
                }
                catch (error) {
                    this.logRequest(request.endpoint, false);
                    request.reject(error);
                    this.emit('request_failed', { endpoint: request.endpoint, error });
                }
            }
        });
    }
    logRequest(endpoint, success) {
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
    getAdaptiveBackoff(endpoint) {
        const recentRequests = this.requestLog
            .filter(r => r.endpoint === endpoint && Date.now() - r.timestamp < 60000);
        const failureRate = recentRequests.filter(r => !r.success).length / recentRequests.length;
        if (failureRate > 0.5) {
            return 5000; // 5 seconds if high failure rate
        }
        else if (failureRate > 0.2) {
            return 2000; // 2 seconds if moderate failure rate
        }
        return 1000; // 1 second base backoff
    }
    // Get current status
    getStatus() {
        const bucketStatus = new Map();
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
    // Batch request support
    executeBatch(requests) {
        return __awaiter(this, void 0, void 0, function* () {
            const promises = requests.map(req => this.executeRequest(req.endpoint, req.execute, req.priority || 5));
            return Promise.all(promises);
        });
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
exports.TieredRateLimiter = TieredRateLimiter;
// Request prioritization helpers
TieredRateLimiter.HIGH_PRIORITY = 10;
TieredRateLimiter.MEDIUM_PRIORITY = 5;
TieredRateLimiter.LOW_PRIORITY = 1;
class RateLimitMonitor {
    constructor() {
        this.metrics = [];
    }
    recordMetric(metric) {
        this.metrics.push(metric);
        // Keep only last hour of metrics
        const oneHourAgo = Date.now() - 3600000;
        this.metrics = this.metrics.filter(m => m.timestamp > oneHourAgo);
    }
    getComplianceReport(windowMs = 10000) {
        const now = Date.now();
        const windowStart = now - windowMs;
        const windowMetrics = this.metrics.filter(m => m.timestamp >= windowStart);
        const byEndpoint = new Map();
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
exports.RateLimitMonitor = RateLimitMonitor;
