interface PerformanceMetric {
  samples: number[];
  average: number;
  min: number;
  max: number;
}

interface PerformanceReport {
  timestamp: number;
  components: Record<string, ComponentMetrics>;
  webVitals: WebVitals;
  warnings: PerformanceWarning[];
}

interface ComponentMetrics {
  average: number;
  min: number;
  max: number;
  samples: number;
}

interface WebVitals {
  FCP: number;  // First Contentful Paint
  LCP: number;  // Largest Contentful Paint
  TTI: number;  // Time to Interactive
  TBT: number;  // Total Blocking Time
}

interface PerformanceWarning {
  component: string;
  issue: string;
  value: number;
}

export class UIPerformanceTracker {
  private metrics: Map<string, PerformanceMetric> = new Map();
  private observer: PerformanceObserver | null = null;

  constructor() {
    this.initializeObserver();
  }

  private initializeObserver(): void {
    if (typeof window === 'undefined') return;

    this.observer = new PerformanceObserver((list) => {
      for (const entry of list.getEntries()) {
        this.processEntry(entry);
      }
    });

    this.observer.observe({
      entryTypes: ['measure', 'navigation', 'paint', 'largest-contentful-paint']
    });
  }

  trackComponentRender(componentName: string, renderTime: number): void {
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

  trackInteraction(interactionName: string, duration: number): void {
    performance.mark(`${interactionName}-start`);

    setTimeout(() => {
      performance.mark(`${interactionName}-end`);
      performance.measure(
        interactionName,
        `${interactionName}-start`,
        `${interactionName}-end`
      );
    }, duration);
  }

  getMetrics(): PerformanceReport {
    const report: PerformanceReport = {
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

  private getWebVitals(): WebVitals {
    const navigation = performance.getEntriesByType('navigation')[0] as PerformanceNavigationTiming;
    const paint = performance.getEntriesByType('paint');

    return {
      FCP: paint.find(p => p.name === 'first-contentful-paint')?.startTime || 0,
      LCP: this.getLargestContentfulPaint(),
      TTI: navigation?.loadEventEnd - navigation?.fetchStart || 0,
      TBT: this.getTotalBlockingTime()
    };
  }

  private getLargestContentfulPaint(): number {
    const entries = performance.getEntriesByType('largest-contentful-paint');
    const lastEntry = entries[entries.length - 1];
    return lastEntry ? lastEntry.startTime : 0;
  }

  private getTotalBlockingTime(): number {
    // Simplified TBT calculation
    const longTasks = performance.getEntriesByType('longtask');
    return longTasks.reduce((total, task) => {
      const blockingTime = Math.max(0, task.duration - 50);
      return total + blockingTime;
    }, 0);
  }

  private getOrCreateMetric(name: string): PerformanceMetric {
    if (!this.metrics.has(name)) {
      this.metrics.set(name, {
        samples: [],
        average: 0,
        min: Infinity,
        max: -Infinity
      });
    }
    return this.metrics.get(name)!;
  }

  private processEntry(entry: PerformanceEntry): void {
    if (entry.entryType === 'measure') {
      this.trackComponentRender(entry.name, entry.duration);
    }
  }

  // Utility hooks for React components
  static useComponentPerformance(componentName: string) {
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

// Global instance
declare global {
  interface Window {
    performanceTracker: UIPerformanceTracker;
  }
}

if (typeof window !== 'undefined') {
  window.performanceTracker = new UIPerformanceTracker();
}