"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.componentTokens = exports.designTokens = void 0;
exports.designTokens = {
    colors: {
        // Blur-inspired dark palette
        background: {
            primary: '#0A0A0A', // Main background
            secondary: '#141414', // Card backgrounds
            tertiary: '#1A1A1A', // Elevated surfaces
            overlay: 'rgba(0,0,0,0.8)'
        },
        text: {
            primary: '#FFFFFF',
            secondary: '#9CA3AF',
            tertiary: '#6B7280',
            inverse: '#000000'
        },
        accent: {
            primary: '#00FF88', // Profit/Long
            secondary: '#FF3333', // Loss/Short
            warning: '#FFB800', // Warnings
            info: '#3B82F6',
            leverage: '#FFB800' // High leverage indicator
        },
        status: {
            success: '#10B981',
            error: '#EF4444',
            warning: '#F59E0B',
            liquidation: '#DC2626' // Liquidation warnings
        }
    },
    typography: {
        fonts: {
            mono: 'SF Mono, Monaco, Inconsolata, monospace',
            sans: 'Inter, -apple-system, BlinkMacSystemFont, sans-serif'
        },
        sizes: {
            xs: '11px',
            sm: '13px',
            base: '15px',
            lg: '18px',
            xl: '24px',
            '2xl': '32px',
            '3xl': '48px',
            '4xl': '64px' // Large numbers display
        },
        weights: {
            regular: 400,
            medium: 500,
            semibold: 600,
            bold: 700,
            black: 900
        }
    },
    spacing: {
        xs: '4px',
        sm: '8px',
        md: '16px',
        lg: '24px',
        xl: '32px',
        '2xl': '48px'
    },
    animation: {
        durations: {
            instant: '100ms',
            fast: '200ms',
            normal: '300ms',
            slow: '500ms'
        },
        easings: {
            default: 'cubic-bezier(0.4, 0, 0.2, 1)',
            bounce: 'cubic-bezier(0.68, -0.55, 0.265, 1.55)',
            smooth: 'cubic-bezier(0.23, 1, 0.32, 1)'
        }
    },
    breakpoints: {
        mobile: '640px',
        tablet: '768px',
        desktop: '1024px',
        wide: '1280px',
        ultrawide: '1536px'
    }
};
// Component-specific tokens
exports.componentTokens = {
    leverageSlider: {
        trackHeight: '8px',
        thumbSize: '24px',
        dangerZone: 100, // 100x+ shows warnings
        extremeZone: 300, // 300x+ shows extreme warnings
        maxEffective: 500 // 500x+ effective leverage cap
    },
    marketCard: {
        minHeight: '120px',
        borderRadius: '12px',
        hoverScale: 1.02,
        shadowElevation: {
            default: '0 2px 8px rgba(0,0,0,0.4)',
            hover: '0 8px 24px rgba(0,0,0,0.6)',
            active: '0 4px 12px rgba(0,0,0,0.8)'
        }
    },
    priceDisplay: {
        changeThreshold: 0.001, // 0.1% triggers animation
        flashDuration: '400ms',
        precisionDigits: 4
    }
};
