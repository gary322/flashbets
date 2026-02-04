import { Connection, Keypair, PublicKey } from '@solana/web3.js';
import { Program } from '@coral-xyz/anchor';
import * as BN from 'bn.js';
import { PolymarketClient } from './polymarket_client';
import { VerseClassifier } from './verse_classifier';
import { Market, PriceUpdate, CachedPrice, Resolution, KeeperMetrics } from './types';

export class MarketIngestionKeeper {
    private program: Program;
    private polymarket: PolymarketClient;
    private connection: Connection;
    private authority: Keypair;
    private priceCache: Map<string, CachedPrice> = new Map();
    private processedResolutions: Set<string> = new Set();
    private classifier: VerseClassifier;
    private lastPrices: Map<string, number> = new Map();

    constructor(
        program: Program,
        connection: Connection,
        authority: Keypair
    ) {
        this.program = program;
        this.connection = connection;
        this.authority = authority;
        this.polymarket = new PolymarketClient();
        this.classifier = new VerseClassifier();
    }

    async start() {
        console.log('Starting market ingestion keeper...');

        // Initial market sync
        await this.syncAllMarkets();

        // Start WebSocket for real-time updates
        this.polymarket.connectWebSocket(async (update) => {
            await this.handlePriceUpdate(update);
        });

        // Periodic full sync (every 5 slots ~ 2 seconds)
        setInterval(() => this.syncAllMarkets(), 2000);

        // Periodic cache update (every 5 seconds for hot markets)
        setInterval(() => this.updateHotMarkets(), 5000);

        // Monitor resolutions (every 5 slots ~ 2 seconds)
        setInterval(() => this.monitorResolutions(), 2000);
    }

    async stop() {
        this.polymarket.disconnect();
    }

    async syncAllMarkets() {
        console.log('Starting full market sync...');
        const batchSize = 1000;
        let offset = 0;
        let allMarkets: Market[] = [];

        // Fetch all markets in batches
        while (true) {
            try {
                const markets = await this.polymarket.fetchMarkets(batchSize, offset);
                if (markets.length === 0) break;

                allMarkets = allMarkets.concat(markets);
                offset += batchSize;

                // Process batch
                await this.processBatch(markets);

                // Respect rate limits
                await new Promise(resolve => setTimeout(resolve, 200));
            } catch (error) {
                console.error(`Error fetching batch at offset ${offset}:`, error);
                await new Promise(resolve => setTimeout(resolve, 5000));
            }
        }

        console.log(`Synced ${allMarkets.length} markets`);
    }

    async processBatch(markets: Market[]) {
        const versesMap = new Map<string, Market[]>();

        // Classify markets into verses
        for (const market of markets) {
            const verseId = this.classifier.classifyMarket(market.question);

            if (!versesMap.has(verseId)) {
                versesMap.set(verseId, []);
            }
            versesMap.get(verseId)!.push(market);
        }

        // Update each verse
        const verses = Array.from(versesMap.entries());
        for (const [verseId, verseMarkets] of verses) {
            await this.updateVerse(verseId, verseMarkets);
        }
    }

    async updateVerse(verseId: string, markets: Market[]) {
        // Calculate weighted average probability
        let totalWeight = 0;
        let weightedSum = 0;

        for (const market of markets) {
            // Weight by 7-day volume and liquidity
            const weight = market.volume * market.liquidity;
            totalWeight += weight;
            weightedSum += market.yes_price * weight;
        }

        const derivedProb = totalWeight > 0 ? weightedSum / totalWeight : 0.5;

        // Update on-chain
        try {
            const versePubkey = await this.getVersePDA(verseId);

            const verseBN = new (BN as any)(verseId, 16);
            await this.program.methods
                .updateVerseProb(verseBN, derivedProb)
                .accounts({
                    verse: versePubkey,
                    authority: this.authority.publicKey,
                })
                .signers([this.authority])
                .rpc();

            console.log(`Updated verse ${verseId}: prob=${derivedProb.toFixed(4)}`);
        } catch (error) {
            console.error(`Failed to update verse ${verseId}:`, error);
        }
    }

    async handlePriceUpdate(update: PriceUpdate) {
        // Update cache
        this.priceCache.set(update.marketId, {
            price: update.yesPrice,
            timestamp: update.timestamp,
        });

        // Check if update is significant (>1% change)
        const lastPrice = this.getLastPrice(update.marketId);
        if (lastPrice === 0) {
            this.lastPrices.set(update.marketId, update.yesPrice);
            return;
        }

        const priceChange = Math.abs(update.yesPrice - lastPrice) / lastPrice;

        if (priceChange > 0.01) {
            // Trigger immediate verse update
            await this.updateMarketVerse(update.marketId, update.yesPrice);
            this.lastPrices.set(update.marketId, update.yesPrice);
        }
    }

    getLastPrice(marketId: string): number {
        return this.lastPrices.get(marketId) || 0;
    }

    async updateHotMarkets() {
        // Update markets with high volume/activity
        const hotMarkets = Array.from(this.priceCache.entries())
            .filter(([_, cache]) => Date.now() - cache.timestamp < 5000)
            .slice(0, 100); // Top 100 hot markets

        for (const [marketId, cache] of hotMarkets) {
            await this.updateMarketVerse(marketId, cache.price);
        }
    }

    async updateMarketVerse(marketId: string, newPrice: number) {
        // Implementation to update a specific market's verse
        // This would need to fetch the market details and update the verse
        console.log(`Updating market ${marketId} with price ${newPrice}`);
    }

    async monitorResolutions() {
        const resolutionQueue: Resolution[] = [];

        const markets = await this.polymarket.fetchMarkets();

        for (const market of markets) {
            if (market.resolved && !this.isProcessed(market.id)) {
                resolutionQueue.push({
                    marketId: market.id,
                    resolution: market.resolution || '',
                    timestamp: Date.now(),
                });
            }
        }

        // Process resolutions
        while (resolutionQueue.length > 0) {
            const resolution = resolutionQueue.shift()!;
            await this.processResolution(resolution);
        }
    }

    isProcessed(marketId: string): boolean {
        return this.processedResolutions.has(marketId);
    }

    async processResolution(resolution: Resolution) {
        console.log(`Processing resolution for market ${resolution.marketId}: ${resolution.resolution}`);
        this.processedResolutions.add(resolution.marketId);
        // Implementation would update on-chain state
    }

    async getVersePDA(verseId: string): Promise<PublicKey> {
        const [versePDA] = await PublicKey.findProgramAddress(
            [
                Buffer.from('verse'),
                Buffer.from(verseId, 'hex')
            ],
            this.program.programId
        );
        return versePDA;
    }

    async collectMetrics(): Promise<KeeperMetrics> {
        return {
            processed: this.processedResolutions.size,
            errors: 0, // Would track actual errors
            queueDepth: this.priceCache.size
        };
    }
}