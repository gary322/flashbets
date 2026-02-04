import { describe, it, expect, beforeEach, afterEach } from '@jest/globals';
import { KeeperCoordinator, KeeperRecovery } from '../src/keeper_coordinator';
import { Redis } from 'ioredis';

// Mock Redis for testing
const REDIS_URL = process.env.REDIS_URL || 'redis://localhost:6379';

describe('KeeperCoordinator', () => {
    let coordinator1: KeeperCoordinator;
    let coordinator2: KeeperCoordinator;
    let coordinator3: KeeperCoordinator;
    let redis: Redis;

    beforeEach(async () => {
        redis = new Redis(REDIS_URL);
        
        // Clear Redis state
        await redis.flushdb();
        
        // Create 3 keeper coordinators
        coordinator1 = new KeeperCoordinator('keeper1', REDIS_URL);
        coordinator2 = new KeeperCoordinator('keeper2', REDIS_URL);
        coordinator3 = new KeeperCoordinator('keeper3', REDIS_URL);
    });

    afterEach(async () => {
        // Stop coordinators
        await coordinator1.stop();
        await coordinator2.stop();
        await coordinator3.stop();
        
        // Close Redis
        await redis.quit();
    });

    it('should elect a leader', async () => {
        // Start all coordinators
        await Promise.all([
            coordinator1.start(),
            coordinator2.start(),
            coordinator3.start(),
        ]);

        // Wait for election
        await new Promise(resolve => setTimeout(resolve, 1000));

        // Check that exactly one is leader
        const leaders = [
            coordinator1.getStatus().isLeader,
            coordinator2.getStatus().isLeader,
            coordinator3.getStatus().isLeader,
        ].filter(Boolean);

        expect(leaders).toHaveLength(1);
    });

    it('should maintain leader lock', async () => {
        await coordinator1.start();
        
        // Wait for election
        await new Promise(resolve => setTimeout(resolve, 500));
        
        const status1 = coordinator1.getStatus();
        expect(status1.isLeader).toBe(true);
        
        // Try to start another coordinator
        await coordinator2.start();
        await new Promise(resolve => setTimeout(resolve, 500));
        
        const status2 = coordinator2.getStatus();
        expect(status2.isLeader).toBe(false);
        
        // Original should still be leader
        expect(coordinator1.getStatus().isLeader).toBe(true);
    });

    it('should handle leader failure', async () => {
        // Start coordinators
        await coordinator1.start();
        await coordinator2.start();
        await coordinator3.start();
        
        await new Promise(resolve => setTimeout(resolve, 1000));
        
        // Find the leader
        let leader: KeeperCoordinator | null = null;
        if (coordinator1.getStatus().isLeader) leader = coordinator1;
        else if (coordinator2.getStatus().isLeader) leader = coordinator2;
        else if (coordinator3.getStatus().isLeader) leader = coordinator3;
        
        expect(leader).not.toBeNull();
        
        // Stop the leader
        await leader!.stop();
        
        // Wait for new election
        await new Promise(resolve => setTimeout(resolve, 35000)); // Wait for lock expiry
        
        // Check that a new leader is elected
        const newLeaders = [
            coordinator2.getStatus().isLeader,
            coordinator3.getStatus().isLeader,
        ].filter(Boolean);
        
        expect(newLeaders).toHaveLength(1);
    }, 40000); // Extended timeout

    it('should distribute work evenly', async () => {
        // Mock market data
        const markets = [];
        for (let i = 0; i < 100; i++) {
            markets.push(`market_${i}`);
        }
        
        // Start coordinators
        await Promise.all([
            coordinator1.start(),
            coordinator2.start(),
            coordinator3.start(),
        ]);
        
        await new Promise(resolve => setTimeout(resolve, 2000));
        
        // Get work distribution
        const distribution = await redis.hget('keeper:work:distribution', 'current');
        expect(distribution).not.toBeNull();
        
        const parsed = JSON.parse(distribution!);
        expect(parsed).toHaveLength(3);
        
        // Check even distribution
        const counts = parsed.map(([_, markets]: [string, string[]]) => markets.length);
        const avg = counts.reduce((a: number, b: number) => a + b) / counts.length;
        
        counts.forEach((count: number) => {
            expect(Math.abs(count - avg)).toBeLessThan(avg * 0.2); // Within 20% of average
        });
    });

    it('should send heartbeats', async () => {
        await coordinator1.start();
        
        // Initial heartbeat
        const heartbeat1 = await redis.get('keeper:keeper1:heartbeat');
        expect(heartbeat1).not.toBeNull();
        
        const hb1 = JSON.parse(heartbeat1!);
        const timestamp1 = hb1.timestamp;
        
        // Wait for next heartbeat
        await new Promise(resolve => setTimeout(resolve, 6000));
        
        const heartbeat2 = await redis.get('keeper:keeper1:heartbeat');
        const hb2 = JSON.parse(heartbeat2!);
        const timestamp2 = hb2.timestamp;
        
        // Timestamp should be updated
        expect(timestamp2).toBeGreaterThan(timestamp1);
    });

    it('should handle work assignment', async () => {
        const workReceived = new Promise<string[]>((resolve) => {
            coordinator1.on('work_received', (data) => {
                resolve(data.markets);
            });
        });
        
        await coordinator1.start();
        
        // Publish work assignment
        await redis.publish(
            'keeper:keeper1:work',
            JSON.stringify({ 
                markets: ['market_1', 'market_2', 'market_3'],
                timestamp: Date.now()
            })
        );
        
        const markets = await workReceived;
        expect(markets).toEqual(['market_1', 'market_2', 'market_3']);
        
        // Check work assignment is stored
        const assignment = await coordinator1.getWorkAssignment();
        expect(assignment).toEqual(['market_1', 'market_2', 'market_3']);
    });

    it('should report progress', async () => {
        await coordinator1.start();
        
        // Report some progress
        await coordinator1.reportProgress(10, 1);
        
        const progress = await redis.hget('keeper:progress', 'keeper1');
        expect(progress).toBe('10');
        
        const errors = await redis.hget('keeper:errors', 'keeper1');
        expect(errors).toBe('1');
        
        // Check internal metrics
        const status = coordinator1.getStatus();
        expect(status.metrics.processed).toBe(10);
        expect(status.metrics.errors).toBe(1);
    });

    it('should handle retry queue', async () => {
        await coordinator1.start();
        
        // Add to retry queue
        await coordinator1.addToRetryQueue('market_123', 'Connection timeout');
        
        // Check retry queue
        const retryItem = await redis.lpop('keeper:retry:queue');
        expect(retryItem).not.toBeNull();
        
        const parsed = JSON.parse(retryItem!);
        expect(parsed.marketId).toBe('market_123');
        expect(parsed.keeperId).toBe('keeper1');
        expect(parsed.error).toBe('Connection timeout');
    });

    it('should perform health check', async () => {
        await coordinator1.start();
        
        // Should be healthy initially
        const healthy = await coordinator1.performHealthCheck();
        expect(healthy).toBe(true);
        
        // Delete heartbeat to simulate unhealthy state
        await redis.del('keeper:keeper1:heartbeat');
        
        const unhealthy = await coordinator1.performHealthCheck();
        expect(unhealthy).toBe(false);
    });
});

