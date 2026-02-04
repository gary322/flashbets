// Interactive Curve Editor for Continuous Distributions
// Allows users to visually edit probability distributions

import React, { useState, useRef, useEffect, useCallback } from 'react';
import { Box, Typography, Slider, IconButton, Tooltip, Paper } from '@mui/material';
import { styled } from '@mui/material/styles';
import RestoreIcon from '@mui/icons-material/Restore';
import SaveIcon from '@mui/icons-material/Save';
import AutoGraphIcon from '@mui/icons-material/AutoGraph';
import { useTheme } from '@mui/material/styles';

// Styled components matching Blur aesthetic
const EditorContainer = styled(Paper)(({ theme }) => ({
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

const Canvas = styled('canvas')({
  width: '100%',
  height: '400px',
  cursor: 'crosshair',
  borderRadius: '4px',
  backgroundColor: 'rgba(0, 0, 0, 0.3)',
});

const ControlPanel = styled(Box)(({ theme }) => ({
  marginTop: theme.spacing(2),
  display: 'flex',
  flexDirection: 'column',
  gap: theme.spacing(2),
}));

const StyledSlider = styled(Slider)(({ theme }) => ({
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

interface Point {
  x: number;
  y: number;
}

interface CurveEditorProps {
  initialDistribution?: number[];
  onDistributionChange?: (distribution: number[]) => void;
  numBuckets?: number;
  editable?: boolean;
}

export const CurveEditor: React.FC<CurveEditorProps> = ({
  initialDistribution,
  onDistributionChange,
  numBuckets = 20,
  editable = true,
}) => {
  const theme = useTheme();
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [distribution, setDistribution] = useState<number[]>(
    initialDistribution || Array(numBuckets).fill(1 / numBuckets)
  );
  const [isDrawing, setIsDrawing] = useState(false);
  const [smoothness, setSmoothness] = useState(50);
  const [skew, setSkew] = useState(0);
  const [kurtosis, setKurtosis] = useState(3);

  // Draw the distribution curve
  const drawCurve = useCallback(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const ctx = canvas.getContext('2d');
    if (!ctx) return;

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
      } else {
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
  const handleMouseMove = useCallback((e: React.MouseEvent<HTMLCanvasElement>) => {
    if (!isDrawing || !editable || !canvasRef.current) return;

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
      onDistributionChange?.(normalized);
    }
  }, [isDrawing, editable, distribution, numBuckets, smoothness, onDistributionChange]);

  // Apply distribution transformations
  const applyNormalDistribution = useCallback(() => {
    const mean = numBuckets / 2 + (skew * numBuckets / 10);
    const variance = (numBuckets / 6) ** 2 / (kurtosis / 3);
    
    const newDistribution = Array(numBuckets).fill(0).map((_, i) => {
      const x = i - mean;
      return Math.exp(-(x ** 2) / (2 * variance)) / Math.sqrt(2 * Math.PI * variance);
    });
    
    // Normalize
    const sum = newDistribution.reduce((a, b) => a + b, 0);
    const normalized = newDistribution.map(v => v / sum);
    
    setDistribution(normalized);
    onDistributionChange?.(normalized);
  }, [numBuckets, skew, kurtosis, onDistributionChange]);

  // Reset distribution
  const resetDistribution = useCallback(() => {
    const uniform = Array(numBuckets).fill(1 / numBuckets);
    setDistribution(uniform);
    onDistributionChange?.(uniform);
    setSkew(0);
    setKurtosis(3);
  }, [numBuckets, onDistributionChange]);

  useEffect(() => {
    drawCurve();
  }, [drawCurve]);

  return (
    <EditorContainer elevation={0}>
      <Box display="flex" justifyContent="space-between" alignItems="center" mb={2}>
        <Typography variant="h6" sx={{ color: 'rgba(255, 255, 255, 0.9)' }}>
          Distribution Editor
        </Typography>
        <Box display="flex" gap={1}>
          <Tooltip title="Apply Normal Distribution">
            <IconButton onClick={applyNormalDistribution} size="small">
              <AutoGraphIcon sx={{ color: '#8247E5' }} />
            </IconButton>
          </Tooltip>
          <Tooltip title="Reset">
            <IconButton onClick={resetDistribution} size="small">
              <RestoreIcon sx={{ color: 'rgba(255, 255, 255, 0.7)' }} />
            </IconButton>
          </Tooltip>
          <Tooltip title="Save">
            <IconButton size="small">
              <SaveIcon sx={{ color: 'rgba(255, 255, 255, 0.7)' }} />
            </IconButton>
          </Tooltip>
        </Box>
      </Box>

      <Canvas
        ref={canvasRef}
        onMouseDown={() => editable && setIsDrawing(true)}
        onMouseUp={() => setIsDrawing(false)}
        onMouseLeave={() => setIsDrawing(false)}
        onMouseMove={handleMouseMove}
      />

      {editable && (
        <ControlPanel>
          <Box>
            <Typography variant="caption" sx={{ color: 'rgba(255, 255, 255, 0.7)' }}>
              Smoothness
            </Typography>
            <StyledSlider
              value={smoothness}
              onChange={(_, value) => setSmoothness(value as number)}
              min={0}
              max={100}
              valueLabelDisplay="auto"
            />
          </Box>
          
          <Box>
            <Typography variant="caption" sx={{ color: 'rgba(255, 255, 255, 0.7)' }}>
              Skew
            </Typography>
            <StyledSlider
              value={skew}
              onChange={(_, value) => setSkew(value as number)}
              min={-5}
              max={5}
              step={0.1}
              valueLabelDisplay="auto"
            />
          </Box>
          
          <Box>
            <Typography variant="caption" sx={{ color: 'rgba(255, 255, 255, 0.7)' }}>
              Kurtosis
            </Typography>
            <StyledSlider
              value={kurtosis}
              onChange={(_, value) => setKurtosis(value as number)}
              min={1}
              max={10}
              step={0.1}
              valueLabelDisplay="auto"
            />
          </Box>
        </ControlPanel>
      )}
    </EditorContainer>
  );
};

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