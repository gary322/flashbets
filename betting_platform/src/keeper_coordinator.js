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
Object.defineProperty(exports, "__esModule", { value: true });
exports.KeeperRecovery = exports.KeeperCoordinator = void 0;
const ioredis_1 = require("ioredis");
const events_1 = require("events");
const os = __importStar(require("os"));
class KeeperCoordinator extends events_1.EventEmitter {
    constructor(keeperId, redisUrl) {
        super();
        this.isLeader = false;
        this.heartbeatInterval = null;
        this.leaderCheckInterval = null;
        this.workDistribution = new Map();
        this.currentWork = [];
        this.metrics = {
            processed: 0,
            errors: 0,
            queueDepth: 0,
        };
        this.keeperId = keeperId;
        this.redis = new ioredis_1.Redis(redisUrl);
        this.redisSub = new ioredis_1.Redis(redisUrl);
    }
    start() {
        return __awaiter(this, void 0, void 0, function* () {
            console.log(`Starting keeper coordinator ${this.keeperId}...`);
            // Register keeper
            yield this.registerKeeper();
            // Start heartbeat
            this.heartbeatInterval = setInterval(() => this.sendHeartbeat(), 5000);
            // Subscribe to work assignments
            yield this.subscribeToWorkAssignments();
            // Participate in leader election
            yield this.participateInElection();
            // Start leader check
            this.leaderCheckInterval = setInterval(() => this.checkLeadership(), 10000);
            // Start work processing based on role
            if (this.isLeader) {
                yield this.startLeaderDuties();
            }
            this.emit('started', { keeperId: this.keeperId, isLeader: this.isLeader });
        });
    }
    stop() {
        return __awaiter(this, void 0, void 0, function* () {
            console.log(`Stopping keeper coordinator ${this.keeperId}...`);
            // Clear intervals
            if (this.heartbeatInterval) {
                clearInterval(this.heartbeatInterval);
            }
            if (this.leaderCheckInterval) {
                clearInterval(this.leaderCheckInterval);
            }
            // Unregister keeper
            yield this.unregisterKeeper();
            // Release leader lock if held
            if (this.isLeader) {
                yield this.releaseLeadership();
            }
            // Close Redis connections
            yield this.redis.quit();
            yield this.redisSub.quit();
            this.emit('stopped', { keeperId: this.keeperId });
        });
    }
    registerKeeper() {
        return __awaiter(this, void 0, void 0, function* () {
            const keeperInfo = {
                id: this.keeperId,
                startTime: Date.now(),
                capabilities: ['markets', 'prices', 'resolutions'],
                status: 'active',
                lastHeartbeat: Date.now(),
                host: os.hostname(),
                workload: [],
            };
            yield this.redis.hset('keepers:registry', this.keeperId, JSON.stringify(keeperInfo));
            // Set initial heartbeat with expiry
            yield this.redis.setex(`keeper:${this.keeperId}:heartbeat`, 30, JSON.stringify({ timestamp: Date.now() }));
        });
    }
    unregisterKeeper() {
        return __awaiter(this, void 0, void 0, function* () {
            yield this.redis.hdel('keepers:registry', this.keeperId);
            yield this.redis.del(`keeper:${this.keeperId}:heartbeat`);
        });
    }
    sendHeartbeat() {
        return __awaiter(this, void 0, void 0, function* () {
            const heartbeat = {
                timestamp: Date.now(),
                processed: this.metrics.processed,
                errors: this.metrics.errors,
                queueDepth: this.metrics.queueDepth,
                cpuUsage: process.cpuUsage(),
                memoryUsage: process.memoryUsage(),
            };
            yield this.redis.setex(`keeper:${this.keeperId}:heartbeat`, 30, JSON.stringify(heartbeat));
            // Update keeper info
            const keeperData = yield this.redis.hget('keepers:registry', this.keeperId);
            if (keeperData) {
                const keeperInfo = JSON.parse(keeperData);
                keeperInfo.lastHeartbeat = Date.now();
                keeperInfo.workload = this.currentWork;
                yield this.redis.hset('keepers:registry', this.keeperId, JSON.stringify(keeperInfo));
            }
            this.emit('heartbeat', heartbeat);
        });
    }
    participateInElection() {
        return __awaiter(this, void 0, void 0, function* () {
            const lockKey = 'keeper:leader:lock';
            const lockValue = this.keeperId;
            const lockTTL = 30000; // 30 seconds
            try {
                // Try to acquire leader lock
                const acquired = yield this.redis.set(lockKey, lockValue, 'PX', lockTTL, 'NX');
                if (acquired === 'OK') {
                    this.isLeader = true;
                    console.log(`Keeper ${this.keeperId} elected as leader`);
                    // Refresh lock periodically
                    setInterval(() => __awaiter(this, void 0, void 0, function* () {
                        const current = yield this.redis.get(lockKey);
                        if (current === this.keeperId) {
                            yield this.redis.pexpire(lockKey, lockTTL);
                        }
                        else {
                            // Lost leadership
                            this.handleLeadershipLoss();
                        }
                    }), lockTTL / 3);
                    this.emit('elected_leader');
                }
                else {
                    console.log(`Keeper ${this.keeperId} is a follower`);
                    this.emit('elected_follower');
                }
            }
            catch (error) {
                console.error('Election error:', error);
                this.emit('election_error', error);
            }
        });
    }
    checkLeadership() {
        return __awaiter(this, void 0, void 0, function* () {
            const lockKey = 'keeper:leader:lock';
            const currentLeader = yield this.redis.get(lockKey);
            if (!currentLeader && !this.isLeader) {
                // No leader, try to become one
                yield this.participateInElection();
            }
            else if (this.isLeader && currentLeader !== this.keeperId) {
                // Lost leadership
                this.handleLeadershipLoss();
            }
        });
    }
    handleLeadershipLoss() {
        console.log(`Keeper ${this.keeperId} lost leadership`);
        this.isLeader = false;
        this.emit('lost_leadership');
    }
    releaseLeadership() {
        return __awaiter(this, void 0, void 0, function* () {
            const lockKey = 'keeper:leader:lock';
            const current = yield this.redis.get(lockKey);
            if (current === this.keeperId) {
                yield this.redis.del(lockKey);
                this.isLeader = false;
            }
        });
    }
    startLeaderDuties() {
        return __awaiter(this, void 0, void 0, function* () {
            // Periodic work distribution
            setInterval(() => this.distributeWork(), 30000);
            // Initial distribution
            yield this.distributeWork();
        });
    }
    distributeWork() {
        return __awaiter(this, void 0, void 0, function* () {
            if (!this.isLeader)
                return;
            const activeKeepers = yield this.getActiveKeepers();
            const markets = yield this.getAllMarkets();
            // Distribute markets among keepers using consistent hashing
            const distribution = this.calculateWorkDistribution(markets, activeKeepers);
            // Publish distribution
            yield this.redis.hset('keeper:work:distribution', 'current', JSON.stringify(Array.from(distribution.entries())));
            yield this.redis.hset('keeper:work:distribution', 'timestamp', Date.now().toString());
            // Notify keepers
            for (const [keeperId, marketIds] of distribution) {
                yield this.redis.publish(`keeper:${keeperId}:work`, JSON.stringify({ markets: marketIds, timestamp: Date.now() }));
            }
            this.workDistribution = distribution;
            this.emit('work_distributed', {
                keeperCount: activeKeepers.length,
                marketCount: markets.length
            });
        });
    }
    calculateWorkDistribution(markets, keepers) {
        const distribution = new Map();
        // Initialize empty arrays for each keeper
        keepers.forEach(keeper => distribution.set(keeper.id, []));
        // Use consistent hashing for distribution
        markets.forEach((marketId, index) => {
            const keeperIndex = this.hashToKeeperIndex(marketId, keepers.length);
            const keeper = keepers[keeperIndex];
            const keeperWork = distribution.get(keeper.id);
            if (keeperWork) {
                keeperWork.push(marketId);
            }
        });
        return distribution;
    }
    hashToKeeperIndex(marketId, keeperCount) {
        // Simple hash function for consistent distribution
        let hash = 0;
        for (let i = 0; i < marketId.length; i++) {
            hash = ((hash << 5) - hash) + marketId.charCodeAt(i);
            hash = hash & hash; // Convert to 32-bit integer
        }
        return Math.abs(hash) % keeperCount;
    }
    subscribeToWorkAssignments() {
        return __awaiter(this, void 0, void 0, function* () {
            const channel = `keeper:${this.keeperId}:work`;
            yield this.redisSub.subscribe(channel);
            this.redisSub.on('message', (channel, message) => __awaiter(this, void 0, void 0, function* () {
                try {
                    const work = JSON.parse(message);
                    yield this.processAssignedWork(work.markets);
                }
                catch (error) {
                    console.error('Error processing work assignment:', error);
                    this.emit('work_assignment_error', error);
                }
            }));
        });
    }
    processAssignedWork(marketIds) {
        return __awaiter(this, void 0, void 0, function* () {
            console.log(`Processing ${marketIds.length} assigned markets`);
            this.currentWork = marketIds;
            this.emit('work_received', {
                marketCount: marketIds.length,
                markets: marketIds
            });
            // Update metrics
            this.metrics.queueDepth = marketIds.length;
        });
    }
    getActiveKeepers() {
        return __awaiter(this, void 0, void 0, function* () {
            const keepers = yield this.redis.hgetall('keepers:registry');
            const active = [];
            for (const [id, data] of Object.entries(keepers)) {
                const keeper = JSON.parse(data);
                const heartbeat = yield this.redis.get(`keeper:${id}:heartbeat`);
                if (heartbeat) {
                    const hb = JSON.parse(heartbeat);
                    if (Date.now() - hb.timestamp < 30000) {
                        keeper.status = 'active';
                        active.push(keeper);
                    }
                    else {
                        keeper.status = 'inactive';
                    }
                }
                else {
                    keeper.status = 'failed';
                }
            }
            return active;
        });
    }
    getAllMarkets() {
        return __awaiter(this, void 0, void 0, function* () {
            // This would fetch actual market IDs from your data source
            // For now, return mock data
            const markets = [];
            for (let i = 0; i < 1000; i++) {
                markets.push(`market_${i}`);
            }
            return markets;
        });
    }
    // Public methods for external use
    getWorkAssignment() {
        return __awaiter(this, void 0, void 0, function* () {
            return this.currentWork;
        });
    }
    reportProgress(processed, errors) {
        return __awaiter(this, void 0, void 0, function* () {
            this.metrics.processed += processed;
            this.metrics.errors += errors;
            this.metrics.queueDepth = Math.max(0, this.metrics.queueDepth - processed);
            yield this.redis.hincrby('keeper:progress', this.keeperId, processed);
            if (errors > 0) {
                yield this.redis.hincrby('keeper:errors', this.keeperId, errors);
            }
        });
    }
    addToRetryQueue(marketId, error) {
        return __awaiter(this, void 0, void 0, function* () {
            yield this.redis.lpush('keeper:retry:queue', JSON.stringify({
                marketId,
                keeperId: this.keeperId,
                error,
                timestamp: Date.now()
            }));
        });
    }
    getStatus() {
        return {
            keeperId: this.keeperId,
            isLeader: this.isLeader,
            workload: this.currentWork.length,
            metrics: this.metrics,
            uptime: process.uptime(),
        };
    }
    // Health check
    performHealthCheck() {
        return __awaiter(this, void 0, void 0, function* () {
            try {
                // Check Redis connectivity
                yield this.redis.ping();
                // Check if heartbeat is recent
                const heartbeat = yield this.redis.get(`keeper:${this.keeperId}:heartbeat`);
                if (!heartbeat)
                    return false;
                const hb = JSON.parse(heartbeat);
                return Date.now() - hb.timestamp < 30000;
            }
            catch (error) {
                return false;
            }
        });
    }
}
exports.KeeperCoordinator = KeeperCoordinator;
// Keeper recovery mechanism
class KeeperRecovery {
    constructor(redis) {
        this.redis = redis;
    }
    detectFailedKeepers() {
        return __awaiter(this, void 0, void 0, function* () {
            const keepers = yield this.redis.hgetall('keepers:registry');
            const failed = [];
            for (const [id, data] of Object.entries(keepers)) {
                const heartbeat = yield this.redis.get(`keeper:${id}:heartbeat`);
                if (!heartbeat) {
                    failed.push(id);
                    continue;
                }
                const hb = JSON.parse(heartbeat);
                if (Date.now() - hb.timestamp > 30000) {
                    failed.push(id);
                }
            }
            return failed;
        });
    }
    redistributeWork(failedKeeperId) {
        return __awaiter(this, void 0, void 0, function* () {
            // Get failed keeper's work
            const distribution = yield this.redis.hget('keeper:work:distribution', 'current');
            if (!distribution)
                return;
            const work = new Map(JSON.parse(distribution));
            const failedWork = work.get(failedKeeperId) || [];
            if (failedWork.length === 0)
                return;
            // Remove failed keeper
            work.delete(failedKeeperId);
            // Get active keepers
            const activeKeepers = Array.from(work.keys());
            if (activeKeepers.length === 0) {
                console.error('No active keepers to redistribute work to');
                return;
            }
            // Redistribute work
            let index = 0;
            for (const marketId of failedWork) {
                const targetKeeper = activeKeepers[index % activeKeepers.length];
                const targetWork = work.get(targetKeeper);
                if (targetWork) {
                    targetWork.push(marketId);
                }
                index++;
            }
            // Update distribution
            yield this.redis.hset('keeper:work:distribution', 'current', JSON.stringify(Array.from(work.entries())));
            // Notify affected keepers
            for (const [keeperId, markets] of work) {
                yield this.redis.publish(`keeper:${keeperId}:work`, JSON.stringify({ markets: markets, timestamp: Date.now() }));
            }
            console.log(`Redistributed ${failedWork.length} markets from failed keeper ${failedKeeperId}`);
        });
    }
}
exports.KeeperRecovery = KeeperRecovery;
