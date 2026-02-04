import { Connection, PublicKey, Keypair } from '@solana/web3.js';
import { AnchorProvider, Program, Wallet } from '@coral-xyz/anchor';
import * as cron from 'node-cron';
import winston from 'winston';
import pLimit from 'p-limit';
import { DraftKingsAdapter } from './providers/draftkings';
import { FanDuelAdapter } from './providers/fanduel';
import { BetMGMAdapter } from './providers/betmgm';
import { ProviderAdapter, SportEvent, LiveOdds } from './providers/adapter';

// Logger setup
const logger = winston.createLogger({
    level: 'info',
    format: winston.format.json(),
    transports: [
        new winston.transports.File({ filename: 'error.log', level: 'error' }),
        new winston.transports.File({ filename: 'combined.log' }),
        new winston.transports.Console({
            format: winston.format.simple()
        })
    ]
});

// Provider configuration
const PROVIDERS: ProviderAdapter[] = [
    new DraftKingsAdapter(),
    new FanDuelAdapter(),
    new BetMGMAdapter(),
];

// Rate limiting
const limit = pLimit(5); // Max 5 concurrent requests

// Sports to monitor
const SPORTS = ['soccer', 'basketball', 'football', 'baseball', 'tennis'];

// Flash market threshold (5 minutes)
const FLASH_THRESHOLD = 300; // seconds

export class FlashIngestor {
    private connection: Connection;
    private program: Program;
    private activeMarkets: Map<string, SportEvent> = new Map();
    private sseClients: Set<any> = new Set();
    
    constructor(
        rpcUrl: string = process.env.RPC_URL || 'http://localhost:8899',
        programId: string = process.env.FLASH_PROGRAM_ID || 'MvFlashProgramID456'
    ) {
        this.connection = new Connection(rpcUrl);
        // Program initialization would happen here with IDL
    }
    
    async start() {
        logger.info('Starting Flash Ingestor...');
        
        // Poll for live events every 2 seconds
        cron.schedule('*/2 * * * * *', () => {
            this.pollLiveEvents();
        });
        
        // Poll for pre-game events every 60 seconds
        cron.schedule('0 * * * * *', () => {
            this.pollPreGameEvents();
        });
        
        // Clean up expired markets every 30 seconds
        cron.schedule('*/30 * * * * *', () => {
            this.cleanupExpiredMarkets();
        });
        
        logger.info('Flash Ingestor started successfully');
    }
    
    private async pollLiveEvents() {
        const tasks = SPORTS.flatMap(sport =>
            PROVIDERS.map(provider =>
                limit(() => this.fetchAndProcessEvents(provider, sport, true))
            )
        );
        
        try {
            await Promise.allSettled(tasks);
        } catch (error) {
            logger.error('Error polling live events:', error);
        }
    }
    
    private async pollPreGameEvents() {
        const tasks = SPORTS.flatMap(sport =>
            PROVIDERS.map(provider =>
                limit(() => this.fetchAndProcessEvents(provider, sport, false))
            )
        );
        
        try {
            await Promise.allSettled(tasks);
        } catch (error) {
            logger.error('Error polling pre-game events:', error);
        }
    }
    
    private async fetchAndProcessEvents(
        provider: ProviderAdapter,
        sport: string,
        live: boolean
    ) {
        try {
            const events = await provider.getEvents(sport, live);
            
            for (const event of events) {
                // Check if qualifies for flash market
                if (this.isFlashMarket(event)) {
                    await this.createFlashMarket(event);
                } else {
                    // Update existing market if already flash
                    const marketId = this.generateMarketId(event);
                    if (this.activeMarkets.has(marketId)) {
                        await this.updateFlashMarket(event);
                    }
                }
            }
        } catch (error) {
            logger.error(`Error fetching events from ${provider.constructor.name}:`, error);
        }
    }
    
    private isFlashMarket(event: SportEvent): boolean {
        // Flash if: live and <5 minutes remaining, or specific quick events
        if (event.status === 'live' && event.timeRemaining && event.timeRemaining <= FLASH_THRESHOLD) {
            return true;
        }
        
        // Check for specific flash-worthy titles
        const flashKeywords = ['next', 'current', 'now', 'immediate', 'quick'];
        return flashKeywords.some(keyword => 
            event.title.toLowerCase().includes(keyword)
        );
    }
    
