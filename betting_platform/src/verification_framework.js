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
exports.VerificationDashboard = exports.KeeperHealthMonitor = exports.PriceFeedValidator = void 0;
const polymarket_client_1 = require("./polymarket_client");
class PriceFeedValidator {
    constructor(program, connection) {
        this.program = program;
        this.connection = connection;
        this.polymarket = new polymarket_client_1.PolymarketClient();
    }
    validatePriceFeed() {
        return __awaiter(this, void 0, void 0, function* () {
            console.log('Starting price feed validation...');
            const report = {
                timestamp: new Date(),
                totalVerses: 0,
                stalePrices: [],
                priceMismatches: [],
                errors: [],
            };
            try {
                // Fetch all price cache PDAs
                const priceCaches = yield this.program.account.priceCachePDA.all();
                report.totalVerses = priceCaches.length;
                // Get current slot
                const currentSlot = yield this.connection.getSlot();
                for (const cache of priceCaches) {
                    const verseId = cache.account.verseId.toString();
                    // Check staleness
                    if (this.isPriceStale(cache.account, currentSlot)) {
                        report.stalePrices.push({
                            verseId,
                            lastUpdateSlot: cache.account.lastUpdateSlot.toNumber(),
                            currentSlot,
                            staleBySlots: currentSlot - cache.account.lastUpdateSlot.toNumber(),
                        });
                    }
                    // Compare with Polymarket price
                    try {
                        const polymarketPrice = yield this.getPolymarketPrice(verseId);
                        const onChainPrice = cache.account.lastPrice.toNumber() / 1e8; // Convert from fixed point
                        const priceDiff = Math.abs(onChainPrice - polymarketPrice) / polymarketPrice;
                        if (priceDiff > 0.05) { // 5% threshold
                            report.priceMismatches.push({
                                verseId,
                                onChainPrice,
                                polymarketPrice,
                                difference: priceDiff * 100,
                            });
                        }
                    }
                    catch (error) {
                        report.errors.push({
                            verseId,
                            error: error instanceof Error ? error.message : 'Unknown error',
                        });
                    }
                }
                report.summary = {
                    healthy: report.stalePrices.length === 0 && report.priceMismatches.length === 0,
                    stalePricePercentage: (report.stalePrices.length / report.totalVerses) * 100,
                    mismatchPercentage: (report.priceMismatches.length / report.totalVerses) * 100,
                };
            }
            catch (error) {
                console.error('Validation error:', error);
                report.errors.push({
                    verseId: 'global',
                    error: error instanceof Error ? error.message : 'Unknown error',
                });
            }
            return report;
        });
    }
    isPriceStale(cache, currentSlot) {
        // Consider stale if not updated for 150 slots (~1 minute)
        return currentSlot > cache.lastUpdateSlot.toNumber() + 150;
    }
    getPolymarketPrice(verseId) {
        return __awaiter(this, void 0, void 0, function* () {
            // This would need to map verse IDs back to Polymarket market IDs
            // For now, return a mock price
            return 0.5 + Math.random() * 0.3;
        });
    }
}
exports.PriceFeedValidator = PriceFeedValidator;
class KeeperHealthMonitor {
    constructor(connection) {
        this.connection = connection;
        this.polymarket = new polymarket_client_1.PolymarketClient();
    }
    checkHealth(keeperId) {
        return __awaiter(this, void 0, void 0, function* () {
            const report = {
                timestamp: new Date(),
                keepers: [],
                overall: {
                    healthy: true,
                    totalKeepers: 0,
                    healthyKeepers: 0,
                    warnings: [],
                },
            };
            try {
                // Check WebSocket connection
                const wsConnected = this.polymarket.isConnected();
                if (!wsConnected) {
                    report.overall.warnings.push('WebSocket connection is down');
                    report.overall.healthy = false;
                }
                // Check API connectivity
                const apiHealthy = yield this.checkApiHealth();
                if (!apiHealthy) {
                    report.overall.warnings.push('API connectivity issues detected');
                    report.overall.healthy = false;
                }
                // Monitor metrics
                const metrics = yield this.collectMetrics();
                report.metrics = metrics;
                // Check for specific keeper if provided
                if (keeperId) {
                    const keeperHealth = yield this.checkKeeperHealth(keeperId);
                    report.keepers.push(keeperHealth);
                }
                report.overall.totalKeepers = report.keepers.length;
                report.overall.healthyKeepers = report.keepers.filter(k => k.healthy).length;
            }
            catch (error) {
                console.error('Health check error:', error);
                report.overall.healthy = false;
                report.overall.warnings.push(error instanceof Error ? error.message : 'Unknown error');
            }
            return report;
        });
    }
    checkApiHealth() {
        return __awaiter(this, void 0, void 0, function* () {
            try {
                const markets = yield this.polymarket.fetchMarkets(1, 0);
                return markets.length > 0;
            }
            catch (error) {
                console.error('API health check failed:', error);
                return false;
            }
        });
    }
    collectMetrics() {
        return __awaiter(this, void 0, void 0, function* () {
            return {
                marketsTracked: 0, // Would be populated from actual keeper data
                versesActive: 0,
                stalePrices: 0,
                wsConnected: this.polymarket.isConnected(),
                apiErrors: 0,
                lastUpdate: Date.now(),
            };
        });
    }
    checkKeeperHealth(keeperId) {
        return __awaiter(this, void 0, void 0, function* () {
            // This would check actual keeper health from on-chain data
            return {
                keeperId,
                healthy: true,
                lastHeartbeat: Date.now(),
                marketsProcessed: 0,
                errors: 0,
                averageLatency: 0,
            };
        });
    }
}
exports.KeeperHealthMonitor = KeeperHealthMonitor;
// Dashboard class for monitoring
class VerificationDashboard {
    constructor(program, connection) {
        this.reportInterval = null;
        this.validator = new PriceFeedValidator(program, connection);
        this.healthMonitor = new KeeperHealthMonitor(connection);
    }
    start() {
        return __awaiter(this, arguments, void 0, function* (intervalMs = 60000) {
            console.log('Starting verification dashboard...');
            // Run initial check
            yield this.runFullCheck();
            // Set up periodic checks
            this.reportInterval = setInterval(() => __awaiter(this, void 0, void 0, function* () {
                yield this.runFullCheck();
            }), intervalMs);
        });
    }
    stop() {
        return __awaiter(this, void 0, void 0, function* () {
            if (this.reportInterval) {
                clearInterval(this.reportInterval);
                this.reportInterval = null;
            }
            console.log('Verification dashboard stopped');
        });
    }
    runFullCheck() {
        return __awaiter(this, void 0, void 0, function* () {
            console.log('\n=== Running Full System Check ===');
            // Run price validation
            const validationReport = yield this.validator.validatePriceFeed();
            this.displayValidationReport(validationReport);
            // Run health check
            const healthReport = yield this.healthMonitor.checkHealth();
            this.displayHealthReport(healthReport);
            // Generate alerts if needed
            this.generateAlerts(validationReport, healthReport);
        });
    }
    displayValidationReport(report) {
        console.log('\n--- Price Feed Validation Report ---');
        console.log(`Total Verses: ${report.totalVerses}`);
        console.log(`Stale Prices: ${report.stalePrices.length}`);
        console.log(`Price Mismatches: ${report.priceMismatches.length}`);
        if (report.stalePrices.length > 0) {
            console.log('\nStale Prices:');
            report.stalePrices.forEach(sp => {
                console.log(`  - Verse ${sp.verseId}: stale by ${sp.staleBySlots} slots`);
            });
        }
        if (report.priceMismatches.length > 0) {
            console.log('\nPrice Mismatches:');
            report.priceMismatches.forEach(pm => {
                console.log(`  - Verse ${pm.verseId}: ${pm.difference.toFixed(2)}% difference`);
            });
        }
    }
    displayHealthReport(report) {
        console.log('\n--- Keeper Health Report ---');
        console.log(`Overall Health: ${report.overall.healthy ? 'HEALTHY' : 'UNHEALTHY'}`);
        if (report.overall.warnings.length > 0) {
            console.log('\nWarnings:');
            report.overall.warnings.forEach(w => console.log(`  - ${w}`));
        }
        if (report.metrics) {
            console.log('\nMetrics:');
            console.log(`  - WebSocket Connected: ${report.metrics.wsConnected}`);
            console.log(`  - Markets Tracked: ${report.metrics.marketsTracked}`);
            console.log(`  - Stale Prices: ${report.metrics.stalePrices}`);
        }
    }
    generateAlerts(validationReport, healthReport) {
        const alerts = [];
        // Check for critical issues
        if (validationReport.summary && validationReport.summary.stalePricePercentage > 10) {
            alerts.push(`CRITICAL: ${validationReport.summary.stalePricePercentage.toFixed(1)}% of prices are stale`);
        }
        if (validationReport.summary && validationReport.summary.mismatchPercentage > 5) {
            alerts.push(`WARNING: ${validationReport.summary.mismatchPercentage.toFixed(1)}% of prices have mismatches`);
        }
        if (!healthReport.overall.healthy) {
            alerts.push('CRITICAL: System health check failed');
        }
        if (alerts.length > 0) {
            console.log('\n🚨 ALERTS:');
            alerts.forEach(alert => console.log(`  ${alert}`));
        }
    }
}
exports.VerificationDashboard = VerificationDashboard;
