import React from 'react';
import styled from '@emotion/styled';

interface QuantumToggleProps {
  active: boolean;
  onChange: (active: boolean) => void;
  disabled?: boolean;
}

const ToggleContainer = styled.div<{ active: boolean; disabled?: boolean }>`
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 16px;
  background: ${props => props.active 
    ? 'rgba(255, 165, 0, 0.1)' 
    : 'rgba(255, 165, 0, 0.05)'
  };
  border: 1px solid ${props => props.active 
    ? props.theme?.colors?.accent?.secondary || '#ffa500'
    : 'rgba(255, 165, 0, 0.2)'
  };
  border-radius: 12px;
  cursor: ${props => props.disabled ? 'not-allowed' : 'pointer'};
  transition: all ${props => props.theme?.animation?.durations?.normal || '300ms'} ${props => props.theme?.animation?.easings?.default || 'ease'};
  opacity: ${props => props.disabled ? 0.5 : 1};
  
  &:hover:not(:disabled) {
    background: rgba(255, 165, 0, 0.15);
    border-color: ${props => props.theme?.colors?.accent?.secondary || '#ffa500'};
  }
`;

const QuantumIcon = styled.div<{ active: boolean }>`
  width: 32px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 20px;
  animation: ${props => props.active ? 'quantumPulse 2s ease-in-out infinite' : 'none'};
  
  @keyframes quantumPulse {
    0%, 100% {
      transform: scale(1) rotate(0deg);
      opacity: 1;
    }
    50% {
      transform: scale(1.1) rotate(180deg);
      opacity: 0.8;
    }
  }
`;

const QuantumInfo = styled.div`
  flex: 1;
`;

const QuantumLabel = styled.div`
  font-size: 14px;
  font-weight: 500;
  color: ${props => props.theme?.colors?.text?.primary || '#fff'};
`;

const QuantumDescription = styled.div`
  font-size: 12px;
  color: ${props => props.theme?.colors?.text?.secondary || '#aaa'};
  margin-top: 2px;
`;

const ToggleSwitch = styled.div<{ active: boolean }>`
  width: ${props => props.theme?.components?.quantumToggle?.width || '50px'};
  height: ${props => props.theme?.components?.quantumToggle?.height || '24px'};
  background: ${props => props.active 
    ? props.theme?.colors?.accent?.secondary || '#ffa500'
    : 'rgba(255, 255, 255, 0.2)'
  };
  border-radius: ${props => props.theme?.components?.quantumToggle?.borderRadius || '12px'};
  position: relative;
  transition: all ${props => props.theme?.animation?.durations?.normal || '300ms'} ${props => props.theme?.animation?.easings?.default || 'ease'};
  
  &::after {
    content: '';
    position: absolute;
    top: 2px;
    left: ${props => props.active ? '26px' : '2px'};
    width: ${props => props.theme?.components?.quantumToggle?.thumbSize || '20px'};
    height: ${props => props.theme?.components?.quantumToggle?.thumbSize || '20px'};
    background: ${props => props.theme?.colors?.text?.primary || '#fff'};
    border-radius: 50%;
    transition: all ${props => props.theme?.animation?.durations?.normal || '300ms'} ${props => props.theme?.animation?.easings?.smooth || 'ease'};
    box-shadow: 0 2px 4px rgba(0, 0, 0, 0.2);
  }
`;

const QuantumStats = styled.div`
  display: flex;
  gap: 16px;
  margin-top: 12px;
  padding-top: 12px;
  border-top: 1px solid rgba(255, 255, 255, 0.1);
`;

const StatItem = styled.div`
  display: flex;
  flex-direction: column;
  gap: 2px;
`;

const StatLabel = styled.span`
  font-size: 11px;
  color: ${props => props.theme?.colors?.text?.tertiary || '#666'};
  text-transform: uppercase;
  letter-spacing: 0.5px;
`;

const StatValue = styled.span<{ highlight?: boolean }>`
  font-size: 14px;
  font-weight: 600;
  color: ${props => props.highlight 
    ? props.theme?.colors?.quantum?.coherent || '#00ff00'
    : props.theme?.colors?.text?.primary || '#fff'
  };
`;

const QuantumIndicator = styled.div`
  display: flex;
  align-items: center;
  gap: 4px;
  margin-top: 8px;
`;

const CoherenceBar = styled.div`
  flex: 1;
  height: 4px;
  background: rgba(255, 255, 255, 0.1);
  border-radius: 2px;
  overflow: hidden;
`;

const CoherenceFill = styled.div<{ coherence: number }>`
  width: ${props => props.coherence}%;
  height: 100%;
  background: ${props => {
    if (props.coherence > 70) return props.theme?.colors?.quantum?.coherent || '#00ff00';
    if (props.coherence > 30) return props.theme?.colors?.status?.warning || '#ffaa00';
    return props.theme?.colors?.quantum?.collapsed || '#ff0000';
  }};
  transition: width ${props => props.theme?.animation?.durations?.normal || '300ms'} ${props => props.theme?.animation?.easings?.default || 'ease'};
`;

const CoherenceLabel = styled.span`
  font-size: 11px;
  color: ${props => props.theme?.colors?.text?.secondary || '#aaa'};
`;

export default function QuantumToggle({ 
  active, 
  onChange, 
  disabled 
}: QuantumToggleProps) {
  const [coherence, setCoherence] = React.useState(100);

  React.useEffect(() => {
    if (active) {
      const interval = setInterval(() => {
        setCoherence(prev => Math.max(0, prev - 1));
      }, 1000);
      return () => clearInterval(interval);
    } else {
      setCoherence(100);
    }
  }, [active]);

  const handleClick = () => {
    if (!disabled) {
      onChange(!active);
    }
  };

  return (
    <ToggleContainer 
      active={active} 
      disabled={disabled}
      onClick={handleClick}
    >
      <QuantumIcon active={active}>⚛️</QuantumIcon>
      
      <QuantumInfo>
        <QuantumLabel>Quantum Mode</QuantumLabel>
        <QuantumDescription>
          Split position across all outcomes
        </QuantumDescription>
        
        {active && (
          <>
            <QuantumStats>
              <StatItem>
                <StatLabel>States</StatLabel>
                <StatValue>Superposition</StatValue>
              </StatItem>
              <StatItem>
                <StatLabel>Enhancement</StatLabel>
                <StatValue highlight>+15%</StatValue>
              </StatItem>
            </QuantumStats>
            
            <QuantumIndicator>
              <CoherenceBar>
                <CoherenceFill coherence={coherence} />
              </CoherenceBar>
              <CoherenceLabel>{coherence}%</CoherenceLabel>
            </QuantumIndicator>
          </>
        )}
      </QuantumInfo>
      
      <ToggleSwitch active={active} />
    </ToggleContainer>
  );
}