    private async createFlashMarket(event: SportEvent) {
        const marketId = this.generateMarketId(event);
        
        // Check if already exists
        if (this.activeMarkets.has(marketId)) {
            return;
        }
        
        try {
            // Call Solana program to create flash verse
            // In production, this would use the actual program methods
            logger.info(`Creating flash market: ${event.title}`);
            
            // Mock transaction for now
            const txData = {
                title: event.title,
                sport_type: this.mapSportType(event.sport),
                time_left: event.timeRemaining || 60,
                outcomes: event.outcomes.map(o => o.name),
            };
            
            // await this.program.methods
            //     .createFlashVerse(
            //         txData.title,
            //         txData.sport_type,
            //         txData.time_left,
            //         txData.outcomes
            //     )
            //     .rpc();
            
            this.activeMarkets.set(marketId, event);
            
            // Broadcast to SSE clients
            this.broadcastUpdate({
                type: 'new_market',
                data: event
            });
            
            logger.info(`Flash market created: ${marketId}`);
        } catch (error) {
            logger.error(`Error creating flash market ${marketId}:`, error);
        }
    }
    
    private async updateFlashMarket(event: SportEvent) {
        const marketId = this.generateMarketId(event);
        
        try {
            // Update odds on-chain
            // In production, would call program method
            
            this.activeMarkets.set(marketId, event);
            
            // Broadcast update
            this.broadcastUpdate({
                type: 'update_market',
                data: event
            });
        } catch (error) {
            logger.error(`Error updating flash market ${marketId}:`, error);
        }
    }
    
    private cleanupExpiredMarkets() {
        const now = Date.now();
        const expired: string[] = [];
        
        for (const [id, event] of this.activeMarkets) {
            const eventTime = event.startTime.getTime();
            const timeRemaining = event.timeRemaining || 0;
            
            // Remove if event finished more than 5 minutes ago
            if (eventTime + timeRemaining * 1000 + 300000 < now) {
                expired.push(id);
            }
        }
        
        for (const id of expired) {
            this.activeMarkets.delete(id);
            logger.info(`Cleaned up expired market: ${id}`);
        }
    }
    
    private generateMarketId(event: SportEvent): string {
        return `${event.sport}_${event.id}_${Math.floor(Date.now() / 60000)}`;
    }
    
    private mapSportType(sport: string): number {
        const mapping: { [key: string]: number } = {
            'soccer': 1,
            'basketball': 2,
            'football': 3,
            'baseball': 4,
            'tennis': 5,
        };
        return mapping[sport.toLowerCase()] || 0;
    }
    
    // SSE support for real-time updates
    addSSEClient(client: any) {
        this.sseClients.add(client);
    }
    
    removeSSEClient(client: any) {
        this.sseClients.delete(client);
    }
    
    private broadcastUpdate(update: any) {
        const data = `data: ${JSON.stringify(update)}\n\n`;
        
        for (const client of this.sseClients) {
            client.write(data);
        }
    }
    
    // Aggregation for best odds
    async aggregateOdds(gameId: string): Promise<LiveOdds> {
        const results = await Promise.allSettled(
            PROVIDERS.map(provider => 
                limit(() => provider.getLiveOdds(gameId))
            )
        );
        
        const validResults = results
            .filter(r => r.status === 'fulfilled')
            .map(r => (r as any).value as LiveOdds);
        
        if (validResults.length === 0) {
            throw new Error('No providers available');
        }
        
        // Calculate weighted average
        const avgProbability = validResults.reduce(
            (sum, r) => sum + r.probability,
            0
        ) / validResults.length;
        
        return {
            probability: avgProbability,
            timestamp: Date.now(),
            volume: validResults.reduce((sum, r) => sum + (r.volume || 0), 0),
            liquidity: validResults.reduce((sum, r) => sum + (r.liquidity || 0), 0),
        };
    }
}

// Start ingestor if run directly
if (require.main === module) {
    const ingestor = new FlashIngestor();
    ingestor.start().catch(console.error);
}