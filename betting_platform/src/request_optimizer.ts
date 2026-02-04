import * as zlib from 'zlib';
import { TieredRateLimiter } from './rate_limiter';
import { promisify } from 'util';

const gzipAsync = promisify(zlib.gzip);
const gunzipAsync = promisify(zlib.gunzip);

interface BatchRequest {
    params: any;
    resolve: (value: any) => void;
    reject: (error: any) => void;
    priority: number;
    timestamp: number;
}

interface BatchConfig {
    maxBatchSize: number;
    maxWaitTime: number;
    compressionThreshold: number;
}

export class RequestOptimizer {
    private batchQueue: Map<string, BatchRequest[]> = new Map();
    private batchTimers: Map<string, NodeJS.Timeout> = new Map();
    private compressionEnabled: boolean = true;
    private parallelRequests: number = 5;
    private config: BatchConfig = {
        maxBatchSize: 100,
        maxWaitTime: 100, // ms
        compressionThreshold: 1024, // bytes
    };

    constructor(private rateLimiter: TieredRateLimiter) {}

    async batchRequest<T>(
        endpoint: string,
        params: any,
        priority: number = 5
    ): Promise<T> {
        const batchKey = this.getBatchKey(endpoint, params);

        return new Promise((resolve, reject) => {
            // Initialize batch queue if needed
            if (!this.batchQueue.has(batchKey)) {
                this.batchQueue.set(batchKey, []);
                
                // Schedule batch execution
                const timer = setTimeout(
                    () => this.executeBatch(batchKey),
                    this.config.maxWaitTime
                );
                this.batchTimers.set(batchKey, timer);
            }

            // Add request to batch
            const batch = this.batchQueue.get(batchKey)!;
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
    }

    private async executeBatch(batchKey: string) {
        const requests = this.batchQueue.get(batchKey);
        if (!requests || requests.length === 0) return;

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
            const payload = await this.maybeCompress(batchPayload);

            // Execute batch through rate limiter
            const results = await this.rateLimiter.executeRequest(
                batchKey,
                () => this.executeBatchRequest(batchKey, payload),
                Math.max(...requests.map(r => r.priority))
            );

            // Distribute results
            if (Array.isArray(results)) {
                results.forEach((result, index) => {
                    if (requests[index]) {
                        requests[index].resolve(result);
                    }
                });
            } else {
                // Single result for all requests
                requests.forEach(req => req.resolve(results));
            }
        } catch (error) {
            // Reject all requests in batch
            requests.forEach(req => req.reject(error));
        }
    }

    private async maybeCompress(data: any): Promise<Buffer | string> {
        const json = JSON.stringify(data);
        
        if (!this.compressionEnabled || json.length < this.config.compressionThreshold) {
            return json;
        }

        const compressed = await gzipAsync(Buffer.from(json));
        
        // Only use compression if it saves space
        if (compressed.length < json.length * 0.9) {
            return compressed;
        }
        
        return json;
    }

    private async executeBatchRequest(endpoint: string, payload: Buffer | string): Promise<any> {
        // This would be the actual HTTP request
        // For now, return mock data
        console.log(`Executing batch request to ${endpoint}, size: ${
            Buffer.isBuffer(payload) ? payload.length : payload.length
        } bytes`);
        
        // Simulate processing
        await new Promise(resolve => setTimeout(resolve, 100));
        
        // Return mock results
        const payloadData = Buffer.isBuffer(payload) 
            ? JSON.parse((await gunzipAsync(payload)).toString())
            : JSON.parse(payload);
            
        return payloadData.requests.map((req: any) => ({
            success: true,
            data: { id: Math.random().toString(36), ...req },
        }));
    }

    private getBatchKey(endpoint: string, params: any): string {
        // Create batch key based on endpoint and common parameters
        const commonParams = this.extractCommonParams(params);
        return `${endpoint}:${JSON.stringify(commonParams)}`;
    }

    private extractCommonParams(params: any): any {
        // Extract parameters that should be the same for batching
        const { id, timestamp, ...common } = params;
        return common;
    }

    // Optimize market fetching with parallel requests
    async optimizeMarketFetch(marketIds: string[]): Promise<any[]> {
        // Group markets by verse for efficient fetching
        const verseGroups = await this.groupMarketsByVerse(marketIds);
        
        // Create parallel fetch tasks
        const tasks: Promise<any>[] = [];
        
        for (const [verseId, ids] of verseGroups) {
            // Split large groups into chunks
            const chunks = this.chunkArray(ids, 50);
            
            for (const chunk of chunks) {
                tasks.push(this.fetchVerseMarkets(verseId, chunk));
            }
        }

        // Execute in parallel with concurrency limit
        const results = await this.executeParallel(tasks, this.parallelRequests);
        
        return results.flat();
    }

    private async groupMarketsByVerse(marketIds: string[]): Promise<Map<string, string[]>> {
        // This would use the verse classifier
        // For now, use simple grouping
        const groups = new Map<string, string[]>();
        
        marketIds.forEach(id => {
            const verseId = `verse_${parseInt(id.split('_')[1]) % 10}`;
            if (!groups.has(verseId)) {
                groups.set(verseId, []);
            }
            groups.get(verseId)!.push(id);
        });
        
        return groups;
    }

    private async fetchVerseMarkets(verseId: string, marketIds: string[]): Promise<any[]> {
        return this.batchRequest(`/verses/${verseId}/markets`, { ids: marketIds });
    }

    private chunkArray<T>(array: T[], size: number): T[][] {
        const chunks: T[][] = [];
        for (let i = 0; i < array.length; i += size) {
            chunks.push(array.slice(i, i + size));
        }
        return chunks;
    }

    private async executeParallel<T>(
        tasks: Promise<T>[],
        concurrency: number
    ): Promise<T[]> {
        const results: T[] = [];
        const executing: Promise<void>[] = [];

        for (const task of tasks) {
            const promise = task.then(result => {
                results.push(result);
            });

            executing.push(promise);

            if (executing.length >= concurrency) {
                await Promise.race(executing);
                executing.splice(
                    executing.findIndex(p => p === promise),
                    1
                );
            }
        }

        await Promise.all(executing);
        return results;
    }

    // Optimize resolution fetching
    async optimizeResolutionFetch(marketIds: string[]): Promise<Map<string, any>> {
        const resolutions = new Map<string, any>();
        
        // Batch by resolution status
        const resolved: string[] = [];
        const pending: string[] = [];
        const disputed: string[] = [];
        
        // This would check actual status
        marketIds.forEach(id => {
            const random = Math.random();
            if (random < 0.7) pending.push(id);
            else if (random < 0.9) resolved.push(id);
            else disputed.push(id);
        });
        
        // Fetch each type in parallel
        const [resolvedData, pendingData, disputedData] = await Promise.all([
            resolved.length > 0 ? this.batchRequest('/resolutions/resolved', { ids: resolved }) : [],
            pending.length > 0 ? this.batchRequest('/resolutions/pending', { ids: pending }) : [],
            disputed.length > 0 ? this.batchRequest('/resolutions/disputed', { ids: disputed }) : [],
        ]);
        
        // Combine results
        const allData = [
            ...(resolvedData as any[]),
            ...(pendingData as any[]),
            ...(disputedData as any[])
        ];
        allData.forEach((res: any) => {
            resolutions.set(res.marketId, res);
        });
        
        return resolutions;
    }

    // Configuration methods
    setCompressionEnabled(enabled: boolean) {
        this.compressionEnabled = enabled;
    }

    setParallelRequests(count: number) {
        this.parallelRequests = Math.max(1, Math.min(10, count));
    }

    setBatchConfig(config: Partial<BatchConfig>) {
        this.config = { ...this.config, ...config };
    }

    // Metrics
    getOptimizationMetrics() {
        const queueSizes = new Map<string, number>();
        
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

// Request deduplication
export class RequestDeduplicator {
    private pendingRequests: Map<string, Promise<any>> = new Map();
    private cache: Map<string, { data: any; timestamp: number }> = new Map();
    private cacheTTL: number = 60000; // 1 minute

    async deduplicate<T>(
        key: string,
        request: () => Promise<T>
    ): Promise<T> {
        // Check cache first
        const cached = this.cache.get(key);
        if (cached && Date.now() - cached.timestamp < this.cacheTTL) {
            return cached.data;
        }

        // Check if request is already pending
        if (this.pendingRequests.has(key)) {
            return this.pendingRequests.get(key)!;
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
    }

    private cleanCache() {
        const now = Date.now();
        const expired: string[] = [];
        
        for (const [key, value] of this.cache) {
            if (now - value.timestamp > this.cacheTTL) {
                expired.push(key);
            }
        }
        
        expired.forEach(key => this.cache.delete(key));
    }

    setCacheTTL(ttl: number) {
        this.cacheTTL = ttl;
    }

    clearCache() {
        this.cache.clear();
    }
}