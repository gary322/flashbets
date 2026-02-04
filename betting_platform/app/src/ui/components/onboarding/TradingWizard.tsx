// Trading Wizard for Beginners
// Step-by-step onboarding with interactive tutorials

import React, { useState, useEffect } from 'react';
import {
  Box,
  Button,
  Dialog,
  DialogContent,
  DialogActions,
  Typography,
  Stepper,
  Step,
  StepLabel,
  StepContent,
  Paper,
  TextField,
  Slider,
  Chip,
  IconButton,
  Fade,
  Zoom,
  Tooltip,
} from '@mui/material';
import { styled } from '@mui/material/styles';
import ArrowForwardIcon from '@mui/icons-material/ArrowForward';
import ArrowBackIcon from '@mui/icons-material/ArrowBack';
import CheckCircleIcon from '@mui/icons-material/CheckCircle';
import InfoIcon from '@mui/icons-material/Info';
import RocketLaunchIcon from '@mui/icons-material/RocketLaunch';
import TrendingUpIcon from '@mui/icons-material/TrendingUp';
import AccountBalanceWalletIcon from '@mui/icons-material/AccountBalanceWallet';

// Styled components with Blur aesthetic
const WizardDialog = styled(Dialog)(({ theme }) => ({
  '& .MuiDialog-paper': {
    backgroundColor: '#0A0A0A',
    backgroundImage: 'radial-gradient(circle at 20% 50%, rgba(130, 71, 229, 0.1) 0%, transparent 50%)',
    border: '1px solid rgba(255, 255, 255, 0.1)',
    borderRadius: '16px',
    maxWidth: '600px',
    overflow: 'visible',
  },
}));

const StyledStepper = styled(Stepper)(({ theme }) => ({
  backgroundColor: 'transparent',
  '& .MuiStepLabel-label': {
    color: 'rgba(255, 255, 255, 0.7)',
    fontFamily: 'Inter',
    fontSize: '14px',
    '&.Mui-active': {
      color: '#8247E5',
      fontWeight: 600,
    },
    '&.Mui-completed': {
      color: 'rgba(130, 71, 229, 0.8)',
    },
  },
  '& .MuiStepIcon-root': {
    color: 'rgba(255, 255, 255, 0.1)',
    '&.Mui-active': {
      color: '#8247E5',
    },
    '&.Mui-completed': {
      color: '#8247E5',
    },
  },
  '& .MuiStepConnector-line': {
    borderColor: 'rgba(255, 255, 255, 0.1)',
  },
}));

const DemoBox = styled(Paper)(({ theme }) => ({
  backgroundColor: 'rgba(255, 255, 255, 0.02)',
  border: '1px solid rgba(255, 255, 255, 0.1)',
  borderRadius: '8px',
  padding: theme.spacing(3),
  marginTop: theme.spacing(2),
  marginBottom: theme.spacing(2),
  position: 'relative',
  overflow: 'hidden',
  '&::before': {
    content: '""',
    position: 'absolute',
    top: 0,
    left: 0,
    right: 0,
    bottom: 0,
    background: 'radial-gradient(circle at var(--mouse-x, 50%) var(--mouse-y, 50%), rgba(130, 71, 229, 0.15) 0%, transparent 40%)',
    opacity: 0,
    transition: 'opacity 0.3s ease',
    pointerEvents: 'none',
  },
  '&:hover::before': {
    opacity: 1,
  },
}));

const StyledButton = styled(Button)(({ theme }) => ({
  backgroundColor: '#8247E5',
  color: '#FFFFFF',
  fontWeight: 600,
  borderRadius: '8px',
  padding: '12px 24px',
  textTransform: 'none',
  '&:hover': {
    backgroundColor: '#6B3AA0',
    boxShadow: '0 0 20px rgba(130, 71, 229, 0.4)',
  },
  '&.Mui-disabled': {
    backgroundColor: 'rgba(255, 255, 255, 0.05)',
    color: 'rgba(255, 255, 255, 0.3)',
  },
}));

const InfoChip = styled(Chip)(({ theme }) => ({
  backgroundColor: 'rgba(130, 71, 229, 0.2)',
  color: '#8247E5',
  border: '1px solid rgba(130, 71, 229, 0.3)',
  fontFamily: 'JetBrains Mono',
  fontSize: '12px',
}));

interface WizardStep {
  label: string;
  description: string;
  content: React.ReactNode;
  demo?: React.ReactNode;
  validation?: () => boolean;
}

interface TradingWizardProps {
  open: boolean;
  onClose: () => void;
  onComplete: (settings: WizardSettings) => void;
}

