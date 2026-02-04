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
const keeper_coordinator_1 = require("../src/keeper_coordinator");
const ioredis_1 = require("ioredis");
// Mock Redis for testing
const REDIS_URL = process.env.REDIS_URL || 'redis://localhost:6379';
(0, globals_1.describe)('KeeperCoordinator', () => {
    let coordinator1;
    let coordinator2;
    let coordinator3;
    let redis;
    (0, globals_1.beforeEach)(() => __awaiter(void 0, void 0, void 0, function* () {
        redis = new ioredis_1.Redis(REDIS_URL);
        // Clear Redis state
        yield redis.flushdb();
        // Create 3 keeper coordinators
        coordinator1 = new keeper_coordinator_1.KeeperCoordinator('keeper1', REDIS_URL);
        coordinator2 = new keeper_coordinator_1.KeeperCoordinator('keeper2', REDIS_URL);
        coordinator3 = new keeper_coordinator_1.KeeperCoordinator('keeper3', REDIS_URL);
    }));
    (0, globals_1.afterEach)(() => __awaiter(void 0, void 0, void 0, function* () {
        // Stop coordinators
        yield coordinator1.stop();
        yield coordinator2.stop();
        yield coordinator3.stop();
        // Close Redis
        yield redis.quit();
    }));
    (0, globals_1.it)('should elect a leader', () => __awaiter(void 0, void 0, void 0, function* () {
        // Start all coordinators
        yield Promise.all([
            coordinator1.start(),
            coordinator2.start(),
            coordinator3.start(),
        ]);
        // Wait for election
        yield new Promise(resolve => setTimeout(resolve, 1000));
        // Check that exactly one is leader
        const leaders = [
            coordinator1.getStatus().isLeader,
            coordinator2.getStatus().isLeader,
            coordinator3.getStatus().isLeader,
        ].filter(Boolean);
        (0, globals_1.expect)(leaders).toHaveLength(1);
    }));
    (0, globals_1.it)('should maintain leader lock', () => __awaiter(void 0, void 0, void 0, function* () {
        yield coordinator1.start();
        // Wait for election
        yield new Promise(resolve => setTimeout(resolve, 500));
        const status1 = coordinator1.getStatus();
        (0, globals_1.expect)(status1.isLeader).toBe(true);
        // Try to start another coordinator
        yield coordinator2.start();
        yield new Promise(resolve => setTimeout(resolve, 500));
        const status2 = coordinator2.getStatus();
        (0, globals_1.expect)(status2.isLeader).toBe(false);
        // Original should still be leader
        (0, globals_1.expect)(coordinator1.getStatus().isLeader).toBe(true);
    }));
    (0, globals_1.it)('should handle leader failure', () => __awaiter(void 0, void 0, void 0, function* () {
        // Start coordinators
        yield coordinator1.start();
        yield coordinator2.start();
        yield coordinator3.start();
        yield new Promise(resolve => setTimeout(resolve, 1000));
        // Find the leader
        let leader = null;
        if (coordinator1.getStatus().isLeader)
            leader = coordinator1;
        else if (coordinator2.getStatus().isLeader)
            leader = coordinator2;
        else if (coordinator3.getStatus().isLeader)
            leader = coordinator3;
        (0, globals_1.expect)(leader).not.toBeNull();
        // Stop the leader
        yield leader.stop();
        // Wait for new election
        yield new Promise(resolve => setTimeout(resolve, 35000)); // Wait for lock expiry
        // Check that a new leader is elected
        const newLeaders = [
            coordinator2.getStatus().isLeader,
            coordinator3.getStatus().isLeader,
        ].filter(Boolean);
        (0, globals_1.expect)(newLeaders).toHaveLength(1);
    }), 40000); // Extended timeout
    (0, globals_1.it)('should distribute work evenly', () => __awaiter(void 0, void 0, void 0, function* () {
        // Mock market data
        const markets = [];
        for (let i = 0; i < 100; i++) {
            markets.push(`market_${i}`);
        }
        // Start coordinators
        yield Promise.all([
            coordinator1.start(),
            coordinator2.start(),
            coordinator3.start(),
        ]);
        yield new Promise(resolve => setTimeout(resolve, 2000));
        // Get work distribution
        const distribution = yield redis.hget('keeper:work:distribution', 'current');
        (0, globals_1.expect)(distribution).not.toBeNull();
        const parsed = JSON.parse(distribution);
        (0, globals_1.expect)(parsed).toHaveLength(3);
        // Check even distribution
        const counts = parsed.map(([_, markets]) => markets.length);
        const avg = counts.reduce((a, b) => a + b) / counts.length;
        counts.forEach((count) => {
            (0, globals_1.expect)(Math.abs(count - avg)).toBeLessThan(avg * 0.2); // Within 20% of average
        });
    }));
    (0, globals_1.it)('should send heartbeats', () => __awaiter(void 0, void 0, void 0, function* () {
        yield coordinator1.start();
        // Initial heartbeat
        const heartbeat1 = yield redis.get('keeper:keeper1:heartbeat');
        (0, globals_1.expect)(heartbeat1).not.toBeNull();
        const hb1 = JSON.parse(heartbeat1);
        const timestamp1 = hb1.timestamp;
        // Wait for next heartbeat
        yield new Promise(resolve => setTimeout(resolve, 6000));
        const heartbeat2 = yield redis.get('keeper:keeper1:heartbeat');
        const hb2 = JSON.parse(heartbeat2);
        const timestamp2 = hb2.timestamp;
        // Timestamp should be updated
        (0, globals_1.expect)(timestamp2).toBeGreaterThan(timestamp1);
    }));
    (0, globals_1.it)('should handle work assignment', () => __awaiter(void 0, void 0, void 0, function* () {
        const workReceived = new Promise((resolve) => {
            coordinator1.on('work_received', (data) => {
                resolve(data.markets);
            });
        });
        yield coordinator1.start();
        // Publish work assignment
        yield redis.publish('keeper:keeper1:work', JSON.stringify({
            markets: ['market_1', 'market_2', 'market_3'],
            timestamp: Date.now()
        }));
        const markets = yield workReceived;
        (0, globals_1.expect)(markets).toEqual(['market_1', 'market_2', 'market_3']);
        // Check work assignment is stored
        const assignment = yield coordinator1.getWorkAssignment();
        (0, globals_1.expect)(assignment).toEqual(['market_1', 'market_2', 'market_3']);
    }));
    (0, globals_1.it)('should report progress', () => __awaiter(void 0, void 0, void 0, function* () {
        yield coordinator1.start();
        // Report some progress
        yield coordinator1.reportProgress(10, 1);
        const progress = yield redis.hget('keeper:progress', 'keeper1');
        (0, globals_1.expect)(progress).toBe('10');
        const errors = yield redis.hget('keeper:errors', 'keeper1');
        (0, globals_1.expect)(errors).toBe('1');
        // Check internal metrics
        const status = coordinator1.getStatus();
        (0, globals_1.expect)(status.metrics.processed).toBe(10);
        (0, globals_1.expect)(status.metrics.errors).toBe(1);
    }));
    (0, globals_1.it)('should handle retry queue', () => __awaiter(void 0, void 0, void 0, function* () {
        yield coordinator1.start();
        // Add to retry queue
        yield coordinator1.addToRetryQueue('market_123', 'Connection timeout');
        // Check retry queue
        const retryItem = yield redis.lpop('keeper:retry:queue');
        (0, globals_1.expect)(retryItem).not.toBeNull();
        const parsed = JSON.parse(retryItem);
        (0, globals_1.expect)(parsed.marketId).toBe('market_123');
        (0, globals_1.expect)(parsed.keeperId).toBe('keeper1');
        (0, globals_1.expect)(parsed.error).toBe('Connection timeout');
    }));
    (0, globals_1.it)('should perform health check', () => __awaiter(void 0, void 0, void 0, function* () {
        yield coordinator1.start();
        // Should be healthy initially
        const healthy = yield coordinator1.performHealthCheck();
        (0, globals_1.expect)(healthy).toBe(true);
        // Delete heartbeat to simulate unhealthy state
        yield redis.del('keeper:keeper1:heartbeat');
        const unhealthy = yield coordinator1.performHealthCheck();
        (0, globals_1.expect)(unhealthy).toBe(false);
    }));
});
(0, globals_1.describe)('KeeperRecovery', () => {
    let recovery;
    let redis;
    (0, globals_1.beforeEach)(() => __awaiter(void 0, void 0, void 0, function* () {
        redis = new ioredis_1.Redis(REDIS_URL);
        yield redis.flushdb();
        recovery = new keeper_coordinator_1.KeeperRecovery(redis);
    }));
    (0, globals_1.afterEach)(() => __awaiter(void 0, void 0, void 0, function* () {
        yield redis.quit();
    }));
    (0, globals_1.it)('should detect failed keepers', () => __awaiter(void 0, void 0, void 0, function* () {
        // Register some keepers
        yield redis.hset('keepers:registry', 'keeper1', JSON.stringify({
            id: 'keeper1',
            lastHeartbeat: Date.now(),
        }));
        yield redis.hset('keepers:registry', 'keeper2', JSON.stringify({
            id: 'keeper2',
            lastHeartbeat: Date.now() - 60000, // 1 minute ago
        }));
        // Set heartbeats
        yield redis.setex('keeper:keeper1:heartbeat', 30, JSON.stringify({
            timestamp: Date.now()
        }));
        // keeper2 has no recent heartbeat
        const failed = yield recovery.detectFailedKeepers();
        (0, globals_1.expect)(failed).toContain('keeper2');
        (0, globals_1.expect)(failed).not.toContain('keeper1');
    }));
    (0, globals_1.it)('should redistribute work from failed keeper', () => __awaiter(void 0, void 0, void 0, function* () {
        // Set up work distribution
        const distribution = [
            ['keeper1', ['market_1', 'market_2']],
            ['keeper2', ['market_3', 'market_4', 'market_5']],
            ['keeper3', ['market_6', 'market_7']],
        ];
        yield redis.hset('keeper:work:distribution', 'current', JSON.stringify(distribution));
        // Redistribute work from keeper2
        yield recovery.redistributeWork('keeper2');
        // Check new distribution
        const newDist = yield redis.hget('keeper:work:distribution', 'current');
        const parsed = JSON.parse(newDist);
        // keeper2 should be removed
        (0, globals_1.expect)(parsed.find(([id]) => id === 'keeper2')).toBeUndefined();
        // keeper2's markets should be redistributed
        const allMarkets = parsed.flatMap(([_, markets]) => markets);
        (0, globals_1.expect)(allMarkets).toContain('market_3');
        (0, globals_1.expect)(allMarkets).toContain('market_4');
        (0, globals_1.expect)(allMarkets).toContain('market_5');
    }));
});
(0, globals_1.describe)('Leader Election Edge Cases', () => {
    let coordinators = [];
    let redis;
    (0, globals_1.beforeEach)(() => __awaiter(void 0, void 0, void 0, function* () {
        redis = new ioredis_1.Redis(REDIS_URL);
        yield redis.flushdb();
    }));
    (0, globals_1.afterEach)(() => __awaiter(void 0, void 0, void 0, function* () {
        for (const coordinator of coordinators) {
            yield coordinator.stop();
        }
        yield redis.quit();
    }));
    (0, globals_1.it)('should handle simultaneous starts', () => __awaiter(void 0, void 0, void 0, function* () {
        // Create 5 coordinators
        for (let i = 0; i < 5; i++) {
            coordinators.push(new keeper_coordinator_1.KeeperCoordinator(`keeper${i}`, REDIS_URL));
        }
        // Start all simultaneously
        yield Promise.all(coordinators.map(c => c.start()));
        yield new Promise(resolve => setTimeout(resolve, 1000));
        // Exactly one should be leader
        const leaders = coordinators.filter(c => c.getStatus().isLeader);
        (0, globals_1.expect)(leaders).toHaveLength(1);
    }));
    (0, globals_1.it)('should handle rapid leader changes', () => __awaiter(void 0, void 0, void 0, function* () {
        const coordinator1 = new keeper_coordinator_1.KeeperCoordinator('keeper1', REDIS_URL);
        const coordinator2 = new keeper_coordinator_1.KeeperCoordinator('keeper2', REDIS_URL);
        coordinators = [coordinator1, coordinator2];
        // Start first coordinator
        yield coordinator1.start();
        yield new Promise(resolve => setTimeout(resolve, 500));
        (0, globals_1.expect)(coordinator1.getStatus().isLeader).toBe(true);
        // Force leadership change by manipulating Redis
        yield redis.del('keeper:leader:lock');
        // Start second coordinator
        yield coordinator2.start();
        yield new Promise(resolve => setTimeout(resolve, 500));
        // One of them should claim leadership
        const leaders = [
            coordinator1.getStatus().isLeader,
            coordinator2.getStatus().isLeader,
        ].filter(Boolean);
        (0, globals_1.expect)(leaders).toHaveLength(1);
    }));
});
