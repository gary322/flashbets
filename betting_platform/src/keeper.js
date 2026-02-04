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
exports.MarketIngestionKeeper = void 0;
const web3_js_1 = require("@solana/web3.js");
const BN = __importStar(require("bn.js"));
const polymarket_client_1 = require("./polymarket_client");
const verse_classifier_1 = require("./verse_classifier");
class MarketIngestionKeeper {
    constructor(program, connection, authority) {
        this.priceCache = new Map();
        this.processedResolutions = new Set();
        this.lastPrices = new Map();
        this.program = program;
        this.connection = connection;
        this.authority = authority;
        this.polymarket = new polymarket_client_1.PolymarketClient();
        this.classifier = new verse_classifier_1.VerseClassifier();
    }
    start() {
        return __awaiter(this, void 0, void 0, function* () {
            console.log('Starting market ingestion keeper...');
            // Initial market sync
            yield this.syncAllMarkets();
            // Start WebSocket for real-time updates
            this.polymarket.connectWebSocket((update) => __awaiter(this, void 0, void 0, function* () {
                yield this.handlePriceUpdate(update);
            }));
            // Periodic full sync (every 5 slots ~ 2 seconds)
            setInterval(() => this.syncAllMarkets(), 2000);
            // Periodic cache update (every 5 seconds for hot markets)
            setInterval(() => this.updateHotMarkets(), 5000);
            // Monitor resolutions (every 5 slots ~ 2 seconds)
            setInterval(() => this.monitorResolutions(), 2000);
        });
    }
    stop() {
        return __awaiter(this, void 0, void 0, function* () {
            this.polymarket.disconnect();
        });
    }
    syncAllMarkets() {
        return __awaiter(this, void 0, void 0, function* () {
            console.log('Starting full market sync...');
            const batchSize = 1000;
            let offset = 0;
            let allMarkets = [];
            // Fetch all markets in batches
            while (true) {
                try {
                    const markets = yield this.polymarket.fetchMarkets(batchSize, offset);
                    if (markets.length === 0)
                        break;
                    allMarkets = allMarkets.concat(markets);
                    offset += batchSize;
                    // Process batch
                    yield this.processBatch(markets);
                    // Respect rate limits
                    yield new Promise(resolve => setTimeout(resolve, 200));
                }
                catch (error) {
                    console.error(`Error fetching batch at offset ${offset}:`, error);
                    yield new Promise(resolve => setTimeout(resolve, 5000));
                }
            }
            console.log(`Synced ${allMarkets.length} markets`);
        });
    }
    processBatch(markets) {
        return __awaiter(this, void 0, void 0, function* () {
            const versesMap = new Map();
            // Classify markets into verses
            for (const market of markets) {
                const verseId = this.classifier.classifyMarket(market.question);
                if (!versesMap.has(verseId)) {
                    versesMap.set(verseId, []);
                }
                versesMap.get(verseId).push(market);
            }
            // Update each verse
            const verses = Array.from(versesMap.entries());
            for (const [verseId, verseMarkets] of verses) {
                yield this.updateVerse(verseId, verseMarkets);
            }
        });
    }
    updateVerse(verseId, markets) {
        return __awaiter(this, void 0, void 0, function* () {
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
                const versePubkey = yield this.getVersePDA(verseId);
                const verseBN = new BN(verseId, 16);
                yield this.program.methods
                    .updateVerseProb(verseBN, derivedProb)
                    .accounts({
                    verse: versePubkey,
                    authority: this.authority.publicKey,
                })
                    .signers([this.authority])
                    .rpc();
                console.log(`Updated verse ${verseId}: prob=${derivedProb.toFixed(4)}`);
            }
            catch (error) {
                console.error(`Failed to update verse ${verseId}:`, error);
            }
        });
    }
    handlePriceUpdate(update) {
        return __awaiter(this, void 0, void 0, function* () {
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
                yield this.updateMarketVerse(update.marketId, update.yesPrice);
                this.lastPrices.set(update.marketId, update.yesPrice);
            }
        });
    }
    getLastPrice(marketId) {
        return this.lastPrices.get(marketId) || 0;
    }
    updateHotMarkets() {
        return __awaiter(this, void 0, void 0, function* () {
            // Update markets with high volume/activity
            const hotMarkets = Array.from(this.priceCache.entries())
                .filter(([_, cache]) => Date.now() - cache.timestamp < 5000)
                .slice(0, 100); // Top 100 hot markets
            for (const [marketId, cache] of hotMarkets) {
                yield this.updateMarketVerse(marketId, cache.price);
            }
        });
    }
    updateMarketVerse(marketId, newPrice) {
        return __awaiter(this, void 0, void 0, function* () {
            // Implementation to update a specific market's verse
            // This would need to fetch the market details and update the verse
            console.log(`Updating market ${marketId} with price ${newPrice}`);
        });
    }
    monitorResolutions() {
        return __awaiter(this, void 0, void 0, function* () {
            const resolutionQueue = [];
            const markets = yield this.polymarket.fetchMarkets();
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
                const resolution = resolutionQueue.shift();
                yield this.processResolution(resolution);
            }
        });
    }
    isProcessed(marketId) {
        return this.processedResolutions.has(marketId);
    }
    processResolution(resolution) {
        return __awaiter(this, void 0, void 0, function* () {
            console.log(`Processing resolution for market ${resolution.marketId}: ${resolution.resolution}`);
            this.processedResolutions.add(resolution.marketId);
            // Implementation would update on-chain state
        });
    }
    getVersePDA(verseId) {
        return __awaiter(this, void 0, void 0, function* () {
            const [versePDA] = yield web3_js_1.PublicKey.findProgramAddress([
                Buffer.from('verse'),
                Buffer.from(verseId, 'hex')
            ], this.program.programId);
            return versePDA;
        });
    }
    collectMetrics() {
        return __awaiter(this, void 0, void 0, function* () {
            return {
                processed: this.processedResolutions.size,
                errors: 0, // Would track actual errors
                queueDepth: this.priceCache.size
            };
        });
    }
}
exports.MarketIngestionKeeper = MarketIngestionKeeper;
