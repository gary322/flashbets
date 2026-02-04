import { Redis } from 'ioredis';
import { EventEmitter } from 'events';
import { KeeperCoordinator, KeeperRecovery } from './keeper_coordinator';

interface KeeperStatus {
    id: string;
    lastHeartbeat: number;
    health: 'healthy' | 'degraded' | 'failed';
    workload: number;
    errorRate: number;
    latency: number;
}

interface FailoverConfig {
    healthCheckInterval: number;
    failureThreshold: number;
    recoveryTimeout: number;
    maxConsecutiveFailures: number;
}

export class FailoverManager extends EventEmitter {
    private redis: Redis;
    private keepers: Map<string, KeeperStatus> = new Map();
    private primaryKeeper: string | null = null;
    private backupKeepers: string[] = [];
    private healthCheckInterval: NodeJS.Timeout | null = null;
    private recovery: KeeperRecovery;
    private config: FailoverConfig = {
        healthCheckInterval: 10000, // 10 seconds
        failureThreshold: 3,
        recoveryTimeout: 60000, // 1 minute
        maxConsecutiveFailures: 5,
    };
    private failureCount: Map<string, number> = new Map();

    constructor(redisUrl: string) {
        super();
        this.redis = new Redis(redisUrl);
        this.recovery = new KeeperRecovery(this.redis);
    }

    async start() {
        console.log('Starting failover manager...');
        
        // Initial keeper discovery
        await this.discoverKeepers();
        
        // Start health monitoring
        this.healthCheckInterval = setInterval(
            () => this.performHealthChecks(),
            this.config.healthCheckInterval
        );
        
        // Subscribe to keeper events
        await this.subscribeToKeeperEvents();
        
        this.emit('started');
    }

    async stop() {
        console.log('Stopping failover manager...');
        
        if (this.healthCheckInterval) {
            clearInterval(this.healthCheckInterval);
        }
        
        await this.redis.quit();
        
        this.emit('stopped');
    }

    private async discoverKeepers() {
        const keepersData = await this.redis.hgetall('keepers:registry');
        
        for (const [id, data] of Object.entries(keepersData)) {
            const keeperInfo = JSON.parse(data);
            const status: KeeperStatus = {
                id,
                lastHeartbeat: keeperInfo.lastHeartbeat,
                health: 'healthy',
                workload: keeperInfo.workload?.length || 0,
                errorRate: 0,
                latency: 0,
            };
            
            this.keepers.set(id, status);
        }
        
        // Identify primary keeper
        const leaderLock = await this.redis.get('keeper:leader:lock');
        if (leaderLock) {
            this.primaryKeeper = leaderLock;
            this.backupKeepers = Array.from(this.keepers.keys())
                .filter(id => id !== this.primaryKeeper);
        }
        
        console.log(`Discovered ${this.keepers.size} keepers`);
        console.log(`Primary: ${this.primaryKeeper}, Backups: ${this.backupKeepers.length}`);
    }

    private async performHealthChecks() {
        const now = Date.now();
        const failedKeepers: string[] = [];
        
        for (const [id, status] of this.keepers) {
            try {
                // Get latest heartbeat
                const heartbeatData = await this.redis.get(`keeper:${id}:heartbeat`);
                
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
                } else if (timeSinceHeartbeat > 15000) {
                    // Degraded if no heartbeat for 15 seconds
                    status.health = 'degraded';
                } else {
                    // Check performance metrics
                    status.errorRate = heartbeat.errors / (heartbeat.processed || 1);
                    status.latency = await this.getKeeperLatency(id);
                    
                    if (status.errorRate > 0.1 || status.latency > 5000) {
                        status.health = 'degraded';
                    } else {
                        status.health = 'healthy';
                    }
                }
                
                // Update failure count
                if (status.health === 'failed') {
                    const count = (this.failureCount.get(id) || 0) + 1;
                    this.failureCount.set(id, count);
                    
                    if (count >= this.config.maxConsecutiveFailures) {
                        await this.handlePermanentFailure(id);
                    }
                } else {
                    this.failureCount.delete(id);
                }
                
            } catch (error) {
                console.error(`Health check failed for keeper ${id}:`, error);
                status.health = 'failed';
                failedKeepers.push(id);
            }
        }
        
        // Handle failures
        for (const keeperId of failedKeepers) {
            await this.handleKeeperFailure(keeperId);
        }
        
