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
exports.FailoverManager = void 0;
const ioredis_1 = require("ioredis");
const events_1 = require("events");
const keeper_coordinator_1 = require("./keeper_coordinator");
class FailoverManager extends events_1.EventEmitter {
    constructor(redisUrl) {
        super();
        this.keepers = new Map();
        this.primaryKeeper = null;
        this.backupKeepers = [];
        this.healthCheckInterval = null;
        this.config = {
            healthCheckInterval: 10000, // 10 seconds
            failureThreshold: 3,
            recoveryTimeout: 60000, // 1 minute
            maxConsecutiveFailures: 5,
        };
        this.failureCount = new Map();
        this.redis = new ioredis_1.Redis(redisUrl);
        this.recovery = new keeper_coordinator_1.KeeperRecovery(this.redis);
    }
    start() {
        return __awaiter(this, void 0, void 0, function* () {
            console.log('Starting failover manager...');
            // Initial keeper discovery
            yield this.discoverKeepers();
            // Start health monitoring
            this.healthCheckInterval = setInterval(() => this.performHealthChecks(), this.config.healthCheckInterval);
            // Subscribe to keeper events
            yield this.subscribeToKeeperEvents();
            this.emit('started');
        });
    }
    stop() {
        return __awaiter(this, void 0, void 0, function* () {
            console.log('Stopping failover manager...');
            if (this.healthCheckInterval) {
                clearInterval(this.healthCheckInterval);
            }
            yield this.redis.quit();
            this.emit('stopped');
        });
    }
    discoverKeepers() {
        return __awaiter(this, void 0, void 0, function* () {
            var _a;
            const keepersData = yield this.redis.hgetall('keepers:registry');
            for (const [id, data] of Object.entries(keepersData)) {
                const keeperInfo = JSON.parse(data);
                const status = {
                    id,
                    lastHeartbeat: keeperInfo.lastHeartbeat,
                    health: 'healthy',
                    workload: ((_a = keeperInfo.workload) === null || _a === void 0 ? void 0 : _a.length) || 0,
                    errorRate: 0,
                    latency: 0,
                };
                this.keepers.set(id, status);
            }
            // Identify primary keeper
            const leaderLock = yield this.redis.get('keeper:leader:lock');
            if (leaderLock) {
                this.primaryKeeper = leaderLock;
                this.backupKeepers = Array.from(this.keepers.keys())
                    .filter(id => id !== this.primaryKeeper);
            }
            console.log(`Discovered ${this.keepers.size} keepers`);
            console.log(`Primary: ${this.primaryKeeper}, Backups: ${this.backupKeepers.length}`);
        });
    }
    performHealthChecks() {
        return __awaiter(this, void 0, void 0, function* () {
            const now = Date.now();
            const failedKeepers = [];
            for (const [id, status] of this.keepers) {
                try {
                    // Get latest heartbeat
                    const heartbeatData = yield this.redis.get(`keeper:${id}:heartbeat`);
                    if (!heartbeatData) {
                        status.health = 'failed';
                        failedKeepers.push(id);
                        continue;
                    }
                    const heartbeat = JSON.parse(heartbeatData);
                    status.lastHeartbeat = heartbeat.timestamp;
                    // Calculate health metrics
                    const timeSinceHeartbeat = now - heartbeat.timestamp;
                    if (timeSinceHeartbeat > 30000) {
                        // No heartbeat for 30 seconds
                        status.health = 'failed';
                        failedKeepers.push(id);
                    }
                    else if (timeSinceHeartbeat > 15000) {
                        // Degraded if no heartbeat for 15 seconds
                        status.health = 'degraded';
                    }
                    else {
                        // Check performance metrics
                        status.errorRate = heartbeat.errors / (heartbeat.processed || 1);
                        status.latency = yield this.getKeeperLatency(id);
                        if (status.errorRate > 0.1 || status.latency > 5000) {
                            status.health = 'degraded';
                        }
                        else {
                            status.health = 'healthy';
                        }
                    }
                    // Update failure count
                    if (status.health === 'failed') {
                        const count = (this.failureCount.get(id) || 0) + 1;
                        this.failureCount.set(id, count);
                        if (count >= this.config.maxConsecutiveFailures) {
                            yield this.handlePermanentFailure(id);
                        }
                    }
                    else {
                        this.failureCount.delete(id);
                    }
                }
                catch (error) {
                    console.error(`Health check failed for keeper ${id}:`, error);
                    status.health = 'failed';
                    failedKeepers.push(id);
                }
            }
            // Handle failures
            for (const keeperId of failedKeepers) {
                yield this.handleKeeperFailure(keeperId);
            }
            this.emit('health_check_completed', {
                healthy: Array.from(this.keepers.values()).filter(k => k.health === 'healthy').length,
                degraded: Array.from(this.keepers.values()).filter(k => k.health === 'degraded').length,
                failed: failedKeepers.length,
            });
        });
    }
    getKeeperLatency(keeperId) {
        return __awaiter(this, void 0, void 0, function* () {
            // Get performance metrics
            const metrics = yield this.redis.hget('keeper:metrics', keeperId);
            if (!metrics)
                return 0;
            const data = JSON.parse(metrics);
            return data.averageLatency || 0;
        });
    }
    handleKeeperFailure(keeperId) {
        return __awaiter(this, void 0, void 0, function* () {
            console.log(`Handling failure for keeper ${keeperId}`);
            if (keeperId === this.primaryKeeper) {
                yield this.handlePrimaryFailure();
            }
            else {
                yield this.handleBackupFailure(keeperId);
            }
            // Redistribute work
            yield this.recovery.redistributeWork(keeperId);
            this.emit('keeper_failed', { keeperId });
        });
    }
    handlePrimaryFailure() {
        return __awaiter(this, void 0, void 0, function* () {
            console.log('Primary keeper failed, initiating failover...');
            // Select new primary from healthy backups
            const newPrimary = yield this.selectHealthiestKeeper();
            if (!newPrimary) {
                console.error('No healthy backup keepers available!');
                this.emit('critical_failure', { message: 'No healthy keepers available' });
                return;
            }
            // Promote backup to primary
            yield this.promoteKeeper(newPrimary);
            // Update local state
            this.backupKeepers = this.backupKeepers.filter(id => id !== newPrimary);
            if (this.primaryKeeper) {
                this.backupKeepers.push(this.primaryKeeper);
            }
            this.primaryKeeper = newPrimary;
            this.emit('failover_completed', {
                oldPrimary: this.primaryKeeper,
                newPrimary,
            });
        });
    }
    handleBackupFailure(keeperId) {
        return __awaiter(this, void 0, void 0, function* () {
            console.log(`Backup keeper ${keeperId} failed`);
            // Remove from backup list
            this.backupKeepers = this.backupKeepers.filter(id => id !== keeperId);
            // Try to recover the keeper
            setTimeout(() => this.attemptKeeperRecovery(keeperId), this.config.recoveryTimeout);
        });
    }
    selectHealthiestKeeper() {
        return __awaiter(this, void 0, void 0, function* () {
            let bestKeeper = null;
            let bestScore = -1;
            for (const [id, status] of this.keepers) {
                if (status.health !== 'healthy')
                    continue;
                // Calculate health score
                const score = this.calculateHealthScore(status);
                if (score > bestScore) {
                    bestScore = score;
                    bestKeeper = id;
                }
            }
            return bestKeeper;
        });
    }
    calculateHealthScore(status) {
        // Higher score is better
        let score = 100;
        // Penalize for errors
        score -= status.errorRate * 100;
        // Penalize for high latency
        score -= Math.min(50, status.latency / 100);
        // Penalize for high workload
        score -= Math.min(20, status.workload / 10);
        return Math.max(0, score);
    }
    promoteKeeper(keeperId) {
        return __awaiter(this, void 0, void 0, function* () {
            console.log(`Promoting keeper ${keeperId} to primary`);
            // Force leader election
            yield this.redis.set('keeper:leader:lock', keeperId, 'PX', 30000, 'XX' // Only set if exists
            );
            // Notify keeper of promotion
            yield this.redis.publish(`keeper:${keeperId}:control`, JSON.stringify({ command: 'become_leader' }));
        });
    }
    attemptKeeperRecovery(keeperId) {
        return __awaiter(this, void 0, void 0, function* () {
            console.log(`Attempting to recover keeper ${keeperId}`);
            // Check if keeper is back online
            const heartbeat = yield this.redis.get(`keeper:${keeperId}:heartbeat`);
            if (heartbeat) {
                const hb = JSON.parse(heartbeat);
                if (Date.now() - hb.timestamp < 30000) {
                    // Keeper is back!
                    console.log(`Keeper ${keeperId} recovered`);
                    const status = this.keepers.get(keeperId);
                    if (status) {
                        status.health = 'healthy';
                        this.backupKeepers.push(keeperId);
                    }
                    this.emit('keeper_recovered', { keeperId });
                    return;
                }
            }
            // Still failed, remove from tracking
            this.keepers.delete(keeperId);
        });
    }
    handlePermanentFailure(keeperId) {
        return __awaiter(this, void 0, void 0, function* () {
            console.log(`Keeper ${keeperId} permanently failed`);
            // Remove from all systems
            yield this.redis.hdel('keepers:registry', keeperId);
            yield this.redis.del(`keeper:${keeperId}:heartbeat`);
            // Remove from local tracking
            this.keepers.delete(keeperId);
            this.backupKeepers = this.backupKeepers.filter(id => id !== keeperId);
            this.emit('keeper_removed', { keeperId });
        });
    }
    subscribeToKeeperEvents() {
        return __awaiter(this, void 0, void 0, function* () {
            const subscriber = this.redis.duplicate();
            yield subscriber.subscribe('keeper:events');
            subscriber.on('message', (channel, message) => __awaiter(this, void 0, void 0, function* () {
                try {
                    const event = JSON.parse(message);
                    switch (event.type) {
                        case 'keeper_joined':
                            yield this.handleKeeperJoined(event.keeperId);
                            break;
                        case 'keeper_left':
                            yield this.handleKeeperLeft(event.keeperId);
                            break;
                        case 'health_degraded':
                            yield this.handleHealthDegraded(event.keeperId);
                            break;
                    }
                }
                catch (error) {
                    console.error('Error handling keeper event:', error);
                }
            }));
        });
    }
    handleKeeperJoined(keeperId) {
        return __awaiter(this, void 0, void 0, function* () {
            console.log(`New keeper joined: ${keeperId}`);
            const status = {
                id: keeperId,
                lastHeartbeat: Date.now(),
                health: 'healthy',
                workload: 0,
                errorRate: 0,
                latency: 0,
            };
            this.keepers.set(keeperId, status);
            this.backupKeepers.push(keeperId);
            this.emit('keeper_joined', { keeperId });
        });
    }
    handleKeeperLeft(keeperId) {
        return __awaiter(this, void 0, void 0, function* () {
            console.log(`Keeper left: ${keeperId}`);
            yield this.handleKeeperFailure(keeperId);
        });
    }
    handleHealthDegraded(keeperId) {
        return __awaiter(this, void 0, void 0, function* () {
            const status = this.keepers.get(keeperId);
            if (status) {
                status.health = 'degraded';
            }
            this.emit('keeper_degraded', { keeperId });
        });
    }
    // Manual failover trigger
    triggerManualFailover(targetKeeperId) {
        return __awaiter(this, void 0, void 0, function* () {
            console.log('Manual failover triggered');
            if (targetKeeperId) {
                // Failover to specific keeper
                const status = this.keepers.get(targetKeeperId);
                if (!status || status.health !== 'healthy') {
                    throw new Error(`Target keeper ${targetKeeperId} is not healthy`);
                }
                yield this.promoteKeeper(targetKeeperId);
            }
            else {
                // Auto-select best keeper
                yield this.handlePrimaryFailure();
            }
        });
    }
    // Get current status
    getStatus() {
        const keeperStatuses = Array.from(this.keepers.values());
        return {
            primary: this.primaryKeeper,
            backups: this.backupKeepers,
            keepers: keeperStatuses,
            summary: {
                total: keeperStatuses.length,
                healthy: keeperStatuses.filter(k => k.health === 'healthy').length,
                degraded: keeperStatuses.filter(k => k.health === 'degraded').length,
                failed: keeperStatuses.filter(k => k.health === 'failed').length,
            },
        };
    }
}
exports.FailoverManager = FailoverManager;
