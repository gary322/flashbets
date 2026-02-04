"use strict";
// Interactive Curve Editor for Continuous Distributions
// Allows users to visually edit probability distributions
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
exports.CurveEditor = void 0;
const react_1 = __importStar(require("react"));
const material_1 = require("@mui/material");
const styles_1 = require("@mui/material/styles");
const Restore_1 = __importDefault(require("@mui/icons-material/Restore"));
const Save_1 = __importDefault(require("@mui/icons-material/Save"));
const AutoGraph_1 = __importDefault(require("@mui/icons-material/AutoGraph"));
const styles_2 = require("@mui/material/styles");
// Styled components matching Blur aesthetic
const EditorContainer = (0, styles_1.styled)(material_1.Paper)(({ theme }) => ({
    backgroundColor: 'rgba(10, 10, 10, 0.9)',
    backdropFilter: 'blur(20px)',
    border: '1px solid rgba(255, 255, 255, 0.1)',
    borderRadius: '8px',
    padding: theme.spacing(3),
    position: 'relative',
    overflow: 'hidden',
    '&::before': {
        content: '""',
        position: 'absolute',
        top: 0,
        left: 0,
        right: 0,
        height: '1px',
        background: 'linear-gradient(90deg, transparent, rgba(130, 71, 229, 0.5), transparent)',
        animation: 'shimmer 2s infinite',
    },
}));
const Canvas = (0, styles_1.styled)('canvas')({
    width: '100%',
    height: '400px',
    cursor: 'crosshair',
    borderRadius: '4px',
    backgroundColor: 'rgba(0, 0, 0, 0.3)',
});
const ControlPanel = (0, styles_1.styled)(material_1.Box)(({ theme }) => ({
    marginTop: theme.spacing(2),
    display: 'flex',
    flexDirection: 'column',
    gap: theme.spacing(2),
}));
const StyledSlider = (0, styles_1.styled)(material_1.Slider)(({ theme }) => ({
    color: '#8247E5',
    '& .MuiSlider-thumb': {
        backgroundColor: '#8247E5',
        border: '2px solid #0A0A0A',
        '&:hover': {
            boxShadow: '0 0 20px rgba(130, 71, 229, 0.5)',
        },
    },
    '& .MuiSlider-track': {
        background: 'linear-gradient(90deg, #6B3AA0 0%, #8247E5 100%)',
    },
    '& .MuiSlider-rail': {
        backgroundColor: 'rgba(255, 255, 255, 0.1)',
    },
}));
const CurveEditor = ({ initialDistribution, onDistributionChange, numBuckets = 20, editable = true, }) => {
    const theme = (0, styles_2.useTheme)();
    const canvasRef = (0, react_1.useRef)(null);
    const [distribution, setDistribution] = (0, react_1.useState)(initialDistribution || Array(numBuckets).fill(1 / numBuckets));
    const [isDrawing, setIsDrawing] = (0, react_1.useState)(false);
    const [smoothness, setSmoothness] = (0, react_1.useState)(50);
    const [skew, setSkew] = (0, react_1.useState)(0);
    const [kurtosis, setKurtosis] = (0, react_1.useState)(3);
    // Draw the distribution curve
    const drawCurve = (0, react_1.useCallback)(() => {
        const canvas = canvasRef.current;
        if (!canvas)
            return;
        const ctx = canvas.getContext('2d');
        if (!ctx)
            return;
        // Set canvas size
        canvas.width = canvas.offsetWidth * 2;
        canvas.height = canvas.offsetHeight * 2;
        ctx.scale(2, 2);
        // Clear canvas
        ctx.clearRect(0, 0, canvas.width, canvas.height);
        // Draw grid
        ctx.strokeStyle = 'rgba(255, 255, 255, 0.05)';
        ctx.lineWidth = 1;
        // Vertical lines
        for (let i = 0; i <= 10; i++) {
            const x = (canvas.width / 2) * (i / 10);
            ctx.beginPath();
            ctx.moveTo(x, 0);
            ctx.lineTo(x, canvas.height / 2);
            ctx.stroke();
        }
        // Horizontal lines
        for (let i = 0; i <= 5; i++) {
            const y = (canvas.height / 2) * (i / 5);
            ctx.beginPath();
            ctx.moveTo(0, y);
            ctx.lineTo(canvas.width / 2, y);
            ctx.stroke();
        }
        // Draw distribution bars
        const barWidth = (canvas.width / 2) / numBuckets;
        const maxValue = Math.max(...distribution);
        // Gradient fill
        const gradient = ctx.createLinearGradient(0, canvas.height / 2, 0, 0);
        gradient.addColorStop(0, 'rgba(130, 71, 229, 0.1)');
        gradient.addColorStop(1, 'rgba(130, 71, 229, 0.6)');
        distribution.forEach((value, index) => {
            const height = (value / maxValue) * (canvas.height / 2) * 0.9;
            const x = index * barWidth;
            ctx.fillStyle = gradient;
            ctx.fillRect(x, canvas.height / 2 - height, barWidth - 2, height);
            // Add glow effect for higher values
            if (value > maxValue * 0.7) {
                ctx.shadowColor = '#8247E5';
                ctx.shadowBlur = 20;
                ctx.fillRect(x, canvas.height / 2 - height, barWidth - 2, height);
                ctx.shadowBlur = 0;
            }
        });
        // Draw smooth curve overlay
        ctx.strokeStyle = '#8247E5';
        ctx.lineWidth = 2;
        ctx.beginPath();
        distribution.forEach((value, index) => {
            const x = index * barWidth + barWidth / 2;
            const y = canvas.height / 2 - (value / maxValue) * (canvas.height / 2) * 0.9;
            if (index === 0) {
                ctx.moveTo(x, y);
            }
            else {
                // Smooth curve using quadratic bezier
                const prevX = (index - 1) * barWidth + barWidth / 2;
                const prevY = canvas.height / 2 - (distribution[index - 1] / maxValue) * (canvas.height / 2) * 0.9;
                const cpX = (prevX + x) / 2;
                const cpY = (prevY + y) / 2;
                ctx.quadraticCurveTo(prevX, prevY, cpX, cpY);
            }
        });
        ctx.stroke();
        // Draw axes
        ctx.strokeStyle = 'rgba(255, 255, 255, 0.3)';
        ctx.lineWidth = 2;
        ctx.beginPath();
        ctx.moveTo(0, canvas.height / 2);
        ctx.lineTo(canvas.width / 2, canvas.height / 2);
        ctx.moveTo(0, 0);
        ctx.lineTo(0, canvas.height / 2);
        ctx.stroke();
        // Labels
        ctx.fillStyle = 'rgba(255, 255, 255, 0.7)';
        ctx.font = '12px Inter';
        ctx.fillText('0%', 5, canvas.height / 2 - 5);
        ctx.fillText('100%', canvas.width / 2 - 30, canvas.height / 2 - 5);
        ctx.fillText('Probability', 5, 15);
    }, [distribution, numBuckets]);
    // Handle mouse drawing
    const handleMouseMove = (0, react_1.useCallback)((e) => {
        if (!isDrawing || !editable || !canvasRef.current)
            return;
        const rect = canvasRef.current.getBoundingClientRect();
        const x = e.clientX - rect.left;
        const y = e.clientY - rect.top;
        const bucketIndex = Math.floor((x / rect.width) * numBuckets);
        const value = Math.max(0, Math.min(1, 1 - (y / rect.height)));
        if (bucketIndex >= 0 && bucketIndex < numBuckets) {
            const newDistribution = [...distribution];
            // Apply smoothing to neighboring buckets
            const smoothRadius = Math.floor(smoothness / 20);
            for (let i = -smoothRadius; i <= smoothRadius; i++) {
                const idx = bucketIndex + i;
                if (idx >= 0 && idx < numBuckets) {
                    const weight = 1 - Math.abs(i) / (smoothRadius + 1);
                    newDistribution[idx] = newDistribution[idx] * (1 - weight) + value * weight;
                }
            }
            // Normalize
            const sum = newDistribution.reduce((a, b) => a + b, 0);
            const normalized = newDistribution.map(v => v / sum);
            setDistribution(normalized);
            onDistributionChange === null || onDistributionChange === void 0 ? void 0 : onDistributionChange(normalized);
        }
    }, [isDrawing, editable, distribution, numBuckets, smoothness, onDistributionChange]);
    // Apply distribution transformations
    const applyNormalDistribution = (0, react_1.useCallback)(() => {
        const mean = numBuckets / 2 + (skew * numBuckets / 10);
        const variance = Math.pow((numBuckets / 6), 2) / (kurtosis / 3);
        const newDistribution = Array(numBuckets).fill(0).map((_, i) => {
            const x = i - mean;
            return Math.exp(-(Math.pow(x, 2)) / (2 * variance)) / Math.sqrt(2 * Math.PI * variance);
        });
        // Normalize
        const sum = newDistribution.reduce((a, b) => a + b, 0);
        const normalized = newDistribution.map(v => v / sum);
        setDistribution(normalized);
        onDistributionChange === null || onDistributionChange === void 0 ? void 0 : onDistributionChange(normalized);
    }, [numBuckets, skew, kurtosis, onDistributionChange]);
    // Reset distribution
    const resetDistribution = (0, react_1.useCallback)(() => {
        const uniform = Array(numBuckets).fill(1 / numBuckets);
        setDistribution(uniform);
        onDistributionChange === null || onDistributionChange === void 0 ? void 0 : onDistributionChange(uniform);
        setSkew(0);
        setKurtosis(3);
    }, [numBuckets, onDistributionChange]);
    (0, react_1.useEffect)(() => {
        drawCurve();
    }, [drawCurve]);
    return (<EditorContainer elevation={0}>
      <material_1.Box display="flex" justifyContent="space-between" alignItems="center" mb={2}>
        <material_1.Typography variant="h6" sx={{ color: 'rgba(255, 255, 255, 0.9)' }}>
          Distribution Editor
        </material_1.Typography>
        <material_1.Box display="flex" gap={1}>
          <material_1.Tooltip title="Apply Normal Distribution">
            <material_1.IconButton onClick={applyNormalDistribution} size="small">
              <AutoGraph_1.default sx={{ color: '#8247E5' }}/>
            </material_1.IconButton>
          </material_1.Tooltip>
          <material_1.Tooltip title="Reset">
            <material_1.IconButton onClick={resetDistribution} size="small">
              <Restore_1.default sx={{ color: 'rgba(255, 255, 255, 0.7)' }}/>
            </material_1.IconButton>
          </material_1.Tooltip>
          <material_1.Tooltip title="Save">
            <material_1.IconButton size="small">
              <Save_1.default sx={{ color: 'rgba(255, 255, 255, 0.7)' }}/>
            </material_1.IconButton>
          </material_1.Tooltip>
        </material_1.Box>
      </material_1.Box>

      <Canvas ref={canvasRef} onMouseDown={() => editable && setIsDrawing(true)} onMouseUp={() => setIsDrawing(false)} onMouseLeave={() => setIsDrawing(false)} onMouseMove={handleMouseMove}/>

      {editable && (<ControlPanel>
          <material_1.Box>
            <material_1.Typography variant="caption" sx={{ color: 'rgba(255, 255, 255, 0.7)' }}>
              Smoothness
            </material_1.Typography>
            <StyledSlider value={smoothness} onChange={(_, value) => setSmoothness(value)} min={0} max={100} valueLabelDisplay="auto"/>
          </material_1.Box>
          
          <material_1.Box>
            <material_1.Typography variant="caption" sx={{ color: 'rgba(255, 255, 255, 0.7)' }}>
              Skew
            </material_1.Typography>
            <StyledSlider value={skew} onChange={(_, value) => setSkew(value)} min={-5} max={5} step={0.1} valueLabelDisplay="auto"/>
          </material_1.Box>
          
          <material_1.Box>
            <material_1.Typography variant="caption" sx={{ color: 'rgba(255, 255, 255, 0.7)' }}>
              Kurtosis
            </material_1.Typography>
            <StyledSlider value={kurtosis} onChange={(_, value) => setKurtosis(value)} min={1} max={10} step={0.1} valueLabelDisplay="auto"/>
          </material_1.Box>
        </ControlPanel>)}
    </EditorContainer>);
};
exports.CurveEditor = CurveEditor;
// Add shimmer animation
const globalStyles = `
  @keyframes shimmer {
    0% {
      transform: translateX(-100%);
    }
    100% {
      transform: translateX(100%);
    }
  }
`;
// Inject global styles
if (typeof window !== 'undefined') {
    const styleElement = document.createElement('style');
    styleElement.innerHTML = globalStyles;
    document.head.appendChild(styleElement);
}
