import React, { useMemo } from 'react';
import styled from '@emotion/styled';
import { motion, AnimatePresence } from 'framer-motion';

interface LeverageSliderProps {
  value: number;
  onChange: (value: number) => void;
  max: number;
  effectiveLeverage: number;
  showWarnings: boolean;
  coverage: number;
}

const SliderContainer = styled.div`
  position: relative;
  padding: 24px 0;
`;

const SliderTrack = styled.div<{ danger: boolean; extreme: boolean }>`
  height: 8px;
  background: ${props => 
    props.extreme ? 'linear-gradient(90deg, #1F2937 0%, #DC2626 100%)' :
    props.danger ? 'linear-gradient(90deg, #1F2937 0%, #F59E0B 100%)' :
    'linear-gradient(90deg, #1F2937 0%, #10B981 100%)'
  };
  border-radius: 4px;
  position: relative;
  cursor: pointer;
`;

const SliderThumb = styled(motion.div)<{ danger: boolean }>`
  width: 24px;
  height: 24px;
  background: ${props => props.danger ? '#DC2626' : '#FFFFFF'};
  border-radius: 50%;
  position: absolute;
  top: 50%;
  transform: translate(-50%, -50%);
  cursor: grab;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.4);
  
  &:active {
    cursor: grabbing;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.6);
  }
`;

const LeverageDisplay = styled.div`
  display: flex;
  justify-content: space-between;
  align-items: baseline;
  margin-bottom: 16px;
`;

const CurrentLeverage = styled.div<{ size: 'normal' | 'large' }>`
  font-size: ${props => props.size === 'large' ? '48px' : '32px'};
  font-weight: 900;
  font-family: ${props => props.theme.typography.fonts.mono};
  color: ${props => props.theme.colors.text.primary};
  
  span {
    font-size: 24px;
    color: ${props => props.theme.colors.text.secondary};
  }
`;

const EffectiveLeverage = styled(motion.div)`
  font-size: 18px;
  color: ${props => props.theme.colors.status.warning};
  font-family: ${props => props.theme.typography.fonts.mono};
`;

const WarningMessage = styled(motion.div)<{ severity: 'warning' | 'danger' }>`
  margin-top: 16px;
  padding: 12px 16px;
  background: ${props => 
    props.severity === 'danger' ? 
    'rgba(220, 38, 38, 0.1)' : 
    'rgba(245, 158, 11, 0.1)'
  };
  border: 1px solid ${props =>
    props.severity === 'danger' ?
    'rgba(220, 38, 38, 0.3)' :
    'rgba(245, 158, 11, 0.3)'
  };
  border-radius: 8px;
  font-size: 13px;
  color: ${props =>
    props.severity === 'danger' ?
    '#EF4444' :
    '#F59E0B'
  };
`;

const PresetButtons = styled.div`
  display: flex;
  gap: 8px;
  margin-top: 16px;
`;

const PresetButton = styled.button<{ active: boolean }>`
  padding: 8px 16px;
  background: ${props => props.active ? 
    props.theme.colors.accent.primary : 
    'transparent'
  };
  color: ${props => props.active ?
    props.theme.colors.background.primary :
    props.theme.colors.text.secondary
  };
  border: 1px solid ${props => props.active ?
    props.theme.colors.accent.primary :
    'rgba(255, 255, 255, 0.1)'
  };
  border-radius: 6px;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  transition: all 200ms ease;
  
  &:hover {
    border-color: ${props => props.theme.colors.accent.primary};
    color: ${props => props.theme.colors.text.primary};
  }
`;

export const LeverageSlider: React.FC<LeverageSliderProps> = ({
  value,
  onChange,
  max,
  effectiveLeverage,
  showWarnings,
  coverage
}) => {
  const presets = [1, 10, 25, 50, 100];

  const warningLevel = useMemo(() => {
    if (effectiveLeverage >= 300) return 'extreme';
    if (effectiveLeverage >= 100) return 'danger';
    if (effectiveLeverage >= 50) return 'warning';
    return 'safe';
  }, [effectiveLeverage]);

  const liquidationBuffer = useMemo(() => {
    return (1 / effectiveLeverage) * 100;
  }, [effectiveLeverage]);

  const handleSliderChange = (e: React.MouseEvent<HTMLDivElement>) => {
    const rect = e.currentTarget.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const percentage = Math.max(0, Math.min(1, x / rect.width));
    const newValue = Math.round(percentage * max);
    onChange(newValue);
  };

  return (
    <SliderContainer>
      <LeverageDisplay>
        <div>
          <div style={{ fontSize: '13px', color: '#6B7280', marginBottom: '4px' }}>
            Base Leverage
          </div>
          <CurrentLeverage size="normal">
            {value}<span>x</span>
          </CurrentLeverage>
        </div>

        {effectiveLeverage !== value && (
          <div style={{ textAlign: 'right' }}>
            <div style={{ fontSize: '13px', color: '#6B7280', marginBottom: '4px' }}>
              Effective (with chain)
            </div>
            <EffectiveLeverage
              initial={{ opacity: 0, y: -10 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ duration: 0.3 }}
            >
              {effectiveLeverage.toFixed(1)}x
            </EffectiveLeverage>
          </div>
        )}
      </LeverageDisplay>

      <SliderTrack
        danger={warningLevel === 'danger' || warningLevel === 'extreme'}
        extreme={warningLevel === 'extreme'}
        onClick={handleSliderChange}
      >
        <SliderThumb
          danger={warningLevel !== 'safe'}
          style={{ left: `${(value / max) * 100}%` }}
          drag="x"
          dragConstraints={{ left: 0, right: 0 }}
          dragElastic={0}
          dragMomentum={false}
          onDrag={(e, info) => {
            const target = e.target as HTMLElement;
            const rect = target.parentElement?.getBoundingClientRect();
            if (!rect) return;
            const percentage = Math.max(0, Math.min(1,
              (info.point.x - rect.left) / rect.width
            ));
            onChange(Math.round(percentage * max));
          }}
          whileHover={{ scale: 1.1 }}
          whileDrag={{ scale: 1.2 }}
        />
      </SliderTrack>

      <PresetButtons>
        {presets.map(preset => (
          <PresetButton
            key={preset}
            active={value === preset}
            onClick={() => onChange(preset)}
          >
            {preset}x
          </PresetButton>
        ))}
        <PresetButton
          active={value === max}
          onClick={() => onChange(max)}
          style={{ marginLeft: 'auto' }}
        >
          MAX
        </PresetButton>
      </PresetButtons>

      <AnimatePresence>
        {showWarnings && warningLevel !== 'safe' && (
          <WarningMessage
            severity={warningLevel === 'extreme' ? 'danger' : 'warning'}
            initial={{ opacity: 0, height: 0 }}
            animate={{ opacity: 1, height: 'auto' }}
            exit={{ opacity: 0, height: 0 }}
          >
            {warningLevel === 'extreme' ? (
              <>
                ⚠️ EXTREME LEVERAGE: {effectiveLeverage.toFixed(1)}x effective
                <br />
                You will be liquidated on a {liquidationBuffer.toFixed(2)}% adverse move
              </>
            ) : warningLevel === 'danger' ? (
              <>
                ⚠️ HIGH LEVERAGE: {effectiveLeverage.toFixed(1)}x effective
                <br />
                Liquidation on {liquidationBuffer.toFixed(2)}% move. Use with caution.
              </>
            ) : (
              <>
                Moderate leverage. Liquidation buffer: {liquidationBuffer.toFixed(2)}%
              </>
            )}
          </WarningMessage>
        )}
      </AnimatePresence>
    </SliderContainer>
  );
};