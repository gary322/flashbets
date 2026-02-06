"use strict";
// Trading Wizard for Beginners
// Step-by-step onboarding with interactive tutorials
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.TradingWizard = void 0;
const react_1 = __importStar(require("react"));
const material_1 = require("@mui/material");
const styles_1 = require("@mui/material/styles");
const ArrowForward_1 = __importDefault(require("@mui/icons-material/ArrowForward"));
const CheckCircle_1 = __importDefault(require("@mui/icons-material/CheckCircle"));
const Info_1 = __importDefault(require("@mui/icons-material/Info"));
const RocketLaunch_1 = __importDefault(require("@mui/icons-material/RocketLaunch"));
const TrendingUp_1 = __importDefault(require("@mui/icons-material/TrendingUp"));
const AccountBalanceWallet_1 = __importDefault(require("@mui/icons-material/AccountBalanceWallet"));
// Styled components with Blur aesthetic
const WizardDialog = (0, styles_1.styled)(material_1.Dialog)(({ theme }) => ({
    '& .MuiDialog-paper': {
        backgroundColor: '#0A0A0A',
        backgroundImage: 'radial-gradient(circle at 20% 50%, rgba(130, 71, 229, 0.1) 0%, transparent 50%)',
        border: '1px solid rgba(255, 255, 255, 0.1)',
        borderRadius: '16px',
        maxWidth: '600px',
        overflow: 'visible',
    },
}));
const StyledStepper = (0, styles_1.styled)(material_1.Stepper)(({ theme }) => ({
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
const DemoBox = (0, styles_1.styled)(material_1.Paper)(({ theme }) => ({
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
const StyledButton = (0, styles_1.styled)(material_1.Button)(({ theme }) => ({
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
const InfoChip = (0, styles_1.styled)(material_1.Chip)(({ theme }) => ({
    backgroundColor: 'rgba(130, 71, 229, 0.2)',
    color: '#8247E5',
    border: '1px solid rgba(130, 71, 229, 0.3)',
    fontFamily: 'JetBrains Mono',
    fontSize: '12px',
}));
const TradingWizard = ({ open, onClose, onComplete, }) => {
    const [activeStep, setActiveStep] = (0, react_1.useState)(0);
    const [settings, setSettings] = (0, react_1.useState)({
        experience: 'beginner',
        initialDeposit: 100,
        riskTolerance: 50,
        preferredLeverage: 10,
        enableAutoChain: false,
    });
    const [demoValue, setDemoValue] = (0, react_1.useState)(0);
    const [isAnimating, setIsAnimating] = (0, react_1.useState)(false);
    // Mouse tracking for interactive effects
    (0, react_1.useEffect)(() => {
        const handleMouseMove = (e) => {
            const elements = document.querySelectorAll('.demo-box');
            elements.forEach((el) => {
                const rect = el.getBoundingClientRect();
                const x = ((e.clientX - rect.left) / rect.width) * 100;
                const y = ((e.clientY - rect.top) / rect.height) * 100;
                el.style.setProperty('--mouse-x', `${x}%`);
                el.style.setProperty('--mouse-y', `${y}%`);
            });
        };
        if (open) {
            document.addEventListener('mousemove', handleMouseMove);
            return () => document.removeEventListener('mousemove', handleMouseMove);
        }
    }, [open]);
    const steps = [
        {
            label: 'Welcome',
            description: 'Start your prediction market journey',
            content: (<material_1.Box>
          <material_1.Typography variant="h4" sx={{ color: '#FFFFFF', mb: 2, fontWeight: 700 }}>
            Welcome to the Future of Prediction Markets
	          </material_1.Typography>
	          <material_1.Typography sx={{ color: 'rgba(255, 255, 255, 0.7)', mb: 3 }}>
	            Let&apos;s set up your trading experience in just a few steps. This wizard will help you
	            understand our platform and configure it to match your trading style.
	          </material_1.Typography>
          <material_1.Box display="flex" gap={2} flexWrap="wrap">
            <InfoChip icon={<RocketLaunch_1.default />} label="500x+ Leverage"/>
            <InfoChip icon={<TrendingUp_1.default />} label="30%+ Yields"/>
            <InfoChip icon={<AccountBalanceWallet_1.default />} label="No KYC"/>
          </material_1.Box>
        </material_1.Box>),
            demo: (<material_1.Box textAlign="center" py={4}>
          <RocketLaunch_1.default sx={{ fontSize: 80, color: '#8247E5', mb: 2 }}/>
          <material_1.Typography variant="h6" sx={{ color: 'rgba(255, 255, 255, 0.9)' }}>
            Ready to maximize your predictions?
          </material_1.Typography>
        </material_1.Box>),
        },
        {
            label: 'Experience Level',
            description: 'Tell us about your trading experience',
            content: (<material_1.Box>
	          <material_1.Typography variant="h6" sx={{ color: '#FFFFFF', mb: 3 }}>
	            What&apos;s your experience with prediction markets?
	          </material_1.Typography>
          <material_1.Box display="flex" flexDirection="column" gap={2}>
            {['beginner', 'intermediate', 'advanced'].map((level) => (<material_1.Paper key={level} sx={{
                        p: 2,
                        cursor: 'pointer',
                        backgroundColor: settings.experience === level ? 'rgba(130, 71, 229, 0.2)' : 'rgba(255, 255, 255, 0.02)',
                        border: settings.experience === level ? '2px solid #8247E5' : '1px solid rgba(255, 255, 255, 0.1)',
                        transition: 'all 0.3s ease',
                        '&:hover': {
                            backgroundColor: 'rgba(130, 71, 229, 0.1)',
                            borderColor: 'rgba(130, 71, 229, 0.5)',
                        },
                    }} onClick={() => setSettings(Object.assign(Object.assign({}, settings), { experience: level }))}>
                <material_1.Typography variant="subtitle1" sx={{ color: '#FFFFFF', textTransform: 'capitalize', mb: 1 }}>
                  {level}
                </material_1.Typography>
                <material_1.Typography variant="body2" sx={{ color: 'rgba(255, 255, 255, 0.6)' }}>
                  {level === 'beginner' && "I'm new to prediction markets and want to learn"}
                  {level === 'intermediate' && "I've traded before but want to explore advanced features"}
                  {level === 'advanced' && "I'm experienced and want full control"}
                </material_1.Typography>
              </material_1.Paper>))}
          </material_1.Box>
        </material_1.Box>),
            validation: () => true,
        },
        {
            label: 'Initial Setup',
            description: 'Configure your trading parameters',
            content: (<material_1.Box>
	          <material_1.Typography variant="h6" sx={{ color: '#FFFFFF', mb: 3 }}>
	            Let&apos;s configure your trading settings
	          </material_1.Typography>
          
          <material_1.Box mb={3}>
            <material_1.Typography sx={{ color: 'rgba(255, 255, 255, 0.7)', mb: 1 }}>
              Initial Deposit (USDC)
            </material_1.Typography>
            <material_1.TextField fullWidth type="number" value={settings.initialDeposit} onChange={(e) => setSettings(Object.assign(Object.assign({}, settings), { initialDeposit: Number(e.target.value) }))} sx={{
                    '& .MuiInputBase-root': {
                        backgroundColor: 'rgba(255, 255, 255, 0.05)',
                        color: '#FFFFFF',
                    },
                    '& .MuiOutlinedInput-notchedOutline': {
                        borderColor: 'rgba(255, 255, 255, 0.2)',
                    },
                }} InputProps={{
                    startAdornment: <material_1.Typography sx={{ color: 'rgba(255, 255, 255, 0.5)', mr: 1 }}>$</material_1.Typography>,
                }}/>
          </material_1.Box>

          <material_1.Box mb={3}>
            <material_1.Typography sx={{ color: 'rgba(255, 255, 255, 0.7)', mb: 1 }}>
              Risk Tolerance
            </material_1.Typography>
            <material_1.Slider value={settings.riskTolerance} onChange={(_, value) => setSettings(Object.assign(Object.assign({}, settings), { riskTolerance: value }))} marks={[
                    { value: 0, label: 'Conservative' },
                    { value: 50, label: 'Moderate' },
                    { value: 100, label: 'Aggressive' },
                ]} sx={{
                    color: '#8247E5',
                    '& .MuiSlider-mark': {
                        backgroundColor: 'rgba(255, 255, 255, 0.2)',
                    },
                    '& .MuiSlider-markLabel': {
                        color: 'rgba(255, 255, 255, 0.5)',
                        fontSize: '12px',
                    },
                }}/>
          </material_1.Box>
        </material_1.Box>),
            demo: (<material_1.Box>
          <material_1.Typography variant="body2" sx={{ color: 'rgba(255, 255, 255, 0.7)', mb: 2 }}>
            With ${settings.initialDeposit} and {settings.preferredLeverage}x leverage:
          </material_1.Typography>
          <material_1.Typography variant="h4" sx={{ color: '#8247E5', mb: 1 }}>
            ${settings.initialDeposit * settings.preferredLeverage}
          </material_1.Typography>
          <material_1.Typography variant="body2" sx={{ color: 'rgba(255, 255, 255, 0.5)' }}>
            Total trading power
          </material_1.Typography>
        </material_1.Box>),
            validation: () => settings.initialDeposit >= 10,
        },
        {
            label: 'Leverage & Chaining',
            description: 'Unlock the power of auto-chaining',
            content: (<material_1.Box>
          <material_1.Typography variant="h6" sx={{ color: '#FFFFFF', mb: 3 }}>
            Leverage and Auto-Chaining
          </material_1.Typography>
          
          <material_1.Box mb={3}>
            <material_1.Box display="flex" alignItems="center" mb={1}>
              <material_1.Typography sx={{ color: 'rgba(255, 255, 255, 0.7)' }}>
                Preferred Leverage
              </material_1.Typography>
              <material_1.Tooltip title="Higher leverage means higher potential returns but also higher risk">
                <material_1.IconButton size="small" sx={{ ml: 1 }}>
                  <Info_1.default sx={{ fontSize: 16, color: 'rgba(255, 255, 255, 0.5)' }}/>
                </material_1.IconButton>
              </material_1.Tooltip>
            </material_1.Box>
            <material_1.Slider value={settings.preferredLeverage} onChange={(_, value) => setSettings(Object.assign(Object.assign({}, settings), { preferredLeverage: value }))} min={1} max={100} marks={[
                    { value: 10, label: '10x' },
                    { value: 50, label: '50x' },
                    { value: 100, label: '100x' },
                ]} sx={{
                    color: '#8247E5',
                    '& .MuiSlider-valueLabel': {
                        backgroundColor: '#8247E5',
                    },
                }} valueLabelDisplay="on" valueLabelFormat={(value) => `${value}x`}/>
          </material_1.Box>

          <material_1.Paper sx={{ p: 2, backgroundColor: 'rgba(130, 71, 229, 0.1)', border: '1px solid rgba(130, 71, 229, 0.3)' }}>
            <material_1.Box display="flex" alignItems="center" justifyContent="space-between" mb={1}>
              <material_1.Typography sx={{ color: '#FFFFFF' }}>
                Enable Auto-Chaining
              </material_1.Typography>
              <material_1.Button variant={settings.enableAutoChain ? "contained" : "outlined"} size="small" onClick={() => setSettings(Object.assign(Object.assign({}, settings), { enableAutoChain: !settings.enableAutoChain }))} sx={{
                    borderColor: '#8247E5',
                    color: settings.enableAutoChain ? '#FFFFFF' : '#8247E5',
                    backgroundColor: settings.enableAutoChain ? '#8247E5' : 'transparent',
                }}>
                {settings.enableAutoChain ? 'Enabled' : 'Disabled'}
              </material_1.Button>
            </material_1.Box>
            <material_1.Typography variant="body2" sx={{ color: 'rgba(255, 255, 255, 0.6)' }}>
              Auto-chaining can boost your effective leverage up to 500x+ through
              automated borrow → liquidity → stake sequences
            </material_1.Typography>
          </material_1.Paper>
        </material_1.Box>),
            demo: (<material_1.Box>
          <material_1.Typography variant="body2" sx={{ color: 'rgba(255, 255, 255, 0.7)', mb: 2 }}>
            Effective Leverage Calculation:
          </material_1.Typography>
          <material_1.Box display="flex" alignItems="center" gap={2} mb={2}>
            <InfoChip label={`Base: ${settings.preferredLeverage}x`}/>
            {settings.enableAutoChain && (<>
                <ArrowForward_1.default sx={{ color: 'rgba(255, 255, 255, 0.5)' }}/>
                <InfoChip label="Chain: 3.6x"/>
                <ArrowForward_1.default sx={{ color: 'rgba(255, 255, 255, 0.5)' }}/>
                <InfoChip label={`Total: ${Math.min(500, settings.preferredLeverage * 3.6).toFixed(0)}x`} sx={{ backgroundColor: 'rgba(130, 71, 229, 0.4)' }}/>
              </>)}
          </material_1.Box>
          {settings.enableAutoChain && (<material_1.Typography variant="caption" sx={{ color: 'rgba(255, 255, 255, 0.5)' }}>
              Chain multiplier: Borrow (1.5x) × Liquidity (1.2x) × Stake (1.2x) = 3.6x
            </material_1.Typography>)}
        </material_1.Box>),
            validation: () => true,
        },
        {
            label: 'Complete',
            description: 'Ready to start trading!',
            content: (<material_1.Box textAlign="center">
          <CheckCircle_1.default sx={{ fontSize: 64, color: '#8247E5', mb: 2 }}/>
	          <material_1.Typography variant="h5" sx={{ color: '#FFFFFF', mb: 2 }}>
	            You&apos;re All Set!
	          </material_1.Typography>
	          <material_1.Typography sx={{ color: 'rgba(255, 255, 255, 0.7)', mb: 3 }}>
	            Your trading environment is configured and ready. Here&apos;s a summary of your settings:
	          </material_1.Typography>
          <material_1.Box display="flex" flexDirection="column" gap={1} alignItems="center">
            <InfoChip label={`Experience: ${settings.experience}`}/>
            <InfoChip label={`Deposit: $${settings.initialDeposit}`}/>
            <InfoChip label={`Leverage: ${settings.preferredLeverage}x`}/>
            <InfoChip label={`Auto-Chain: ${settings.enableAutoChain ? 'Enabled' : 'Disabled'}`}/>
          </material_1.Box>
        </material_1.Box>),
        },
    ];
    const handleNext = () => {
        const currentValidation = steps[activeStep].validation;
        if (!currentValidation || currentValidation()) {
            if (activeStep === steps.length - 1) {
                onComplete(settings);
            }
            else {
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
    return (<WizardDialog open={open} onClose={onClose} maxWidth="md" fullWidth>
      <material_1.DialogContent sx={{ p: 4 }}>
        <StyledStepper activeStep={activeStep} orientation="horizontal" sx={{ mb: 4 }}>
          {steps.map((step, index) => (<material_1.Step key={step.label}>
              <material_1.StepLabel>{step.label}</material_1.StepLabel>
            </material_1.Step>))}
        </StyledStepper>

        <material_1.Fade in={!isAnimating} timeout={300}>
          <material_1.Box>
            {currentStep.content}
            
            {currentStep.demo && (<DemoBox className="demo-box" elevation={0}>
                {currentStep.demo}
              </DemoBox>)}
          </material_1.Box>
        </material_1.Fade>
      </material_1.DialogContent>

      <material_1.DialogActions sx={{ p: 3, pt: 0 }}>
        <material_1.Button onClick={activeStep === 0 ? onClose : handleBack} sx={{ color: 'rgba(255, 255, 255, 0.7)' }}>
          {activeStep === 0 ? 'Cancel' : 'Back'}
        </material_1.Button>
        <material_1.Box flex={1}/>
        <StyledButton onClick={handleNext} endIcon={activeStep < steps.length - 1 ? <ArrowForward_1.default /> : <CheckCircle_1.default />}>
          {activeStep === steps.length - 1 ? 'Start Trading' : 'Next'}
        </StyledButton>
      </material_1.DialogActions>
    </WizardDialog>);
};
exports.TradingWizard = TradingWizard;
