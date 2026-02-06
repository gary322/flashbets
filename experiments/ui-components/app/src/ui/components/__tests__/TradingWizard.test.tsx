// Comprehensive tests for TradingWizard component
import React from 'react';
import { render, screen, fireEvent, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ThemeProvider } from '@mui/material/styles';
import { TradingWizard } from '../onboarding/TradingWizard';
import { createTheme } from '@mui/material/styles';

const darkTheme = createTheme({
  palette: {
    mode: 'dark',
  },
});

const renderWithTheme = (component: React.ReactElement) => {
  return render(
    <ThemeProvider theme={darkTheme}>
      {component}
    </ThemeProvider>
  );
};

describe('TradingWizard Component', () => {
  const mockOnClose = jest.fn();
  const mockOnComplete = jest.fn();

  beforeEach(() => {
    jest.clearAllMocks();
  });

  describe('Basic Rendering', () => {
    it('should render wizard dialog when open', () => {
      renderWithTheme(
        <TradingWizard
          open={true}
          onClose={mockOnClose}
          onComplete={mockOnComplete}
        />
      );

      expect(screen.getByText('Welcome to the Future of Prediction Markets')).toBeInTheDocument();
      expect(screen.getByText('500x+ Leverage')).toBeInTheDocument();
      expect(screen.getByText('30%+ Yields')).toBeInTheDocument();
      expect(screen.getByText('No KYC')).toBeInTheDocument();
    });

    it('should not render when closed', () => {
      const { container } = renderWithTheme(
        <TradingWizard
          open={false}
          onClose={mockOnClose}
          onComplete={mockOnComplete}
        />
      );

      expect(container.firstChild).toBeNull();
    });

    it('should render stepper with all steps', () => {
      renderWithTheme(
        <TradingWizard
          open={true}
          onClose={mockOnClose}
          onComplete={mockOnComplete}
        />
      );

      expect(screen.getByText('Welcome')).toBeInTheDocument();
      expect(screen.getByText('Experience Level')).toBeInTheDocument();
      expect(screen.getByText('Initial Setup')).toBeInTheDocument();
      expect(screen.getByText('Leverage & Chaining')).toBeInTheDocument();
      expect(screen.getByText('Complete')).toBeInTheDocument();
    });
  });

  describe('Step Navigation', () => {
    it('should navigate through steps with Next button', async () => {
      renderWithTheme(
        <TradingWizard
          open={true}
          onClose={mockOnClose}
          onComplete={mockOnComplete}
        />
      );

      // Step 1: Welcome
      expect(screen.getByText('Welcome to the Future of Prediction Markets')).toBeInTheDocument();
      
      const nextButton = screen.getByText('Next');
      await userEvent.click(nextButton);

      // Step 2: Experience Level
      await waitFor(() => {
        expect(screen.getByText("What's your experience with prediction markets?")).toBeInTheDocument();
      });

      // Select experience level
      const beginnerOption = screen.getByText('beginner');
      await userEvent.click(beginnerOption);
      await userEvent.click(nextButton);

      // Step 3: Initial Setup
      await waitFor(() => {
        expect(screen.getByText("Let's configure your trading settings")).toBeInTheDocument();
      });
    });

    it('should navigate back with Back button', async () => {
      renderWithTheme(
        <TradingWizard
          open={true}
          onClose={mockOnClose}
          onComplete={mockOnComplete}
        />
      );

      // Go to step 2
      await userEvent.click(screen.getByText('Next'));
      
      await waitFor(() => {
        expect(screen.getByText("What's your experience with prediction markets?")).toBeInTheDocument();
      });

      // Go back
      await userEvent.click(screen.getByText('Back'));

      await waitFor(() => {
        expect(screen.getByText('Welcome to the Future of Prediction Markets')).toBeInTheDocument();
      });
    });

    it('should close wizard with Cancel button on first step', async () => {
      renderWithTheme(
        <TradingWizard
          open={true}
          onClose={mockOnClose}
          onComplete={mockOnComplete}
        />
      );

      await userEvent.click(screen.getByText('Cancel'));
      expect(mockOnClose).toHaveBeenCalled();
    });
  });

  describe('Experience Level Selection', () => {
    it('should allow selecting experience level', async () => {
      renderWithTheme(
        <TradingWizard
          open={true}
          onClose={mockOnClose}
          onComplete={mockOnComplete}
        />
      );

      // Navigate to experience step
      await userEvent.click(screen.getByText('Next'));

      // Click on intermediate option
      const intermediateOption = screen.getByText('intermediate');
      await userEvent.click(intermediateOption);

      // Verify visual feedback
      const intermediateCard = intermediateOption.closest('[class*="MuiPaper"]');
      expect(intermediateCard).toHaveStyle({
        backgroundColor: expect.stringContaining('rgba(130, 71, 229'),
      });
    });

    it('should show correct descriptions for each level', async () => {
      renderWithTheme(
        <TradingWizard
          open={true}
          onClose={mockOnClose}
          onComplete={mockOnComplete}
        />
      );

      await userEvent.click(screen.getByText('Next'));

      expect(screen.getByText("I'm new to prediction markets and want to learn")).toBeInTheDocument();
      expect(screen.getByText("I've traded before but want to explore advanced features")).toBeInTheDocument();
      expect(screen.getByText("I'm experienced and want full control")).toBeInTheDocument();
    });
  });

  describe('Initial Setup Configuration', () => {
    it('should allow configuring initial deposit', async () => {
      renderWithTheme(
        <TradingWizard
          open={true}
          onClose={mockOnClose}
          onComplete={mockOnComplete}
        />
      );

      // Navigate to setup step
      await userEvent.click(screen.getByText('Next'));
      await userEvent.click(screen.getByText('beginner'));
      await userEvent.click(screen.getByText('Next'));

      // Find deposit input
      const depositInput = screen.getByRole('spinbutton');
      
      // Clear and type new value
      await userEvent.clear(depositInput);
      await userEvent.type(depositInput, '500');

      expect(depositInput).toHaveValue(500);
    });

    it('should validate minimum deposit', async () => {
      renderWithTheme(
        <TradingWizard
          open={true}
          onClose={mockOnClose}
          onComplete={mockOnComplete}
        />
      );

      // Navigate to setup step
      await userEvent.click(screen.getByText('Next'));
      await userEvent.click(screen.getByText('beginner'));
      await userEvent.click(screen.getByText('Next'));

      // Set deposit below minimum
      const depositInput = screen.getByRole('spinbutton');
      await userEvent.clear(depositInput);
      await userEvent.type(depositInput, '5');

      // Try to proceed
      await userEvent.click(screen.getByText('Next'));

      // Should not advance (stays on same step)
      expect(screen.getByText("Let's configure your trading settings")).toBeInTheDocument();
    });

    it('should configure risk tolerance', async () => {
      renderWithTheme(
        <TradingWizard
          open={true}
          onClose={mockOnClose}
          onComplete={mockOnComplete}
        />
      );

      // Navigate to setup step
      await userEvent.click(screen.getByText('Next'));
      await userEvent.click(screen.getByText('beginner'));
      await userEvent.click(screen.getByText('Next'));

      // Check risk tolerance slider labels
      expect(screen.getByText('Conservative')).toBeInTheDocument();
      expect(screen.getByText('Moderate')).toBeInTheDocument();
      expect(screen.getByText('Aggressive')).toBeInTheDocument();
    });
  });

  describe('Leverage and Chaining Configuration', () => {
    const navigateToLeverageStep = async () => {
      await userEvent.click(screen.getByText('Next'));
      await userEvent.click(screen.getByText('beginner'));
      await userEvent.click(screen.getByText('Next'));
      await userEvent.click(screen.getByText('Next'));
    };

    it('should configure leverage preference', async () => {
      renderWithTheme(
        <TradingWizard
          open={true}
          onClose={mockOnClose}
          onComplete={mockOnComplete}
        />
      );

      await navigateToLeverageStep();

      // Check leverage slider is present
      expect(screen.getByText('Preferred Leverage')).toBeInTheDocument();
      expect(screen.getByText('10x')).toBeInTheDocument();
      expect(screen.getByText('50x')).toBeInTheDocument();
      expect(screen.getByText('100x')).toBeInTheDocument();
    });

    it('should toggle auto-chaining', async () => {
      renderWithTheme(
        <TradingWizard
          open={true}
          onClose={mockOnClose}
          onComplete={mockOnComplete}
        />
      );

      await navigateToLeverageStep();

      const autoChainButton = screen.getByRole('button', { name: /Disabled/i });
      await userEvent.click(autoChainButton);

      expect(screen.getByRole('button', { name: /Enabled/i })).toBeInTheDocument();
    });

    it('should show effective leverage calculation with auto-chain', async () => {
      renderWithTheme(
        <TradingWizard
          open={true}
          onClose={mockOnClose}
          onComplete={mockOnComplete}
        />
      );

      await navigateToLeverageStep();

      // Enable auto-chain
      const autoChainButton = screen.getByRole('button', { name: /Disabled/i });
      await userEvent.click(autoChainButton);

      // Check chain multiplier explanation
      expect(screen.getByText(/Chain multiplier:/)).toBeInTheDocument();
      expect(screen.getByText(/Borrow \(1.5x\)/)).toBeInTheDocument();
    });
  });

  describe('Completion Flow', () => {
    const completeWizard = async () => {
      // Step through all screens
      await userEvent.click(screen.getByText('Next')); // Welcome
      await userEvent.click(screen.getByText('intermediate')); // Experience
      await userEvent.click(screen.getByText('Next'));
      
      // Initial setup - increase deposit
      const depositInput = screen.getByRole('spinbutton');
      await userEvent.clear(depositInput);
      await userEvent.type(depositInput, '250');
      await userEvent.click(screen.getByText('Next'));
      
      // Leverage - enable auto-chain
      const autoChainButton = screen.getByRole('button', { name: /Disabled/i });
      await userEvent.click(autoChainButton);
      await userEvent.click(screen.getByText('Next'));
    };

    it('should show completion summary', async () => {
      renderWithTheme(
        <TradingWizard
          open={true}
          onClose={mockOnClose}
          onComplete={mockOnComplete}
        />
      );

      await completeWizard();

      expect(screen.getByText("You're All Set!")).toBeInTheDocument();
      expect(screen.getByText('Experience: intermediate')).toBeInTheDocument();
      expect(screen.getByText('Deposit: $250')).toBeInTheDocument();
      expect(screen.getByText('Auto-Chain: Enabled')).toBeInTheDocument();
    });

    it('should call onComplete with settings', async () => {
      renderWithTheme(
        <TradingWizard
          open={true}
          onClose={mockOnClose}
          onComplete={mockOnComplete}
        />
      );

      await completeWizard();

      await userEvent.click(screen.getByText('Start Trading'));

      expect(mockOnComplete).toHaveBeenCalledWith({
        experience: 'intermediate',
        initialDeposit: 250,
        riskTolerance: 50,
        preferredLeverage: expect.any(Number),
        enableAutoChain: true,
      });
    });
  });

  describe('Demo Visualizations', () => {
    it('should show trading power calculation', async () => {
      renderWithTheme(
        <TradingWizard
          open={true}
          onClose={mockOnClose}
          onComplete={mockOnComplete}
        />
      );

      // Navigate to setup step
      await userEvent.click(screen.getByText('Next'));
      await userEvent.click(screen.getByText('beginner'));
      await userEvent.click(screen.getByText('Next'));

      // Check demo box shows calculation
      const demoBox = screen.getByText(/Total trading power/i).closest('div');
      expect(demoBox).toBeInTheDocument();
    });
  });

  describe('Animation and Transitions', () => {
    it('should animate between steps', async () => {
      renderWithTheme(
        <TradingWizard
          open={true}
          onClose={mockOnClose}
          onComplete={mockOnComplete}
        />
      );

      const welcomeText = screen.getByText('Welcome to the Future of Prediction Markets');
      expect(welcomeText).toBeVisible();

      await userEvent.click(screen.getByText('Next'));

      // Content should transition
      await waitFor(() => {
        expect(welcomeText).not.toBeInTheDocument();
      });
    });
  });

  describe('Keyboard Navigation', () => {
    it('should support keyboard navigation', async () => {
      renderWithTheme(
        <TradingWizard
          open={true}
          onClose={mockOnClose}
          onComplete={mockOnComplete}
        />
      );

      // Tab to Next button
      const nextButton = screen.getByText('Next');
      nextButton.focus();
      expect(document.activeElement).toBe(nextButton);

      // Press Enter
      fireEvent.keyDown(nextButton, { key: 'Enter' });

      await waitFor(() => {
        expect(screen.getByText("What's your experience with prediction markets?")).toBeInTheDocument();
      });
    });

    it('should handle Escape key to close', async () => {
      renderWithTheme(
        <TradingWizard
          open={true}
          onClose={mockOnClose}
          onComplete={mockOnComplete}
        />
      );

      fireEvent.keyDown(document.body, { key: 'Escape' });
      
      expect(mockOnClose).toHaveBeenCalled();
    });
  });

  describe('Edge Cases', () => {
    it('should handle rapid clicking', async () => {
      renderWithTheme(
        <TradingWizard
          open={true}
          onClose={mockOnClose}
          onComplete={mockOnComplete}
        />
      );

      const nextButton = screen.getByText('Next');
      
      // Rapid clicks
      for (let i = 0; i < 5; i++) {
        fireEvent.click(nextButton);
      }

      // Should only advance one step
      await waitFor(() => {
        expect(screen.getByText("What's your experience with prediction markets?")).toBeInTheDocument();
      });
    });

    it('should persist settings through navigation', async () => {
      renderWithTheme(
        <TradingWizard
          open={true}
          onClose={mockOnClose}
          onComplete={mockOnComplete}
        />
      );

      // Go to experience and select
      await userEvent.click(screen.getByText('Next'));
      await userEvent.click(screen.getByText('advanced'));
      await userEvent.click(screen.getByText('Next'));

      // Go to leverage step
      await userEvent.click(screen.getByText('Next'));

      // Go back to experience
      await userEvent.click(screen.getByText('Back'));
      await userEvent.click(screen.getByText('Back'));

      // Advanced should still be selected
      const advancedOption = screen.getByText('advanced');
      const advancedCard = advancedOption.closest('[class*="MuiPaper"]');
      expect(advancedCard).toHaveStyle({
        backgroundColor: expect.stringContaining('rgba(130, 71, 229'),
      });
    });
  });
});