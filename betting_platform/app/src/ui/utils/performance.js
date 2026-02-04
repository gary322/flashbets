"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.UIPerformanceTracker = void 0;
class UIPerformanceTracker {
    constructor() {
        this.metrics = new Map();
        this.observer = null;
        this.initializeObserver();
    }
    initializeObserver() {
        if (typeof window === 'undefined')
            return;
        this.observer = new PerformanceObserver((list) => {
            for (const entry of list.getEntries()) {
                this.processEntry(entry);
            }
        });
        this.observer.observe({
            entryTypes: ['measure', 'navigation', 'paint', 'largest-contentful-paint']
        });
    }
    trackComponentRender(componentName, renderTime) {
        const metric = this.getOrCreateMetric(componentName);
        metric.samples.push(renderTime);
        if (metric.samples.length > 100) {
            metric.samples.shift(); // Keep last 100 samples
        }
        // Update statistics
        metric.average = metric.samples.reduce((a, b) => a + b, 0) / metric.samples.length;
        metric.min = Math.min(...metric.samples);
        metric.max = Math.max(...metric.samples);
        // Warn if render time exceeds threshold
        if (renderTime > 16.67) { // 60fps threshold
            console.warn(`Slow render detected in ${componentName}: ${renderTime.toFixed(2)}ms`);
        }
    }
    trackInteraction(interactionName, duration) {
        performance.mark(`${interactionName}-start`);
        setTimeout(() => {
            performance.mark(`${interactionName}-end`);
            performance.measure(interactionName, `${interactionName}-start`, `${interactionName}-end`);
        }, duration);
    }
    getMetrics() {
        const report = {
            timestamp: Date.now(),
            components: {},
            webVitals: this.getWebVitals(),
            warnings: []
        };
        this.metrics.forEach((metric, name) => {
            report.components[name] = {
                average: metric.average,
                min: metric.min,
                max: metric.max,
                samples: metric.samples.length
            };
            if (metric.average > 16.67) {
                report.warnings.push({
                    component: name,
                    issue: 'Slow average render time',
                    value: metric.average
                });
            }
        });
        return report;
    }
    getWebVitals() {
        var _a;
        const navigation = performance.getEntriesByType('navigation')[0];
        const paint = performance.getEntriesByType('paint');
        return {
            FCP: ((_a = paint.find(p => p.name === 'first-contentful-paint')) === null || _a === void 0 ? void 0 : _a.startTime) || 0,
            LCP: this.getLargestContentfulPaint(),
            TTI: (navigation === null || navigation === void 0 ? void 0 : navigation.loadEventEnd) - (navigation === null || navigation === void 0 ? void 0 : navigation.fetchStart) || 0,
            TBT: this.getTotalBlockingTime()
        };
    }
    getLargestContentfulPaint() {
        const entries = performance.getEntriesByType('largest-contentful-paint');
        const lastEntry = entries[entries.length - 1];
        return lastEntry ? lastEntry.startTime : 0;
    }
    getTotalBlockingTime() {
        // Simplified TBT calculation
        const longTasks = performance.getEntriesByType('longtask');
        return longTasks.reduce((total, task) => {
            const blockingTime = Math.max(0, task.duration - 50);
            return total + blockingTime;
        }, 0);
    }
    getOrCreateMetric(name) {
        if (!this.metrics.has(name)) {
            this.metrics.set(name, {
                samples: [],
                average: 0,
                min: Infinity,
                max: -Infinity
            });
        }
        return this.metrics.get(name);
    }
    processEntry(entry) {
        if (entry.entryType === 'measure') {
            this.trackComponentRender(entry.name, entry.duration);
        }
    }
    // Utility hooks for React components
    static useComponentPerformance(componentName) {
        const startTime = performance.now();
        return {
            endTracking: () => {
                const endTime = performance.now();
                const duration = endTime - startTime;
                if (typeof window !== 'undefined' && window.performanceTracker) {
                    window.performanceTracker.trackComponentRender(componentName, duration);
                }
            }
        };
    }
}
exports.UIPerformanceTracker = UIPerformanceTracker;
if (typeof window !== 'undefined') {
    window.performanceTracker = new UIPerformanceTracker();
}