interface WizardSettings {
  experience: 'beginner' | 'intermediate' | 'advanced';
  initialDeposit: number;
  riskTolerance: number;
  preferredLeverage: number;
  enableAutoChain: boolean;
}

export const TradingWizard: React.FC<TradingWizardProps> = ({
  open,
  onClose,
  onComplete,
}) => {
  const [activeStep, setActiveStep] = useState(0);
  const [settings, setSettings] = useState<WizardSettings>({
    experience: 'beginner',
    initialDeposit: 100,
    riskTolerance: 50,
    preferredLeverage: 10,
    enableAutoChain: false,
  });
  const [demoValue, setDemoValue] = useState(0);
  const [isAnimating, setIsAnimating] = useState(false);

  // Mouse tracking for interactive effects
  useEffect(() => {
    const handleMouseMove = (e: MouseEvent) => {
      const elements = document.querySelectorAll('.demo-box');
      elements.forEach((el) => {
        const rect = el.getBoundingClientRect();
        const x = ((e.clientX - rect.left) / rect.width) * 100;
        const y = ((e.clientY - rect.top) / rect.height) * 100;
        (el as HTMLElement).style.setProperty('--mouse-x', `${x}%`);
        (el as HTMLElement).style.setProperty('--mouse-y', `${y}%`);
      });
    };

    if (open) {
      document.addEventListener('mousemove', handleMouseMove);
      return () => document.removeEventListener('mousemove', handleMouseMove);
    }
  }, [open]);

  const steps: WizardStep[] = [
    {
      label: 'Welcome',
      description: 'Start your prediction market journey',
      content: (
        <Box>
          <Typography variant="h4" sx={{ color: '#FFFFFF', mb: 2, fontWeight: 700 }}>
            Welcome to the Future of Prediction Markets
          </Typography>
          <Typography sx={{ color: 'rgba(255, 255, 255, 0.7)', mb: 3 }}>
            Let's set up your trading experience in just a few steps. This wizard will help you
            understand our platform and configure it to match your trading style.
          </Typography>
          <Box display="flex" gap={2} flexWrap="wrap">
            <InfoChip icon={<RocketLaunchIcon />} label="500x+ Leverage" />
            <InfoChip icon={<TrendingUpIcon />} label="30%+ Yields" />
            <InfoChip icon={<AccountBalanceWalletIcon />} label="No KYC" />
          </Box>
        </Box>
      ),
      demo: (
        <Box textAlign="center" py={4}>
          <RocketLaunchIcon sx={{ fontSize: 80, color: '#8247E5', mb: 2 }} />
          <Typography variant="h6" sx={{ color: 'rgba(255, 255, 255, 0.9)' }}>
            Ready to maximize your predictions?
          </Typography>
        </Box>
      ),
    },
    {
      label: 'Experience Level',
      description: 'Tell us about your trading experience',
      content: (
        <Box>
          <Typography variant="h6" sx={{ color: '#FFFFFF', mb: 3 }}>
            What's your experience with prediction markets?
          </Typography>
          <Box display="flex" flexDirection="column" gap={2}>
            {['beginner', 'intermediate', 'advanced'].map((level) => (
              <Paper
                key={level}
                sx={{
                  p: 2,
                  cursor: 'pointer',
                  backgroundColor: settings.experience === level ? 'rgba(130, 71, 229, 0.2)' : 'rgba(255, 255, 255, 0.02)',
                  border: settings.experience === level ? '2px solid #8247E5' : '1px solid rgba(255, 255, 255, 0.1)',
                  transition: 'all 0.3s ease',
                  '&:hover': {
                    backgroundColor: 'rgba(130, 71, 229, 0.1)',
                    borderColor: 'rgba(130, 71, 229, 0.5)',
                  },
                }}
                onClick={() => setSettings({ ...settings, experience: level as any })}
              >
                <Typography variant="subtitle1" sx={{ color: '#FFFFFF', textTransform: 'capitalize', mb: 1 }}>
                  {level}
                </Typography>
                <Typography variant="body2" sx={{ color: 'rgba(255, 255, 255, 0.6)' }}>
                  {level === 'beginner' && "I'm new to prediction markets and want to learn"}
                  {level === 'intermediate' && "I've traded before but want to explore advanced features"}
                  {level === 'advanced' && "I'm experienced and want full control"}
                </Typography>
              </Paper>
            ))}
          </Box>
        </Box>
      ),
      validation: () => true,
    },
    {
      label: 'Initial Setup',
      description: 'Configure your trading parameters',
      content: (
        <Box>
          <Typography variant="h6" sx={{ color: '#FFFFFF', mb: 3 }}>
            Let's configure your trading settings
          </Typography>
          
          <Box mb={3}>
            <Typography sx={{ color: 'rgba(255, 255, 255, 0.7)', mb: 1 }}>
              Initial Deposit (USDC)
            </Typography>
            <TextField
              fullWidth
              type="number"
              value={settings.initialDeposit}
              onChange={(e) => setSettings({ ...settings, initialDeposit: Number(e.target.value) })}
              sx={{
                '& .MuiInputBase-root': {
                  backgroundColor: 'rgba(255, 255, 255, 0.05)',
                  color: '#FFFFFF',
                },
                '& .MuiOutlinedInput-notchedOutline': {
                  borderColor: 'rgba(255, 255, 255, 0.2)',
                },
              }}
              InputProps={{
                startAdornment: <Typography sx={{ color: 'rgba(255, 255, 255, 0.5)', mr: 1 }}>$</Typography>,
              }}
            />
          </Box>

          <Box mb={3}>
            <Typography sx={{ color: 'rgba(255, 255, 255, 0.7)', mb: 1 }}>
              Risk Tolerance
            </Typography>
            <Slider
              value={settings.riskTolerance}
              onChange={(_, value) => setSettings({ ...settings, riskTolerance: value as number })}
              marks={[
                { value: 0, label: 'Conservative' },
                { value: 50, label: 'Moderate' },
                { value: 100, label: 'Aggressive' },
              ]}
              sx={{
                color: '#8247E5',
                '& .MuiSlider-mark': {
                  backgroundColor: 'rgba(255, 255, 255, 0.2)',
                },
                '& .MuiSlider-markLabel': {
                  color: 'rgba(255, 255, 255, 0.5)',
                  fontSize: '12px',
                },
              }}
            />
          </Box>
        </Box>
      ),
      demo: (
        <Box>
          <Typography variant="body2" sx={{ color: 'rgba(255, 255, 255, 0.7)', mb: 2 }}>
            With ${settings.initialDeposit} and {settings.preferredLeverage}x leverage:
          </Typography>
          <Typography variant="h4" sx={{ color: '#8247E5', mb: 1 }}>
            ${settings.initialDeposit * settings.preferredLeverage}
          </Typography>
          <Typography variant="body2" sx={{ color: 'rgba(255, 255, 255, 0.5)' }}>
            Total trading power
          </Typography>
        </Box>
      ),
      validation: () => settings.initialDeposit >= 10,
    },
    {
      label: 'Leverage & Chaining',
      description: 'Unlock the power of auto-chaining',
      content: (
        <Box>
          <Typography variant="h6" sx={{ color: '#FFFFFF', mb: 3 }}>
            Leverage and Auto-Chaining
          </Typography>
          
          <Box mb={3}>
            <Box display="flex" alignItems="center" mb={1}>
              <Typography sx={{ color: 'rgba(255, 255, 255, 0.7)' }}>
                Preferred Leverage
              </Typography>
              <Tooltip title="Higher leverage means higher potential returns but also higher risk">
                <IconButton size="small" sx={{ ml: 1 }}>
                  <InfoIcon sx={{ fontSize: 16, color: 'rgba(255, 255, 255, 0.5)' }} />
                </IconButton>
              </Tooltip>
            </Box>
            <Slider
              value={settings.preferredLeverage}
              onChange={(_, value) => setSettings({ ...settings, preferredLeverage: value as number })}
              min={1}
              max={100}
              marks={[
                { value: 10, label: '10x' },
                { value: 50, label: '50x' },
                { value: 100, label: '100x' },
              ]}
              sx={{
                color: '#8247E5',
                '& .MuiSlider-valueLabel': {
                  backgroundColor: '#8247E5',
                },
              }}
              valueLabelDisplay="on"
              valueLabelFormat={(value) => `${value}x`}
            />
          </Box>

          <Paper sx={{ p: 2, backgroundColor: 'rgba(130, 71, 229, 0.1)', border: '1px solid rgba(130, 71, 229, 0.3)' }}>
            <Box display="flex" alignItems="center" justifyContent="space-between" mb={1}>
              <Typography sx={{ color: '#FFFFFF' }}>
                Enable Auto-Chaining
              </Typography>
              <Button
                variant={settings.enableAutoChain ? "contained" : "outlined"}
                size="small"
                onClick={() => setSettings({ ...settings, enableAutoChain: !settings.enableAutoChain })}
                sx={{
                  borderColor: '#8247E5',
                  color: settings.enableAutoChain ? '#FFFFFF' : '#8247E5',
                  backgroundColor: settings.enableAutoChain ? '#8247E5' : 'transparent',
                }}
              >
                {settings.enableAutoChain ? 'Enabled' : 'Disabled'}
              </Button>
            </Box>
            <Typography variant="body2" sx={{ color: 'rgba(255, 255, 255, 0.6)' }}>
              Auto-chaining can boost your effective leverage up to 500x+ through
              automated borrow → liquidity → stake sequences
            </Typography>
          </Paper>
        </Box>
      ),
      demo: (
        <Box>
          <Typography variant="body2" sx={{ color: 'rgba(255, 255, 255, 0.7)', mb: 2 }}>
            Effective Leverage Calculation:
          </Typography>
          <Box display="flex" alignItems="center" gap={2} mb={2}>
            <InfoChip label={`Base: ${settings.preferredLeverage}x`} />
            {settings.enableAutoChain && (
              <>
                <ArrowForwardIcon sx={{ color: 'rgba(255, 255, 255, 0.5)' }} />
                <InfoChip label="Chain: 3.6x" />
                <ArrowForwardIcon sx={{ color: 'rgba(255, 255, 255, 0.5)' }} />
                <InfoChip 
                  label={`Total: ${Math.min(500, settings.preferredLeverage * 3.6).toFixed(0)}x`}
                  sx={{ backgroundColor: 'rgba(130, 71, 229, 0.4)' }}
                />
              </>
            )}
          </Box>
          {settings.enableAutoChain && (
            <Typography variant="caption" sx={{ color: 'rgba(255, 255, 255, 0.5)' }}>
              Chain multiplier: Borrow (1.5x) × Liquidity (1.2x) × Stake (1.2x) = 3.6x
            </Typography>
          )}
        </Box>
      ),
      validation: () => true,
    },
    {
      label: 'Complete',
      description: 'Ready to start trading!',
      content: (
        <Box textAlign="center">
          <CheckCircleIcon sx={{ fontSize: 64, color: '#8247E5', mb: 2 }} />
          <Typography variant="h5" sx={{ color: '#FFFFFF', mb: 2 }}>
            You're All Set!
          </Typography>
          <Typography sx={{ color: 'rgba(255, 255, 255, 0.7)', mb: 3 }}>
            Your trading environment is configured and ready. Here's a summary of your settings:
          </Typography>
          <Box display="flex" flexDirection="column" gap={1} alignItems="center">
            <InfoChip label={`Experience: ${settings.experience}`} />
            <InfoChip label={`Deposit: $${settings.initialDeposit}`} />
            <InfoChip label={`Leverage: ${settings.preferredLeverage}x`} />
            <InfoChip label={`Auto-Chain: ${settings.enableAutoChain ? 'Enabled' : 'Disabled'}`} />
          </Box>
        </Box>
      ),
    },
  ];

  const handleNext = () => {
    const currentValidation = steps[activeStep].validation;
    if (!currentValidation || currentValidation()) {
      if (activeStep === steps.length - 1) {
        onComplete(settings);
      } else {
        setIsAnimating(true);
        setTimeout(() => {
          setActiveStep((prev) => prev + 1);
          setIsAnimating(false);
        }, 300);
      }
    }
  };

  const handleBack = () => {
    setIsAnimating(true);
    setTimeout(() => {
      setActiveStep((prev) => prev - 1);
      setIsAnimating(false);
    }, 300);
  };

  const currentStep = steps[activeStep];

  return (
    <WizardDialog open={open} onClose={onClose} maxWidth="md" fullWidth>
      <DialogContent sx={{ p: 4 }}>
        <StyledStepper activeStep={activeStep} orientation="horizontal" sx={{ mb: 4 }}>
          {steps.map((step, index) => (
            <Step key={step.label}>
              <StepLabel>{step.label}</StepLabel>
            </Step>
          ))}
        </StyledStepper>

        <Fade in={!isAnimating} timeout={300}>
          <Box>
            {currentStep.content}
            
            {currentStep.demo && (
              <DemoBox className="demo-box" elevation={0}>
                {currentStep.demo}
              </DemoBox>
            )}
          </Box>
        </Fade>
      </DialogContent>

      <DialogActions sx={{ p: 3, pt: 0 }}>
        <Button
          onClick={activeStep === 0 ? onClose : handleBack}
          sx={{ color: 'rgba(255, 255, 255, 0.7)' }}
        >
          {activeStep === 0 ? 'Cancel' : 'Back'}
        </Button>
        <Box flex={1} />
        <StyledButton
          onClick={handleNext}
          endIcon={activeStep < steps.length - 1 ? <ArrowForwardIcon /> : <CheckCircleIcon />}
        >
          {activeStep === steps.length - 1 ? 'Start Trading' : 'Next'}
        </StyledButton>
      </DialogActions>
    </WizardDialog>
  );
};