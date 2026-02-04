import { Redis } from 'ioredis';
import { EventEmitter } from 'events';
import * as os from 'os';

interface KeeperInfo {
    id: string;
    startTime: number;
    capabilities: string[];
    status: 'active' | 'inactive' | 'failed';
    lastHeartbeat: number;
    host: string;
    workload: string[];
}

interface Heartbeat {
    timestamp: number;
    processed: number;
    errors: number;
    queueDepth: number;
    cpuUsage: any;
    memoryUsage: any;
}

interface WorkAssignment {
    markets: string[];
    timestamp: number;
}

export class KeeperCoordinator extends EventEmitter {
    private redis: Redis;
    private redisSub: Redis;
    private keeperId: string;
    private isLeader: boolean = false;
    private heartbeatInterval: NodeJS.Timeout | null = null;
    private leaderCheckInterval: NodeJS.Timeout | null = null;
    private workDistribution: Map<string, string[]> = new Map();
    private currentWork: string[] = [];
    private metrics = {
        processed: 0,
        errors: 0,
        queueDepth: 0,
    };

    constructor(keeperId: string, redisUrl: string) {
        super();
        this.keeperId = keeperId;
        this.redis = new Redis(redisUrl);
        this.redisSub = new Redis(redisUrl);
    }

    async start() {
        console.log(`Starting keeper coordinator ${this.keeperId}...`);
        
        // Register keeper
        await this.registerKeeper();

        // Start heartbeat
        this.heartbeatInterval = setInterval(
            () => this.sendHeartbeat(),
            5000
        );

        // Subscribe to work assignments
        await this.subscribeToWorkAssignments();

        // Participate in leader election
        await this.participateInElection();

        // Start leader check
        this.leaderCheckInterval = setInterval(
            () => this.checkLeadership(),
            10000
        );

        // Start work processing based on role
        if (this.isLeader) {
            await this.startLeaderDuties();
        }

        this.emit('started', { keeperId: this.keeperId, isLeader: this.isLeader });
    }

    async stop() {
        console.log(`Stopping keeper coordinator ${this.keeperId}...`);
        
        // Clear intervals
        if (this.heartbeatInterval) {
            clearInterval(this.heartbeatInterval);
        }
        if (this.leaderCheckInterval) {
            clearInterval(this.leaderCheckInterval);
        }

        // Unregister keeper
        await this.unregisterKeeper();

        // Release leader lock if held
        if (this.isLeader) {
            await this.releaseLeadership();
        }

        // Close Redis connections
        await this.redis.quit();
        await this.redisSub.quit();

        this.emit('stopped', { keeperId: this.keeperId });
    }

    private async registerKeeper() {
        const keeperInfo: KeeperInfo = {
            id: this.keeperId,
            startTime: Date.now(),
            capabilities: ['markets', 'prices', 'resolutions'],
            status: 'active',
            lastHeartbeat: Date.now(),
            host: os.hostname(),
            workload: [],
        };

        await this.redis.hset(
            'keepers:registry',
            this.keeperId,
            JSON.stringify(keeperInfo)
        );

        // Set initial heartbeat with expiry
        await this.redis.setex(
            `keeper:${this.keeperId}:heartbeat`,
            30,
            JSON.stringify({ timestamp: Date.now() })
        );
    }

    private async unregisterKeeper() {
        await this.redis.hdel('keepers:registry', this.keeperId);
        await this.redis.del(`keeper:${this.keeperId}:heartbeat`);
    }

    private async sendHeartbeat() {
        const heartbeat: Heartbeat = {
            timestamp: Date.now(),
            processed: this.metrics.processed,
            errors: this.metrics.errors,
            queueDepth: this.metrics.queueDepth,
            cpuUsage: process.cpuUsage(),
            memoryUsage: process.memoryUsage(),
        };

        await this.redis.setex(
            `keeper:${this.keeperId}:heartbeat`,
            30,
            JSON.stringify(heartbeat)
        );

        // Update keeper info
        const keeperData = await this.redis.hget('keepers:registry', this.keeperId);
        if (keeperData) {
            const keeperInfo = JSON.parse(keeperData) as KeeperInfo;
            keeperInfo.lastHeartbeat = Date.now();
            keeperInfo.workload = this.currentWork;
            await this.redis.hset(
                'keepers:registry',
                this.keeperId,
                JSON.stringify(keeperInfo)
            );
        }

        this.emit('heartbeat', heartbeat);
    }

