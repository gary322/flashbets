export const designTokens = {
  colors: {
    // John Ive-inspired minimalist palette
    background: {
      primary: '#000000',      // Pure black background
      secondary: '#080808',    // Slightly elevated surfaces
      tertiary: '#0A0A0A',     // Card backgrounds
      overlay: 'rgba(0,0,0,0.8)'
    },

    text: {
      primary: '#FFFFFF',
      secondary: 'rgba(255, 255, 255, 0.6)',
      tertiary: 'rgba(255, 255, 255, 0.4)',
      inverse: '#000000'
    },

    accent: {
      primary: '#FFD60A',       // Gold primary accent
      secondary: '#FFA500',     // Orange secondary accent
      success: '#4CD964',       // Apple green for success
      error: '#FF3B30',         // Apple red for errors
      info: '#007AFF',          // Apple blue for info
      leverage: '#FFA500'       // Orange for leverage
    },

    status: {
      success: '#4CD964',
      error: '#FF3B30',
      warning: '#FF9500',
      liquidation: '#FF3B30'    // Red for liquidations
    },

    verse: {
      politics: '#7B3FF2',      // Purple for politics
      crypto: '#00D4FF',        // Blue for crypto
      sports: '#4CD964',        // Green for sports
      science: '#FF9500',       // Orange for science
      default: '#FFD60A'        // Gold default
    },

    quantum: {
      superposition: 'rgba(255, 165, 0, 0.5)',  // Semi-transparent orange
      entangled: 'rgba(255, 214, 10, 0.5)',     // Semi-transparent gold
      collapsed: '#FF3B30',                      // Red for collapsed states
      coherent: '#4CD964'                        // Green for coherent states
    }
  },

  typography: {
    fonts: {
      mono: 'SF Mono, monospace',
      sans: '-apple-system, BlinkMacSystemFont, "SF Pro Display", "Helvetica Neue", sans-serif'
    },

    sizes: {
      xs: '11px',
      sm: '13px',
      base: '15px',
      lg: '18px',
      xl: '24px',
      '2xl': '32px',
      '3xl': '48px',
      '4xl': '64px'    // Large numbers display
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
export const componentTokens = {
  leverageSlider: {
    trackHeight: '8px',
    thumbSize: '24px',
    dangerZone: 100,      // 100x+ shows warnings
    extremeZone: 300,     // 300x+ shows extreme warnings
    maxEffective: 500     // 500x+ effective leverage cap
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
    changeThreshold: 0.001,  // 0.1% triggers animation
    flashDuration: '400ms',
    precisionDigits: 4
  },

  verseTree: {
    nodeHeight: '40px',
    indentSize: '28px',
    iconSize: '20px',
    borderRadius: '6px',
    expandIconSize: '16px'
  },

  quantumToggle: {
    width: '48px',
    height: '24px',
    thumbSize: '20px',
    borderRadius: '12px'
  },

  panel: {
    leftWidth: '320px',
    rightWidth: '360px',
    headerHeight: '64px',
    gap: '1px'
  }
};