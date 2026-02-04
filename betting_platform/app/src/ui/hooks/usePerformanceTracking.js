"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.usePerformanceTracking = void 0;
const react_1 = require("react");
const usePerformanceTracking = (componentName) => {
    const renderStartTime = (0, react_1.useRef)(0);
    const frameCount = (0, react_1.useRef)(0);
    const lastFrameTime = (0, react_1.useRef)(0);
    (0, react_1.useEffect)(() => {
        // Track initial render
        renderStartTime.current = performance.now();
        return () => {
            // Track unmount
            const renderDuration = performance.now() - renderStartTime.current;
            if (typeof window !== 'undefined' && window.performanceTracker) {
                window.performanceTracker.trackComponentRender(componentName, renderDuration);
            }
        };
    }, [componentName]);
    const trackInteraction = (interactionName, callback) => {
        const startTime = performance.now();
        callback();
        const duration = performance.now() - startTime;
        if (typeof window !== 'undefined' && window.performanceTracker) {
            window.performanceTracker.trackInteraction(`${componentName}-${interactionName}`, duration);
        }
    };
    const measureFPS = () => {
        const currentTime = performance.now();
        if (lastFrameTime.current !== 0) {
            const deltaTime = currentTime - lastFrameTime.current;
            const fps = 1000 / deltaTime;
            if (fps < 55) { // Below 60fps threshold
                console.warn(`Low FPS detected in ${componentName}: ${fps.toFixed(1)}`);
            }
        }
        lastFrameTime.current = currentTime;
        frameCount.current++;
        requestAnimationFrame(measureFPS);
    };
    const startFPSMonitoring = () => {
        lastFrameTime.current = performance.now();
        requestAnimationFrame(measureFPS);
    };
    return {
        trackInteraction,
        startFPSMonitoring
    };
};
exports.usePerformanceTracking = usePerformanceTracking;