describe('KeeperRecovery', () => {
    let recovery: KeeperRecovery;
    let redis: Redis;

    beforeEach(async () => {
        redis = new Redis(REDIS_URL);
        await redis.flushdb();
        recovery = new KeeperRecovery(redis);
    });

    afterEach(async () => {
        await redis.quit();
    });

    it('should detect failed keepers', async () => {
        // Register some keepers
        await redis.hset('keepers:registry', 'keeper1', JSON.stringify({
            id: 'keeper1',
            lastHeartbeat: Date.now(),
        }));
        await redis.hset('keepers:registry', 'keeper2', JSON.stringify({
            id: 'keeper2',
            lastHeartbeat: Date.now() - 60000, // 1 minute ago
        }));
        
        // Set heartbeats
        await redis.setex('keeper:keeper1:heartbeat', 30, JSON.stringify({
            timestamp: Date.now()
        }));
        // keeper2 has no recent heartbeat
        
        const failed = await recovery.detectFailedKeepers();
        
        expect(failed).toContain('keeper2');
        expect(failed).not.toContain('keeper1');
    });

    it('should redistribute work from failed keeper', async () => {
        // Set up work distribution
        const distribution = [
            ['keeper1', ['market_1', 'market_2']],
            ['keeper2', ['market_3', 'market_4', 'market_5']],
            ['keeper3', ['market_6', 'market_7']],
        ];
        
        await redis.hset(
            'keeper:work:distribution',
            'current',
            JSON.stringify(distribution)
        );
        
        // Redistribute work from keeper2
        await recovery.redistributeWork('keeper2');
        
        // Check new distribution
        const newDist = await redis.hget('keeper:work:distribution', 'current');
        const parsed = JSON.parse(newDist!);
        
        // keeper2 should be removed
        expect(parsed.find(([id]: [string, string[]]) => id === 'keeper2')).toBeUndefined();
        
        // keeper2's markets should be redistributed
        const allMarkets = parsed.flatMap(([_, markets]: [string, string[]]) => markets);
        expect(allMarkets).toContain('market_3');
        expect(allMarkets).toContain('market_4');
        expect(allMarkets).toContain('market_5');
    });
});

describe('Leader Election Edge Cases', () => {
    let coordinators: KeeperCoordinator[] = [];
    let redis: Redis;

    beforeEach(async () => {
        redis = new Redis(REDIS_URL);
        await redis.flushdb();
    });

    afterEach(async () => {
        for (const coordinator of coordinators) {
            await coordinator.stop();
        }
        await redis.quit();
    });

    it('should handle simultaneous starts', async () => {
        // Create 5 coordinators
        for (let i = 0; i < 5; i++) {
            coordinators.push(new KeeperCoordinator(`keeper${i}`, REDIS_URL));
        }
        
        // Start all simultaneously
        await Promise.all(coordinators.map(c => c.start()));
        
        await new Promise(resolve => setTimeout(resolve, 1000));
        
        // Exactly one should be leader
        const leaders = coordinators.filter(c => c.getStatus().isLeader);
        expect(leaders).toHaveLength(1);
    });

    it('should handle rapid leader changes', async () => {
        const coordinator1 = new KeeperCoordinator('keeper1', REDIS_URL);
        const coordinator2 = new KeeperCoordinator('keeper2', REDIS_URL);
        
        coordinators = [coordinator1, coordinator2];
        
        // Start first coordinator
        await coordinator1.start();
        await new Promise(resolve => setTimeout(resolve, 500));
        expect(coordinator1.getStatus().isLeader).toBe(true);
        
        // Force leadership change by manipulating Redis
        await redis.del('keeper:leader:lock');
        
        // Start second coordinator
        await coordinator2.start();
        await new Promise(resolve => setTimeout(resolve, 500));
        
        // One of them should claim leadership
        const leaders = [
            coordinator1.getStatus().isLeader,
            coordinator2.getStatus().isLeader,
        ].filter(Boolean);
        
        expect(leaders).toHaveLength(1);
    });
});