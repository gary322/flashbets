import React from 'react';
import { render } from '../test-utils';
import { RiskMetrics } from '../../components/trading/RiskMetrics';

describe('RiskMetrics', () => {
  const defaultProps = {
    leverage: 10,
    liquidationPrice: 0.45,
    entryPrice: 0.50,
    marketVolatility: 0.05
  };

  describe('Rendering', () => {
    it('should render all metric cards', () => {
      const { getByText } = render(
        <RiskMetrics {...defaultProps} />
      );

      expect(getByText('Liquidation Price')).toBeInTheDocument();
      expect(getByText('Max Loss (1σ move)')).toBeInTheDocument();
      expect(getByText('Risk Level')).toBeInTheDocument();
      expect(getByText('Market Volatility')).toBeInTheDocument();
    });

    it('should display liquidation price correctly', () => {
      const { getByText } = render(
        <RiskMetrics {...defaultProps} />
      );

      expect(getByText('45.00%')).toBeInTheDocument();
    });

    it('should handle null liquidation price', () => {
      const { getByText } = render(
        <RiskMetrics {...defaultProps} liquidationPrice={null} />
      );

      expect(getByText('--')).toBeInTheDocument();
    });
  });

  describe('Liquidation Distance Calculation', () => {
    it('should calculate liquidation distance correctly', () => {
      const { getByText } = render(
        <RiskMetrics {...defaultProps} />
      );

      // Distance = |0.50 - 0.45| / 0.50 * 100 = 10%
      expect(getByText('10.00% from current')).toBeInTheDocument();
    });

    it('should show warning icon when liquidation is near', () => {
      const { getByText } = render(
        <RiskMetrics 
          {...defaultProps} 
          liquidationPrice={0.495} 
          entryPrice={0.50}
        />
      );

      // Distance = 1%, should show warning
      expect(getByText('⚠️')).toBeInTheDocument();
    });

    it('should not show warning when liquidation is far', () => {
      const { queryByText } = render(
        <RiskMetrics 
          {...defaultProps} 
          liquidationPrice={0.40} 
          entryPrice={0.50}
        />
      );

      expect(queryByText('⚠️')).not.toBeInTheDocument();
    });

    it('should apply danger styling when liquidation is very close', () => {
      const { container } = render(
        <RiskMetrics 
          {...defaultProps} 
          liquidationPrice={0.497} 
          entryPrice={0.50}
        />
      );

      const dangerCard = container.querySelector('[danger="true"]');
      expect(dangerCard).toBeInTheDocument();
    });
  });

  describe('Max Loss Calculation', () => {
    it('should calculate max loss based on leverage and volatility', () => {
      const { getByText } = render(
        <RiskMetrics {...defaultProps} />
      );

      // Max loss = 10 * 0.05 * 100 = $50
      expect(getByText('$50.00')).toBeInTheDocument();
      expect(getByText('per $100 position')).toBeInTheDocument();
    });

    it('should apply danger color for high max loss', () => {
      const { container } = render(
        <RiskMetrics 
          {...defaultProps} 
          leverage={100} 
          marketVolatility={0.10}
        />
      );

      // Max loss = 100 * 0.10 * 100 = $1000
      const maxLossValue = container.querySelector('[color="#DC2626"]');
      expect(maxLossValue).toBeInTheDocument();
      expect(maxLossValue).toHaveTextContent('$1000.00');
    });
  });

  describe('Risk Level Assessment', () => {
    it('should show low risk for leverage < 50', () => {
      const { getByText } = render(
        <RiskMetrics {...defaultProps} leverage={25} />
      );

      expect(getByText('LOW')).toBeInTheDocument();
      expect(getByText('Based on 25x leverage')).toBeInTheDocument();
    });

    it('should show medium risk for leverage 50-100', () => {
      const { getByText } = render(
        <RiskMetrics {...defaultProps} leverage={75} />
      );

      expect(getByText('MEDIUM')).toBeInTheDocument();
    });

    it('should show high risk for leverage > 100', () => {
      const { getByText } = render(
        <RiskMetrics {...defaultProps} leverage={150} />
      );

      expect(getByText('HIGH')).toBeInTheDocument();
    });

    it('should apply correct colors for risk levels', () => {
      const { rerender, container } = render(
        <RiskMetrics {...defaultProps} leverage={25} />
      );

      let riskValue = container.querySelector('[color="#00FF88"]');
      expect(riskValue).toHaveTextContent('LOW');

      rerender(<RiskMetrics {...defaultProps} leverage={75} />);
      riskValue = container.querySelector('[color="#FFB800"]');
      expect(riskValue).toHaveTextContent('MEDIUM');

      rerender(<RiskMetrics {...defaultProps} leverage={150} />);
      riskValue = container.querySelector('[color="#DC2626"]');
      expect(riskValue).toHaveTextContent('HIGH');
    });
  });

  describe('Market Volatility Display', () => {
    it('should display market volatility as percentage', () => {
      const { getByText } = render(
        <RiskMetrics {...defaultProps} />
      );

      expect(getByText('5.0%')).toBeInTheDocument();
      expect(getByText('24h average')).toBeInTheDocument();
    });

    it('should handle different volatility values', () => {
      const { getByText, rerender } = render(
        <RiskMetrics {...defaultProps} marketVolatility={0.025} />
      );

      expect(getByText('2.5%')).toBeInTheDocument();

      rerender(<RiskMetrics {...defaultProps} marketVolatility={0.15} />);
      expect(getByText('15.0%')).toBeInTheDocument();
    });

    it('should use default volatility when not provided', () => {
      const { getByText } = render(
        <RiskMetrics 
          leverage={10}
          liquidationPrice={0.45}
          entryPrice={0.50}
        />
      );

      // Default is 0.05 = 5%
      expect(getByText('5.0%')).toBeInTheDocument();
    });
  });

  describe('Progress Bar Animation', () => {
    it('should render progress bar for liquidation distance', () => {
      const { container } = render(
        <RiskMetrics {...defaultProps} />
      );

      const progressBar = container.querySelector('[style*="height: 4px"]');
      expect(progressBar).toBeInTheDocument();
    });

    it('should animate progress fill', () => {
      const { container } = render(
        <RiskMetrics {...defaultProps} />
      );

      const progressFill = container.querySelector('[initial]');
      expect(progressFill).toBeInTheDocument();
      expect(progressFill).toHaveAttribute('animate');
    });

    it('should apply danger styling to progress bar when close', () => {
      const { container } = render(
        <RiskMetrics 
          {...defaultProps} 
          liquidationPrice={0.495}
          entryPrice={0.50}
        />
      );

      const progressFill = container.querySelector('[danger="true"]');
      expect(progressFill).toBeInTheDocument();
    });

    it('should cap progress bar at 100%', () => {
      const { container } = render(
        <RiskMetrics 
          {...defaultProps} 
          liquidationPrice={0.30}
          entryPrice={0.50}
        />
      );

      // Distance is 40%, but progress bar should cap at 100%
      const progressFill = container.querySelector('[animate]');
      expect(progressFill).toHaveAttribute('animate', expect.stringContaining('width: 100%'));
    });
  });

  describe('Edge Cases', () => {
    it('should handle missing entry price', () => {
      const { queryByText } = render(
        <RiskMetrics 
          leverage={10}
          liquidationPrice={0.45}
        />
      );

      // Should not show distance calculation
      expect(queryByText('from current')).not.toBeInTheDocument();
    });

    it('should handle zero leverage', () => {
      const { container } = render(
        <RiskMetrics 
          {...defaultProps}
          leverage={0}
        />
      );

      expect(container).toBeInTheDocument();
    });

    it('should handle extreme leverage values', () => {
      const { getByText } = render(
        <RiskMetrics 
          {...defaultProps}
          leverage={1000}
        />
      );

      expect(getByText('HIGH')).toBeInTheDocument();
      expect(getByText('$5000.00')).toBeInTheDocument(); // Max loss
    });

    it('should handle very small volatility', () => {
      const { getByText } = render(
        <RiskMetrics 
          {...defaultProps}
          marketVolatility={0.001}
        />
      );

      expect(getByText('0.1%')).toBeInTheDocument();
    });
  });

  describe('Hover Effects', () => {
    it('should apply hover scale to metric cards', () => {
      const { container } = render(
        <RiskMetrics {...defaultProps} />
      );

      const cards = container.querySelectorAll('[whileHover]');
      expect(cards.length).toBe(4); // All 4 metric cards

      cards.forEach(card => {
        expect(card).toHaveAttribute('whileHover');
      });
    });
  });

  describe('Price Formatting', () => {
    it('should format prices correctly', () => {
      const testCases = [
        { price: 0.12345, expected: '12.35%' },
        { price: 0.5, expected: '50.00%' },
        { price: 0.999, expected: '99.90%' },
        { price: 0.001, expected: '0.10%' }
      ];

      testCases.forEach(({ price, expected }) => {
        const { getByText } = render(
          <RiskMetrics {...defaultProps} liquidationPrice={price} />
        );

        expect(getByText(expected)).toBeInTheDocument();
      });
    });
  });
});