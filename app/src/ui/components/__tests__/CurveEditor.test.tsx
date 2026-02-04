// Comprehensive tests for CurveEditor component
import React from 'react';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ThemeProvider } from '@mui/material/styles';
import { CurveEditor } from '../trading/CurveEditor';
import { createTheme } from '@mui/material/styles';

// Mock dark theme for testing
const darkTheme = createTheme({
  palette: {
    mode: 'dark',
  },
});

// Helper to render with theme
const renderWithTheme = (component: React.ReactElement) => {
  return render(
    <ThemeProvider theme={darkTheme}>
      {component}
    </ThemeProvider>
  );
};

describe('CurveEditor Component', () => {
  const mockOnChange = jest.fn();
  const mockOnSave = jest.fn();

  const defaultDistribution = {
    mean: 5000,
    variance: 1000,
    skewness: 0,
    kurtosis: 0,
  };

  beforeEach(() => {
    jest.clearAllMocks();
  });

  describe('Basic Rendering', () => {
    it('should render the curve editor with all controls', () => {
      renderWithTheme(
        <CurveEditor
          distribution={defaultDistribution}
          onChange={mockOnChange}
          marketId="test-market"
        />
      );

      // Check title
      expect(screen.getByText('Probability Distribution')).toBeInTheDocument();

      // Check all sliders are present
      expect(screen.getByText('Mean')).toBeInTheDocument();
      expect(screen.getByText('Variance')).toBeInTheDocument();
      expect(screen.getByText('Skewness')).toBeInTheDocument();
      expect(screen.getByText('Kurtosis')).toBeInTheDocument();

      // Check action buttons
      expect(screen.getByText('Reset')).toBeInTheDocument();
      expect(screen.getByText('Optimize')).toBeInTheDocument();
      expect(screen.getByText('Smooth')).toBeInTheDocument();
    });

    it('should display correct initial values', () => {
      renderWithTheme(
        <CurveEditor
          distribution={defaultDistribution}
          onChange={mockOnChange}
          marketId="test-market"
        />
      );

      // Check that initial values are displayed
      expect(screen.getByText('50.00%')).toBeInTheDocument(); // Mean
      expect(screen.getByText('10.00%')).toBeInTheDocument(); // Std Dev
    });
  });

  describe('Slider Interactions', () => {
    it('should update mean when slider is moved', async () => {
      const { container } = renderWithTheme(
        <CurveEditor
          distribution={defaultDistribution}
          onChange={mockOnChange}
          marketId="test-market"
        />
      );

      // Find mean slider
      const meanSlider = container.querySelector('[aria-label="Mean"]');
      expect(meanSlider).toBeTruthy();

      // Simulate slider change
      fireEvent.change(meanSlider!, { target: { value: 7000 } });

      await waitFor(() => {
        expect(mockOnChange).toHaveBeenCalledWith({
          ...defaultDistribution,
          mean: 7000,
        });
      });
    });

    it('should update variance when slider is moved', async () => {
      const { container } = renderWithTheme(
        <CurveEditor
          distribution={defaultDistribution}
          onChange={mockOnChange}
          marketId="test-market"
        />
      );

      const varianceSlider = container.querySelector('[aria-label="Variance"]');
      fireEvent.change(varianceSlider!, { target: { value: 2000 } });

      await waitFor(() => {
        expect(mockOnChange).toHaveBeenCalledWith({
          ...defaultDistribution,
          variance: 2000,
        });
      });
    });

    it('should update skewness when slider is moved', async () => {
      const { container } = renderWithTheme(
        <CurveEditor
          distribution={defaultDistribution}
          onChange={mockOnChange}
          marketId="test-market"
        />
      );

      const skewnessSlider = container.querySelector('[aria-label="Skewness"]');
      fireEvent.change(skewnessSlider!, { target: { value: 50 } });

      await waitFor(() => {
        expect(mockOnChange).toHaveBeenCalledWith({
          ...defaultDistribution,
          skewness: 50,
        });
      });
    });

    it('should update kurtosis when slider is moved', async () => {
      const { container } = renderWithTheme(
        <CurveEditor
          distribution={defaultDistribution}
          onChange={mockOnChange}
          marketId="test-market"
        />
      );

      const kurtosisSlider = container.querySelector('[aria-label="Kurtosis"]');
      fireEvent.change(kurtosisSlider!, { target: { value: -50 } });

      await waitFor(() => {
        expect(mockOnChange).toHaveBeenCalledWith({
          ...defaultDistribution,
          kurtosis: -50,
        });
      });
    });
  });

  describe('Button Actions', () => {
    it('should reset distribution when Reset is clicked', async () => {
      renderWithTheme(
        <CurveEditor
          distribution={{
            mean: 7000,
            variance: 2000,
            skewness: 50,
            kurtosis: -25,
          }}
          onChange={mockOnChange}
          marketId="test-market"
        />
      );

      const resetButton = screen.getByText('Reset');
      await userEvent.click(resetButton);

      expect(mockOnChange).toHaveBeenCalledWith({
        mean: 5000,
        variance: 1000,
        skewness: 0,
        kurtosis: 0,
      });
    });

    it('should optimize distribution when Optimize is clicked', async () => {
      renderWithTheme(
        <CurveEditor
          distribution={defaultDistribution}
          onChange={mockOnChange}
          marketId="test-market"
        />
      );

      const optimizeButton = screen.getByText('Optimize');
      await userEvent.click(optimizeButton);

      // Should call onChange with optimized values
      expect(mockOnChange).toHaveBeenCalled();
      const optimizedDist = mockOnChange.mock.calls[0][0];
      
      // Verify optimization adjusts variance
      expect(optimizedDist.variance).toBeLessThan(defaultDistribution.variance);
      expect(optimizedDist.skewness).toBe(0);
      expect(optimizedDist.kurtosis).toBe(0);
    });

    it('should smooth distribution when Smooth is clicked', async () => {
      renderWithTheme(
        <CurveEditor
          distribution={{
            mean: 5000,
            variance: 3000,
            skewness: 100,
            kurtosis: 100,
          }}
          onChange={mockOnChange}
          marketId="test-market"
        />
      );

      const smoothButton = screen.getByText('Smooth');
      await userEvent.click(smoothButton);

      // Should reduce extreme values
      const smoothedDist = mockOnChange.mock.calls[0][0];
      expect(Math.abs(smoothedDist.skewness)).toBeLessThan(100);
      expect(Math.abs(smoothedDist.kurtosis)).toBeLessThan(100);
    });

    it('should call onSave when provided and Save is clicked', async () => {
      renderWithTheme(
        <CurveEditor
          distribution={defaultDistribution}
          onChange={mockOnChange}
          onSave={mockOnSave}
          marketId="test-market"
        />
      );

      const saveButton = screen.getByText('Save');
      await userEvent.click(saveButton);

      expect(mockOnSave).toHaveBeenCalledWith(defaultDistribution);
    });
  });

  describe('Curve Visualization', () => {
    it('should render the curve visualization canvas', () => {
      const { container } = renderWithTheme(
        <CurveEditor
          distribution={defaultDistribution}
          onChange={mockOnChange}
          marketId="test-market"
        />
      );

      const canvas = container.querySelector('canvas');
      expect(canvas).toBeTruthy();
      expect(canvas).toHaveAttribute('width', '400');
      expect(canvas).toHaveAttribute('height', '200');
    });

    it('should update curve when distribution changes', async () => {
      const { rerender } = renderWithTheme(
        <CurveEditor
          distribution={defaultDistribution}
          onChange={mockOnChange}
          marketId="test-market"
        />
      );

      // Change distribution
      const newDistribution = {
        mean: 7000,
        variance: 500,
        skewness: 50,
        kurtosis: 0,
      };

      rerender(
        <ThemeProvider theme={darkTheme}>
          <CurveEditor
            distribution={newDistribution}
            onChange={mockOnChange}
            marketId="test-market"
          />
        </ThemeProvider>
      );

      // Verify mean percentage updated
      expect(screen.getByText('70.00%')).toBeInTheDocument();
    });
  });

  describe('Edge Cases', () => {
    it('should handle extreme values gracefully', async () => {
      const { container } = renderWithTheme(
        <CurveEditor
          distribution={defaultDistribution}
          onChange={mockOnChange}
          marketId="test-market"
        />
      );

      // Try to set mean to maximum
      const meanSlider = container.querySelector('[aria-label="Mean"]');
      fireEvent.change(meanSlider!, { target: { value: 10000 } });

      await waitFor(() => {
        expect(mockOnChange).toHaveBeenCalledWith({
          ...defaultDistribution,
          mean: 10000,
        });
      });

      // Try to set variance to minimum
      const varianceSlider = container.querySelector('[aria-label="Variance"]');
      fireEvent.change(varianceSlider!, { target: { value: 100 } });

      await waitFor(() => {
        expect(mockOnChange).toHaveBeenLastCalledWith({
          ...defaultDistribution,
          variance: 100,
        });
      });
    });

    it('should prevent invalid distribution states', async () => {
      renderWithTheme(
        <CurveEditor
          distribution={{
            mean: 5000,
            variance: 0, // Invalid: variance cannot be 0
            skewness: 0,
            kurtosis: 0,
          }}
          onChange={mockOnChange}
          marketId="test-market"
        />
      );

      // Component should handle gracefully and show minimum variance
      expect(screen.getByText('1.00%')).toBeInTheDocument(); // Min std dev
    });
  });

  describe('Keyboard Accessibility', () => {
    it('should be keyboard navigable', async () => {
      renderWithTheme(
        <CurveEditor
          distribution={defaultDistribution}
          onChange={mockOnChange}
          marketId="test-market"
        />
      );

      // Tab through elements
      const resetButton = screen.getByText('Reset');
      resetButton.focus();
      expect(document.activeElement).toBe(resetButton);

      // Press Enter on focused button
      fireEvent.keyDown(resetButton, { key: 'Enter' });
      expect(mockOnChange).toHaveBeenCalled();
    });
  });

  describe('Performance', () => {
    it('should throttle slider updates', async () => {
      const { container } = renderWithTheme(
        <CurveEditor
          distribution={defaultDistribution}
          onChange={mockOnChange}
          marketId="test-market"
        />
      );

      const meanSlider = container.querySelector('[aria-label="Mean"]');
      
      // Rapid slider movements
      for (let i = 0; i < 10; i++) {
        fireEvent.change(meanSlider!, { target: { value: 5000 + i * 100 } });
      }

      // Should not call onChange 10 times due to throttling
      await waitFor(() => {
        expect(mockOnChange).toHaveBeenCalledTimes(1);
      });
    });
  });
});