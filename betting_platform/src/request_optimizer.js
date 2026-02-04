"use strict";
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
var __awaiter = (this && this.__awaiter) || function (thisArg, _arguments, P, generator) {
    function adopt(value) { return value instanceof P ? value : new P(function (resolve) { resolve(value); }); }
    return new (P || (P = Promise))(function (resolve, reject) {
        function fulfilled(value) { try { step(generator.next(value)); } catch (e) { reject(e); } }
        function rejected(value) { try { step(generator["throw"](value)); } catch (e) { reject(e); } }
        function step(result) { result.done ? resolve(result.value) : adopt(result.value).then(fulfilled, rejected); }
        step((generator = generator.apply(thisArg, _arguments || [])).next());
    });
};
var __rest = (this && this.__rest) || function (s, e) {
    var t = {};
    for (var p in s) if (Object.prototype.hasOwnProperty.call(s, p) && e.indexOf(p) < 0)
        t[p] = s[p];
    if (s != null && typeof Object.getOwnPropertySymbols === "function")
        for (var i = 0, p = Object.getOwnPropertySymbols(s); i < p.length; i++) {
            if (e.indexOf(p[i]) < 0 && Object.prototype.propertyIsEnumerable.call(s, p[i]))
                t[p[i]] = s[p[i]];
        }
    return t;
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.RequestDeduplicator = exports.RequestOptimizer = void 0;
const zlib = __importStar(require("zlib"));
const util_1 = require("util");
const gzipAsync = (0, util_1.promisify)(zlib.gzip);
const gunzipAsync = (0, util_1.promisify)(zlib.gunzip);
class RequestOptimizer {
    constructor(rateLimiter) {
        this.rateLimiter = rateLimiter;
        this.batchQueue = new Map();
        this.batchTimers = new Map();
        this.compressionEnabled = true;
        this.parallelRequests = 5;
        this.config = {
            maxBatchSize: 100,
            maxWaitTime: 100, // ms
            compressionThreshold: 1024, // bytes
        };
    }
    batchRequest(endpoint_1, params_1) {
        return __awaiter(this, arguments, void 0, function* (endpoint, params, priority = 5) {
            const batchKey = this.getBatchKey(endpoint, params);
            return new Promise((resolve, reject) => {
                // Initialize batch queue if needed
                if (!this.batchQueue.has(batchKey)) {
                    this.batchQueue.set(batchKey, []);
                    // Schedule batch execution
                    const timer = setTimeout(() => this.executeBatch(batchKey), this.config.maxWaitTime);
                    this.batchTimers.set(batchKey, timer);
                }
                // Add request to batch
                const batch = this.batchQueue.get(batchKey);
                batch.push({
                    params,
                    resolve,
                    reject,
                    priority,
                    timestamp: Date.now(),
                });
                // Execute immediately if batch is full
                if (batch.length >= this.config.maxBatchSize) {
                    const timer = this.batchTimers.get(batchKey);
                    if (timer) {
                        clearTimeout(timer);
                        this.batchTimers.delete(batchKey);
                    }
                    this.executeBatch(batchKey);
                }
            });
        });
    }
    executeBatch(batchKey) {
        return __awaiter(this, void 0, void 0, function* () {
            const requests = this.batchQueue.get(batchKey);
            if (!requests || requests.length === 0)
                return;
            // Clear batch
            this.batchQueue.delete(batchKey);
            this.batchTimers.delete(batchKey);
            // Sort by priority
            requests.sort((a, b) => b.priority - a.priority);
            try {
                // Create batch payload
                const batchPayload = {
                    requests: requests.map(r => r.params),
                    timestamp: Date.now(),
                    count: requests.length,
                };
                // Compress if beneficial
                const payload = yield this.maybeCompress(batchPayload);
                // Execute batch through rate limiter
                const results = yield this.rateLimiter.executeRequest(batchKey, () => this.executeBatchRequest(batchKey, payload), Math.max(...requests.map(r => r.priority)));
                // Distribute results
                if (Array.isArray(results)) {
                    results.forEach((result, index) => {
                        if (requests[index]) {
                            requests[index].resolve(result);
                        }
                    });
                }
                else {
                    // Single result for all requests
                    requests.forEach(req => req.resolve(results));
                }
            }
            catch (error) {
                // Reject all requests in batch
                requests.forEach(req => req.reject(error));
            }
        });
    }
    maybeCompress(data) {
        return __awaiter(this, void 0, void 0, function* () {
            const json = JSON.stringify(data);
            if (!this.compressionEnabled || json.length < this.config.compressionThreshold) {
                return json;
            }
            const compressed = yield gzipAsync(Buffer.from(json));
            // Only use compression if it saves space
            if (compressed.length < json.length * 0.9) {
                return compressed;
            }
            return json;
        });
    }
    executeBatchRequest(endpoint, payload) {
        return __awaiter(this, void 0, void 0, function* () {
            // This would be the actual HTTP request
            // For now, return mock data
            console.log(`Executing batch request to ${endpoint}, size: ${Buffer.isBuffer(payload) ? payload.length : payload.length} bytes`);
            // Simulate processing
            yield new Promise(resolve => setTimeout(resolve, 100));
            // Return mock results
            const payloadData = Buffer.isBuffer(payload)
                ? JSON.parse((yield gunzipAsync(payload)).toString())
                : JSON.parse(payload);
            return payloadData.requests.map((req) => ({
                success: true,
                data: Object.assign({ id: Math.random().toString(36) }, req),
            }));
        });
    }
    getBatchKey(endpoint, params) {
        // Create batch key based on endpoint and common parameters
        const commonParams = this.extractCommonParams(params);
        return `${endpoint}:${JSON.stringify(commonParams)}`;
    }
    extractCommonParams(params) {
        // Extract parameters that should be the same for batching
        const { id, timestamp } = params, common = __rest(params, ["id", "timestamp"]);
        return common;
    }
    // Optimize market fetching with parallel requests
    optimizeMarketFetch(marketIds) {
        return __awaiter(this, void 0, void 0, function* () {
            // Group markets by verse for efficient fetching
            const verseGroups = yield this.groupMarketsByVerse(marketIds);
            // Create parallel fetch tasks
            const tasks = [];
            for (const [verseId, ids] of verseGroups) {
                // Split large groups into chunks
                const chunks = this.chunkArray(ids, 50);
                for (const chunk of chunks) {
                    tasks.push(this.fetchVerseMarkets(verseId, chunk));
                }
            }
            // Execute in parallel with concurrency limit
            const results = yield this.executeParallel(tasks, this.parallelRequests);
            return results.flat();
        });
    }
    groupMarketsByVerse(marketIds) {
        return __awaiter(this, void 0, void 0, function* () {
            // This would use the verse classifier
            // For now, use simple grouping
            const groups = new Map();
            marketIds.forEach(id => {
                const verseId = `verse_${parseInt(id.split('_')[1]) % 10}`;
                if (!groups.has(verseId)) {
                    groups.set(verseId, []);
                }
                groups.get(verseId).push(id);
            });
            return groups;
        });
    }
    fetchVerseMarkets(verseId, marketIds) {
        return __awaiter(this, void 0, void 0, function* () {
            return this.batchRequest(`/verses/${verseId}/markets`, { ids: marketIds });
        });
    }
    chunkArray(array, size) {
        const chunks = [];
        for (let i = 0; i < array.length; i += size) {
            chunks.push(array.slice(i, i + size));
        }
        return chunks;
    }
    executeParallel(tasks, concurrency) {
        return __awaiter(this, void 0, void 0, function* () {
            const results = [];
            const executing = [];
            for (const task of tasks) {
                const promise = task.then(result => {
                    results.push(result);
                });
                executing.push(promise);
                if (executing.length >= concurrency) {
                    yield Promise.race(executing);
                    executing.splice(executing.findIndex(p => p === promise), 1);
                }
            }
            yield Promise.all(executing);
            return results;
        });
    }
    // Optimize resolution fetching
    optimizeResolutionFetch(marketIds) {
        return __awaiter(this, void 0, void 0, function* () {
            const resolutions = new Map();
            // Batch by resolution status
            const resolved = [];
            const pending = [];
            const disputed = [];
            // This would check actual status
            marketIds.forEach(id => {
                const random = Math.random();
                if (random < 0.7)
                    pending.push(id);
                else if (random < 0.9)
                    resolved.push(id);
                else
                    disputed.push(id);
            });
            // Fetch each type in parallel
            const [resolvedData, pendingData, disputedData] = yield Promise.all([
                resolved.length > 0 ? this.batchRequest('/resolutions/resolved', { ids: resolved }) : [],
                pending.length > 0 ? this.batchRequest('/resolutions/pending', { ids: pending }) : [],
                disputed.length > 0 ? this.batchRequest('/resolutions/disputed', { ids: disputed }) : [],
            ]);
            // Combine results
            const allData = [
                ...resolvedData,
                ...pendingData,
                ...disputedData
            ];
            allData.forEach((res) => {
                resolutions.set(res.marketId, res);
            });
            return resolutions;
        });
    }
    // Configuration methods
    setCompressionEnabled(enabled) {
        this.compressionEnabled = enabled;
    }
    setParallelRequests(count) {
        this.parallelRequests = Math.max(1, Math.min(10, count));
    }
    setBatchConfig(config) {
        this.config = Object.assign(Object.assign({}, this.config), config);
    }
    // Metrics
    getOptimizationMetrics() {
        const queueSizes = new Map();
        for (const [key, batch] of this.batchQueue) {
            queueSizes.set(key, batch.length);
        }
        return {
            activeQueues: this.batchQueue.size,
            queueSizes: Object.fromEntries(queueSizes),
            compressionEnabled: this.compressionEnabled,
            parallelRequests: this.parallelRequests,
            config: this.config,
        };
    }
}
exports.RequestOptimizer = RequestOptimizer;
// Request deduplication
class RequestDeduplicator {
    constructor() {
        this.pendingRequests = new Map();
        this.cache = new Map();
        this.cacheTTL = 60000; // 1 minute
    }
    deduplicate(key, request) {
        return __awaiter(this, void 0, void 0, function* () {
            // Check cache first
            const cached = this.cache.get(key);
            if (cached && Date.now() - cached.timestamp < this.cacheTTL) {
                return cached.data;
            }
            // Check if request is already pending
            if (this.pendingRequests.has(key)) {
                return this.pendingRequests.get(key);
            }
            // Execute request
            const promise = request()
                .then(data => {
                // Cache result
                this.cache.set(key, { data, timestamp: Date.now() });
                // Clean old cache entries
                this.cleanCache();
                return data;
            })
                .finally(() => {
                this.pendingRequests.delete(key);
            });
            this.pendingRequests.set(key, promise);
            return promise;
        });
    }
    cleanCache() {
        const now = Date.now();
        const expired = [];
        for (const [key, value] of this.cache) {
            if (now - value.timestamp > this.cacheTTL) {
                expired.push(key);
            }
        }
        expired.forEach(key => this.cache.delete(key));
    }
    setCacheTTL(ttl) {
        this.cacheTTL = ttl;
    }
    clearCache() {
        this.cache.clear();
    }
}
exports.RequestDeduplicator = RequestDeduplicator;
