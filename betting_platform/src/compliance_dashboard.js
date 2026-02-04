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
exports.IntegratedMonitoringDashboard = exports.KeeperNetworkHealthDashboard = exports.RateLimitComplianceChecker = void 0;
const events_1 = require("events");
const ioredis_1 = require("ioredis");
const rate_limiter_1 = require("./rate_limiter");
const failover_manager_1 = require("./failover_manager");
class RateLimitComplianceChecker {
    constructor() {
        this.violations = [];
        this.complianceHistory = new Map();
        this.monitor = new rate_limiter_1.RateLimitMonitor();
    }
    checkCompliance() {
        return __awaiter(this, arguments, void 0, function* (windowMs = 10000) {
            const report = this.monitor.getComplianceReport(windowMs);
            const timestamp = Date.now();
            const violations = [];
            // Check each endpoint
            for (const [endpoint, count] of Object.entries(report.usage)) {
                const limit = this.getLimitForEndpoint(endpoint);
                const compliant = count <= limit;
                const metric = {
                    timestamp,
                    endpoint,
                    requestCount: count,
                    windowSize: windowMs,
                    limit,
                    compliant,
                };
                if (!compliant) {
                    violations.push(metric);
                    this.violations.push(metric);
                }
                // Update compliance history
                if (!this.complianceHistory.has(endpoint)) {
                    this.complianceHistory.set(endpoint, []);
                }
                this.complianceHistory.get(endpoint).push(compliant);
                // Keep only last 100 checks
                const history = this.complianceHistory.get(endpoint);
                if (history.length > 100) {
                    history.shift();
                }
            }
            return {
                compliant: violations.length === 0,
                violations,
                report,
            };
        });
    }
    getLimitForEndpoint(endpoint) {
        // Free tier limits per 10 seconds
        if (endpoint.includes('/markets'))
            return 50;
        if (endpoint.includes('/orders'))
            return 100;
        if (endpoint.includes('/resolutions'))
            return 10;
        return 50; // Default
    }
    getComplianceRate(endpoint) {
        if (endpoint) {
            const history = this.complianceHistory.get(endpoint);
            if (!history || history.length === 0)
                return 1;
            const compliantCount = history.filter(c => c).length;
            return compliantCount / history.length;
        }
        // Overall compliance rate
        let totalCompliant = 0;
        let totalChecks = 0;
        for (const history of this.complianceHistory.values()) {
            totalCompliant += history.filter(c => c).length;
            totalChecks += history.length;
        }
        return totalChecks > 0 ? totalCompliant / totalChecks : 1;
    }
    getViolationSummary() {
        const byEndpoint = new Map();
        for (const violation of this.violations) {
            const count = byEndpoint.get(violation.endpoint) || 0;
            byEndpoint.set(violation.endpoint, count + 1);
        }
        // Get recent violations (last hour)
        const oneHourAgo = Date.now() - 3600000;
        const recentViolations = this.violations.filter(v => v.timestamp > oneHourAgo);
        return {
            total: this.violations.length,
            byEndpoint,
            recentViolations,
        };
    }
    generateComplianceReport() {
        const overallRate = this.getComplianceRate();
        const summary = this.getViolationSummary();
        let report = '=== Rate Limit Compliance Report ===\n\n';
        report += `Overall Compliance Rate: ${(overallRate * 100).toFixed(2)}%\n`;
        report += `Total Violations: ${summary.total}\n\n`;
        report += 'Compliance by Endpoint:\n';
        for (const [endpoint, _] of this.complianceHistory) {
            const rate = this.getComplianceRate(endpoint);
            report += `  ${endpoint}: ${(rate * 100).toFixed(2)}%\n`;
        }
        if (summary.byEndpoint.size > 0) {
            report += '\nViolations by Endpoint:\n';
            for (const [endpoint, count] of summary.byEndpoint) {
                report += `  ${endpoint}: ${count} violations\n`;
            }
        }
        if (summary.recentViolations.length > 0) {
            report += '\nRecent Violations (Last Hour):\n';
            for (const violation of summary.recentViolations.slice(0, 10)) {
                const time = new Date(violation.timestamp).toLocaleTimeString();
                report += `  [${time}] ${violation.endpoint}: ${violation.requestCount}/${violation.limit} requests\n`;
            }
        }
        return report;
    }
}
exports.RateLimitComplianceChecker = RateLimitComplianceChecker;
class KeeperNetworkHealthDashboard extends events_1.EventEmitter {
    constructor(redisUrl) {
        super();
        this.healthHistory = new Map();
        this.updateInterval = null;
        this.alertThresholds = {
            errorRate: 0.1,
            latency: 5000,
            uptimeMinutes: 5,
        };
        this.redis = new ioredis_1.Redis(redisUrl);
        this.failoverManager = new failover_manager_1.FailoverManager(redisUrl);
    }
    start() {
        return __awaiter(this, arguments, void 0, function* (updateIntervalMs = 30000) {
            console.log('Starting Keeper Network Health Dashboard...');
            // Start failover manager
            yield this.failoverManager.start();
            // Initial update
            yield this.updateHealthMetrics();
            // Set up periodic updates
            this.updateInterval = setInterval(() => this.updateHealthMetrics(), updateIntervalMs);
            this.emit('started');
        });
    }
    stop() {
        return __awaiter(this, void 0, void 0, function* () {
            if (this.updateInterval) {
                clearInterval(this.updateInterval);
            }
            yield this.failoverManager.stop();
            yield this.redis.quit();
            this.emit('stopped');
        });
    }
    updateHealthMetrics() {
        return __awaiter(this, void 0, void 0, function* () {
            const status = this.failoverManager.getStatus();
            const timestamp = Date.now();
            // Update health history for each keeper
            for (const keeper of status.keepers) {
                const metric = {
                    keeperId: keeper.id,
                    timestamp,
                    health: keeper.health,
                    uptime: yield this.getKeeperUptime(keeper.id),
                    workload: keeper.workload,
                    errorRate: keeper.errorRate,
                    latency: keeper.latency,
                };
                if (!this.healthHistory.has(keeper.id)) {
                    this.healthHistory.set(keeper.id, []);
                }
                this.healthHistory.get(keeper.id).push(metric);
                // Keep only last 100 metrics
                const history = this.healthHistory.get(keeper.id);
                if (history.length > 100) {
                    history.shift();
                }
                // Check for alerts
                this.checkAlerts(metric);
            }
            this.emit('metrics_updated', { timestamp, status });
        });
    }
    getKeeperUptime(keeperId) {
        return __awaiter(this, void 0, void 0, function* () {
            const keeperData = yield this.redis.hget('keepers:registry', keeperId);
            if (!keeperData)
                return 0;
            const info = JSON.parse(keeperData);
            return Math.floor((Date.now() - info.startTime) / 60000); // minutes
        });
    }
    checkAlerts(metric) {
        const alerts = [];
        if (metric.health === 'failed') {
            alerts.push(`Keeper ${metric.keeperId} has failed`);
        }
        if (metric.errorRate > this.alertThresholds.errorRate) {
            alerts.push(`Keeper ${metric.keeperId} has high error rate: ${(metric.errorRate * 100).toFixed(1)}%`);
        }
        if (metric.latency > this.alertThresholds.latency) {
            alerts.push(`Keeper ${metric.keeperId} has high latency: ${metric.latency}ms`);
        }
        if (metric.health === 'healthy' && metric.uptime < this.alertThresholds.uptimeMinutes) {
            alerts.push(`Keeper ${metric.keeperId} was recently started (${metric.uptime} minutes ago)`);
        }
        if (alerts.length > 0) {
            this.emit('alerts', { keeperId: metric.keeperId, alerts });
        }
    }
    getNetworkHealth() {
        const status = this.failoverManager.getStatus();
        const currentAlerts = [];
        // Calculate overall health
        const healthyRatio = status.summary.healthy / status.summary.total;
        let overall = 'healthy';
        if (healthyRatio < 0.5) {
            overall = 'critical';
            currentAlerts.push('More than 50% of keepers are unhealthy');
        }
        else if (healthyRatio < 0.8) {
            overall = 'degraded';
            currentAlerts.push('Some keepers are experiencing issues');
        }
        // Get keeper details with history
        const keeperDetails = status.keepers.map(keeper => {
            const history = this.healthHistory.get(keeper.id) || [];
            const recentHistory = history.slice(-10);
            return Object.assign(Object.assign({}, keeper), { uptimePercent: this.calculateUptime(keeper.id), averageLatency: this.calculateAverageLatency(keeper.id), recentHealth: recentHistory.map(h => h.health) });
        });
        return {
            overall,
            keepers: keeperDetails,
            summary: status.summary,
            alerts: currentAlerts,
        };
    }
    calculateUptime(keeperId) {
        const history = this.healthHistory.get(keeperId) || [];
        if (history.length === 0)
            return 0;
        const healthyCount = history.filter(h => h.health === 'healthy').length;
        return (healthyCount / history.length) * 100;
    }
    calculateAverageLatency(keeperId) {
        const history = this.healthHistory.get(keeperId) || [];
        if (history.length === 0)
            return 0;
        const sum = history.reduce((acc, h) => acc + h.latency, 0);
        return Math.round(sum / history.length);
    }
    generateHealthReport() {
        const health = this.getNetworkHealth();
        let report = '=== Keeper Network Health Report ===\n\n';
        report += `Overall Status: ${health.overall.toUpperCase()}\n`;
        report += `Total Keepers: ${health.summary.total}\n`;
        report += `Healthy: ${health.summary.healthy}\n`;
        report += `Degraded: ${health.summary.degraded}\n`;
        report += `Failed: ${health.summary.failed}\n\n`;
        report += 'Keeper Details:\n';
        for (const keeper of health.keepers) {
            report += `\n${keeper.id}:\n`;
            report += `  Status: ${keeper.health}\n`;
            report += `  Uptime: ${keeper.uptimePercent.toFixed(1)}%\n`;
            report += `  Workload: ${keeper.workload} markets\n`;
            report += `  Error Rate: ${(keeper.errorRate * 100).toFixed(1)}%\n`;
            report += `  Avg Latency: ${keeper.averageLatency}ms\n`;
        }
        if (health.alerts.length > 0) {
            report += '\nActive Alerts:\n';
            for (const alert of health.alerts) {
                report += `  ⚠️  ${alert}\n`;
            }
        }
        return report;
    }
    // Get historical data for charts
    getHistoricalData(keeperId, metricName) {
        if (keeperId) {
            const history = this.healthHistory.get(keeperId) || [];
            if (metricName) {
                return history.map(h => ({
                    timestamp: h.timestamp,
                    value: h[metricName],
                }));
            }
            return history;
        }
        // Return all keeper data
        const allData = [];
        for (const [id, history] of this.healthHistory) {
            allData.push({
                keeperId: id,
                data: history,
            });
        }
        return allData;
    }
    // Dashboard API
    getDashboardData() {
        return {
            network: this.getNetworkHealth(),
            compliance: new RateLimitComplianceChecker().getComplianceRate(),
            historical: {
                keepers: this.getHistoricalData(),
            },
            lastUpdate: Date.now(),
        };
    }
}
exports.KeeperNetworkHealthDashboard = KeeperNetworkHealthDashboard;
// Combined dashboard for complete monitoring
class IntegratedMonitoringDashboard {
    constructor(redisUrl) {
        this.updateInterval = null;
        this.complianceChecker = new RateLimitComplianceChecker();
        this.healthDashboard = new KeeperNetworkHealthDashboard(redisUrl);
    }
    start() {
        return __awaiter(this, void 0, void 0, function* () {
            yield this.healthDashboard.start();
            // Periodic compliance checks
            this.updateInterval = setInterval(() => this.runComplianceCheck(), 60000 // Every minute
            );
            console.log('Integrated Monitoring Dashboard started');
        });
    }
    stop() {
        return __awaiter(this, void 0, void 0, function* () {
            if (this.updateInterval) {
                clearInterval(this.updateInterval);
            }
            yield this.healthDashboard.stop();
        });
    }
    runComplianceCheck() {
        return __awaiter(this, void 0, void 0, function* () {
            const compliance = yield this.complianceChecker.checkCompliance();
            if (!compliance.compliant) {
                console.warn('Rate limit violations detected:', compliance.violations);
            }
        });
    }
    generateFullReport() {
        let report = '=== INTEGRATED SYSTEM MONITORING REPORT ===\n\n';
        report += new Date().toLocaleString() + '\n\n';
        report += this.healthDashboard.generateHealthReport();
        report += '\n\n';
        report += this.complianceChecker.generateComplianceReport();
        return report;
    }
    getSystemStatus() {
        return {
            health: this.healthDashboard.getNetworkHealth(),
            compliance: {
                rate: this.complianceChecker.getComplianceRate(),
                violations: this.complianceChecker.getViolationSummary(),
            },
            timestamp: Date.now(),
        };
    }
}
exports.IntegratedMonitoringDashboard = IntegratedMonitoringDashboard;