    private async participateInElection() {
        const lockKey = 'keeper:leader:lock';
        const lockValue = this.keeperId;
        const lockTTL = 30000; // 30 seconds

        try {
            // Try to acquire leader lock
            const acquired = await this.redis.set(
                lockKey,
                lockValue,
                'PX',
                lockTTL,
                'NX'
            );

            if (acquired === 'OK') {
                this.isLeader = true;
                console.log(`Keeper ${this.keeperId} elected as leader`);
                
                // Refresh lock periodically
                setInterval(async () => {
                    const current = await this.redis.get(lockKey);
                    if (current === this.keeperId) {
                        await this.redis.pexpire(lockKey, lockTTL);
                    } else {
                        // Lost leadership
                        this.handleLeadershipLoss();
                    }
                }, lockTTL / 3);

                this.emit('elected_leader');
            } else {
                console.log(`Keeper ${this.keeperId} is a follower`);
                this.emit('elected_follower');
            }
        } catch (error) {
            console.error('Election error:', error);
            this.emit('election_error', error);
        }
    }

    private async checkLeadership() {
        const lockKey = 'keeper:leader:lock';
        const currentLeader = await this.redis.get(lockKey);

        if (!currentLeader && !this.isLeader) {
            // No leader, try to become one
            await this.participateInElection();
        } else if (this.isLeader && currentLeader !== this.keeperId) {
            // Lost leadership
            this.handleLeadershipLoss();
        }
    }

    private handleLeadershipLoss() {
        console.log(`Keeper ${this.keeperId} lost leadership`);
        this.isLeader = false;
        this.emit('lost_leadership');
    }

    private async releaseLeadership() {
        const lockKey = 'keeper:leader:lock';
        const current = await this.redis.get(lockKey);
        
        if (current === this.keeperId) {
            await this.redis.del(lockKey);
            this.isLeader = false;
        }
    }

    private async startLeaderDuties() {
        // Periodic work distribution
        setInterval(() => this.distributeWork(), 30000);
        
        // Initial distribution
        await this.distributeWork();
    }

    private async distributeWork() {
        if (!this.isLeader) return;

        const activeKeepers = await this.getActiveKeepers();
        const markets = await this.getAllMarkets();

        // Distribute markets among keepers using consistent hashing
        const distribution = this.calculateWorkDistribution(markets, activeKeepers);

        // Publish distribution
        await this.redis.hset(
            'keeper:work:distribution',
            'current',
            JSON.stringify(Array.from(distribution.entries()))
        );

        await this.redis.hset(
            'keeper:work:distribution',
            'timestamp',
            Date.now().toString()
        );

        // Notify keepers
        for (const [keeperId, marketIds] of distribution) {
            await this.redis.publish(
                `keeper:${keeperId}:work`,
                JSON.stringify({ markets: marketIds, timestamp: Date.now() })
            );
        }

        this.workDistribution = distribution;
        this.emit('work_distributed', { 
            keeperCount: activeKeepers.length, 
            marketCount: markets.length 
        });
    }

