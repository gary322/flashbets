import { Connection, PublicKey } from '@solana/web3.js';
import { Program } from '@coral-xyz/anchor';
import { PolymarketClient } from './polymarket_client';

export class PriceFeedValidator {
    private program: Program;
    private connection: Connection;
    private polymarket: PolymarketClient;

    constructor(program: Program, connection: Connection) {
        this.program = program;
        this.connection = connection;
        this.polymarket = new PolymarketClient();
    }

    async validatePriceFeed(): Promise<ValidationReport> {
        console.log('Starting price feed validation...');
        
        const report: ValidationReport = {
            timestamp: new Date(),
            totalVerses: 0,
            stalePrices: [],
            priceMismatches: [],
            errors: [],
        };

        try {
            // Fetch all price cache PDAs
            const priceCaches = await (this.program.account as any).priceCachePDA.all();
            report.totalVerses = priceCaches.length;

            // Get current slot
            const currentSlot = await this.connection.getSlot();

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
                    const polymarketPrice = await this.getPolymarketPrice(verseId);
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
                } catch (error) {
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

        } catch (error) {
            console.error('Validation error:', error);
            report.errors.push({
                verseId: 'global',
                error: error instanceof Error ? error.message : 'Unknown error',
            });
        }

        return report;
    }

    private isPriceStale(cache: any, currentSlot: number): boolean {
        // Consider stale if not updated for 150 slots (~1 minute)
        return currentSlot > cache.lastUpdateSlot.toNumber() + 150;
    }

    private async getPolymarketPrice(verseId: string): Promise<number> {
        // This would need to map verse IDs back to Polymarket market IDs
        // For now, return a mock price
        return 0.5 + Math.random() * 0.3;
    }
}

export class KeeperHealthMonitor {
    private connection: Connection;
    private polymarket: PolymarketClient;

    constructor(connection: Connection) {
        this.connection = connection;
        this.polymarket = new PolymarketClient();
    }

    async checkHealth(keeperId?: string): Promise<HealthReport> {
        const report: HealthReport = {
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
            const apiHealthy = await this.checkApiHealth();
            if (!apiHealthy) {
                report.overall.warnings.push('API connectivity issues detected');
                report.overall.healthy = false;
            }

            // Monitor metrics
            const metrics = await this.collectMetrics();
            report.metrics = metrics;

            // Check for specific keeper if provided
            if (keeperId) {
                const keeperHealth = await this.checkKeeperHealth(keeperId);
                report.keepers.push(keeperHealth);
            }

            report.overall.totalKeepers = report.keepers.length;
            report.overall.healthyKeepers = report.keepers.filter(k => k.healthy).length;

        } catch (error) {
            console.error('Health check error:', error);
            report.overall.healthy = false;
            report.overall.warnings.push(error instanceof Error ? error.message : 'Unknown error');
        }

        return report;
    }

    private async checkApiHealth(): Promise<boolean> {
        try {
            const markets = await this.polymarket.fetchMarkets(1, 0);
            return markets.length > 0;
        } catch (error) {
            console.error('API health check failed:', error);
            return false;
        }
    }

    private async collectMetrics(): Promise<KeeperMetrics> {
        return {
            marketsTracked: 0, // Would be populated from actual keeper data
            versesActive: 0,
            stalePrices: 0,
            wsConnected: this.polymarket.isConnected(),
            apiErrors: 0,
            lastUpdate: Date.now(),
        };
    }

    private async checkKeeperHealth(keeperId: string): Promise<KeeperHealthStatus> {
        // This would check actual keeper health from on-chain data
        return {
            keeperId,
            healthy: true,
            lastHeartbeat: Date.now(),
            marketsProcessed: 0,
            errors: 0,
            averageLatency: 0,
        };
    }
}

// Types for the verification framework
export interface ValidationReport {
    timestamp: Date;
    totalVerses: number;
    stalePrices: StalePrice[];
    priceMismatches: PriceMismatch[];
    errors: ValidationError[];
    summary?: {
        healthy: boolean;
        stalePricePercentage: number;
        mismatchPercentage: number;
    };
}

export interface StalePrice {
    verseId: string;
    lastUpdateSlot: number;
    currentSlot: number;
    staleBySlots: number;
}

export interface PriceMismatch {
    verseId: string;
    onChainPrice: number;
    polymarketPrice: number;
    difference: number; // percentage
}

export interface ValidationError {
    verseId: string;
    error: string;
}

export interface HealthReport {
    timestamp: Date;
    keepers: KeeperHealthStatus[];
    overall: {
        healthy: boolean;
        totalKeepers: number;
        healthyKeepers: number;
        warnings: string[];
    };
    metrics?: KeeperMetrics;
}

export interface KeeperHealthStatus {
    keeperId: string;
    healthy: boolean;
    lastHeartbeat: number;
    marketsProcessed: number;
    errors: number;
    averageLatency: number;
}

export interface KeeperMetrics {
    marketsTracked: number;
    versesActive: number;
    stalePrices: number;
    wsConnected: boolean;
    apiErrors: number;
    lastUpdate: number;
}

// Dashboard class for monitoring
export class VerificationDashboard {
    private validator: PriceFeedValidator;
    private healthMonitor: KeeperHealthMonitor;
    private reportInterval: NodeJS.Timeout | null = null;

    constructor(program: Program, connection: Connection) {
        this.validator = new PriceFeedValidator(program, connection);
        this.healthMonitor = new KeeperHealthMonitor(connection);
    }

    async start(intervalMs: number = 60000) {
        console.log('Starting verification dashboard...');
        
        // Run initial check
        await this.runFullCheck();

        // Set up periodic checks
        this.reportInterval = setInterval(async () => {
            await this.runFullCheck();
        }, intervalMs);
    }

    async stop() {
        if (this.reportInterval) {
            clearInterval(this.reportInterval);
            this.reportInterval = null;
        }
        console.log('Verification dashboard stopped');
    }

    private async runFullCheck() {
        console.log('\n=== Running Full System Check ===');
        
        // Run price validation
        const validationReport = await this.validator.validatePriceFeed();
        this.displayValidationReport(validationReport);

        // Run health check
        const healthReport = await this.healthMonitor.checkHealth();
        this.displayHealthReport(healthReport);

        // Generate alerts if needed
        this.generateAlerts(validationReport, healthReport);
    }

    private displayValidationReport(report: ValidationReport) {
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

    private displayHealthReport(report: HealthReport) {
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

    private generateAlerts(validationReport: ValidationReport, healthReport: HealthReport) {
        const alerts: string[] = [];

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