        this.emit('health_check_completed', {
            healthy: Array.from(this.keepers.values()).filter(k => k.health === 'healthy').length,
            degraded: Array.from(this.keepers.values()).filter(k => k.health === 'degraded').length,
            failed: failedKeepers.length,
        });
    }

    private async getKeeperLatency(keeperId: string): Promise<number> {
        // Get performance metrics
        const metrics = await this.redis.hget('keeper:metrics', keeperId);
        if (!metrics) return 0;
        
        const data = JSON.parse(metrics);
        return data.averageLatency || 0;
    }

    private async handleKeeperFailure(keeperId: string) {
        console.log(`Handling failure for keeper ${keeperId}`);
        
        if (keeperId === this.primaryKeeper) {
            await this.handlePrimaryFailure();
        } else {
            await this.handleBackupFailure(keeperId);
        }
        
        // Redistribute work
        await this.recovery.redistributeWork(keeperId);
        
        this.emit('keeper_failed', { keeperId });
    }

    private async handlePrimaryFailure() {
        console.log('Primary keeper failed, initiating failover...');
        
        // Select new primary from healthy backups
        const newPrimary = await this.selectHealthiestKeeper();
        
        if (!newPrimary) {
            console.error('No healthy backup keepers available!');
            this.emit('critical_failure', { message: 'No healthy keepers available' });
            return;
        }
        
        // Promote backup to primary
        await this.promoteKeeper(newPrimary);
        
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
    }

    private async handleBackupFailure(keeperId: string) {
        console.log(`Backup keeper ${keeperId} failed`);
        
        // Remove from backup list
        this.backupKeepers = this.backupKeepers.filter(id => id !== keeperId);
        
        // Try to recover the keeper
        setTimeout(() => this.attemptKeeperRecovery(keeperId), this.config.recoveryTimeout);
    }

    private async selectHealthiestKeeper(): Promise<string | null> {
        let bestKeeper: string | null = null;
        let bestScore = -1;
        
        for (const [id, status] of this.keepers) {
            if (status.health !== 'healthy') continue;
            
            // Calculate health score
            const score = this.calculateHealthScore(status);
            
            if (score > bestScore) {
                bestScore = score;
                bestKeeper = id;
            }
        }
        
        return bestKeeper;
    }

    private calculateHealthScore(status: KeeperStatus): number {
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

    private async promoteKeeper(keeperId: string) {
        console.log(`Promoting keeper ${keeperId} to primary`);
        
        // Force leader election
        await this.redis.set(
            'keeper:leader:lock',
            keeperId,
            'PX',
            30000,
            'XX' // Only set if exists
        );
        
        // Notify keeper of promotion
        await this.redis.publish(
            `keeper:${keeperId}:control`,
            JSON.stringify({ command: 'become_leader' })
        );
    }

    private async attemptKeeperRecovery(keeperId: string) {
        console.log(`Attempting to recover keeper ${keeperId}`);
        
        // Check if keeper is back online
        const heartbeat = await this.redis.get(`keeper:${keeperId}:heartbeat`);
        
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
    }

    private async handlePermanentFailure(keeperId: string) {
        console.log(`Keeper ${keeperId} permanently failed`);
        
        // Remove from all systems
        await this.redis.hdel('keepers:registry', keeperId);
        await this.redis.del(`keeper:${keeperId}:heartbeat`);
        
        // Remove from local tracking
        this.keepers.delete(keeperId);
        this.backupKeepers = this.backupKeepers.filter(id => id !== keeperId);
        
        this.emit('keeper_removed', { keeperId });
    }

    private async subscribeToKeeperEvents() {
        const subscriber = this.redis.duplicate();
        
        await subscriber.subscribe('keeper:events');
        
        subscriber.on('message', async (channel, message) => {
            try {
                const event = JSON.parse(message);
                
                switch (event.type) {
                    case 'keeper_joined':
                        await this.handleKeeperJoined(event.keeperId);
                        break;
                    case 'keeper_left':
                        await this.handleKeeperLeft(event.keeperId);
                        break;
                    case 'health_degraded':
                        await this.handleHealthDegraded(event.keeperId);
                        break;
                }
            } catch (error) {
                console.error('Error handling keeper event:', error);
            }
        });
    }

    private async handleKeeperJoined(keeperId: string) {
        console.log(`New keeper joined: ${keeperId}`);
        
        const status: KeeperStatus = {
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
    }

    private async handleKeeperLeft(keeperId: string) {
        console.log(`Keeper left: ${keeperId}`);
        
        await this.handleKeeperFailure(keeperId);
    }

    private async handleHealthDegraded(keeperId: string) {
        const status = this.keepers.get(keeperId);
        if (status) {
            status.health = 'degraded';
        }
        
        this.emit('keeper_degraded', { keeperId });
    }

    // Manual failover trigger
    async triggerManualFailover(targetKeeperId?: string) {
        console.log('Manual failover triggered');
        
        if (targetKeeperId) {
            // Failover to specific keeper
            const status = this.keepers.get(targetKeeperId);
            if (!status || status.health !== 'healthy') {
                throw new Error(`Target keeper ${targetKeeperId} is not healthy`);
            }
            
            await this.promoteKeeper(targetKeeperId);
        } else {
            // Auto-select best keeper
            await this.handlePrimaryFailure();
        }
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