    private calculateWorkDistribution(
        markets: string[], 
        keepers: KeeperInfo[]
    ): Map<string, string[]> {
        const distribution = new Map<string, string[]>();
        
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

    private hashToKeeperIndex(marketId: string, keeperCount: number): number {
        // Simple hash function for consistent distribution
        let hash = 0;
        for (let i = 0; i < marketId.length; i++) {
            hash = ((hash << 5) - hash) + marketId.charCodeAt(i);
            hash = hash & hash; // Convert to 32-bit integer
        }
        return Math.abs(hash) % keeperCount;
    }

    private async subscribeToWorkAssignments() {
        const channel = `keeper:${this.keeperId}:work`;
        
        await this.redisSub.subscribe(channel);
        
        this.redisSub.on('message', async (channel, message) => {
            try {
                const work = JSON.parse(message) as WorkAssignment;
                await this.processAssignedWork(work.markets);
            } catch (error) {
                console.error('Error processing work assignment:', error);
                this.emit('work_assignment_error', error);
            }
        });
    }

    private async processAssignedWork(marketIds: string[]) {
        console.log(`Processing ${marketIds.length} assigned markets`);
        this.currentWork = marketIds;
        
        this.emit('work_received', { 
            marketCount: marketIds.length,
            markets: marketIds 
        });

        // Update metrics
        this.metrics.queueDepth = marketIds.length;
    }

    private async getActiveKeepers(): Promise<KeeperInfo[]> {
        const keepers = await this.redis.hgetall('keepers:registry');
        const active: KeeperInfo[] = [];

        for (const [id, data] of Object.entries(keepers)) {
            const keeper = JSON.parse(data) as KeeperInfo;
            const heartbeat = await this.redis.get(`keeper:${id}:heartbeat`);

            if (heartbeat) {
                const hb = JSON.parse(heartbeat);
                if (Date.now() - hb.timestamp < 30000) {
                    keeper.status = 'active';
                    active.push(keeper);
                } else {
                    keeper.status = 'inactive';
                }
            } else {
                keeper.status = 'failed';
            }
        }

        return active;
    }

    private async getAllMarkets(): Promise<string[]> {
        // This would fetch actual market IDs from your data source
        // For now, return mock data
        const markets: string[] = [];
        for (let i = 0; i < 1000; i++) {
            markets.push(`market_${i}`);
        }
        return markets;
    }

    // Public methods for external use
    async getWorkAssignment(): Promise<string[]> {
        return this.currentWork;
    }

    async reportProgress(processed: number, errors: number) {
        this.metrics.processed += processed;
        this.metrics.errors += errors;
        this.metrics.queueDepth = Math.max(0, this.metrics.queueDepth - processed);

        await this.redis.hincrby('keeper:progress', this.keeperId, processed);
        
        if (errors > 0) {
            await this.redis.hincrby('keeper:errors', this.keeperId, errors);
        }
    }

    async addToRetryQueue(marketId: string, error: string) {
        await this.redis.lpush(
            'keeper:retry:queue',
            JSON.stringify({ 
                marketId, 
                keeperId: this.keeperId,
                error,
                timestamp: Date.now()
            })
        );
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
    async performHealthCheck(): Promise<boolean> {
        try {
            // Check Redis connectivity
            await this.redis.ping();
            
            // Check if heartbeat is recent
            const heartbeat = await this.redis.get(`keeper:${this.keeperId}:heartbeat`);
            if (!heartbeat) return false;
            
            const hb = JSON.parse(heartbeat);
            return Date.now() - hb.timestamp < 30000;
        } catch (error) {
            return false;
        }
    }
}

// Keeper recovery mechanism
export class KeeperRecovery {
    constructor(private redis: Redis) {}

    async detectFailedKeepers(): Promise<string[]> {
        const keepers = await this.redis.hgetall('keepers:registry');
        const failed: string[] = [];

        for (const [id, data] of Object.entries(keepers)) {
            const heartbeat = await this.redis.get(`keeper:${id}:heartbeat`);
            
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
    }

    async redistributeWork(failedKeeperId: string) {
        // Get failed keeper's work
        const distribution = await this.redis.hget('keeper:work:distribution', 'current');
        if (!distribution) return;

        const work = new Map(JSON.parse(distribution));
        const failedWork = work.get(failedKeeperId) as string[] || [];
        
        if (failedWork.length === 0) return;

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
            const targetWork = work.get(targetKeeper) as string[];
            if (targetWork) {
                targetWork.push(marketId);
            }
            index++;
        }

        // Update distribution
        await this.redis.hset(
            'keeper:work:distribution',
            'current',
            JSON.stringify(Array.from(work.entries()))
        );

        // Notify affected keepers
        for (const [keeperId, markets] of work) {
            await this.redis.publish(
                `keeper:${keeperId}:work`,
                JSON.stringify({ markets: markets as string[], timestamp: Date.now() })
            );
        }

        console.log(`Redistributed ${failedWork.length} markets from failed keeper ${failedKeeperId}`);
    }
}