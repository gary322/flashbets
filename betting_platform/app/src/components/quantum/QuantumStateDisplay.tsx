import React from 'react';
import styled from '@emotion/styled';

export interface QuantumState {
  outcome: string;
  amplitude: number;
  phase: number;
  probability: number;
  allocation: number;
}

interface QuantumStateDisplayProps {
  states: QuantumState[];
  totalAmount: number;
  isActive: boolean;
}

const Container = styled.div`
  margin-bottom: 24px;
`;

const Title = styled.div`
  font-size: 12px;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  color: ${props => props.theme.colors.text.tertiary};
  margin-bottom: 12px;
`;

const StateGrid = styled.div`
  display: grid;
  gap: 12px;
`;

const StateItem = styled.div<{ isCollapsed?: boolean }>`
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 12px;
  background: ${props => props.isCollapsed 
    ? 'rgba(255, 59, 48, 0.1)'
    : 'rgba(255, 165, 0, 0.05)'
  };
  border: 1px solid ${props => props.isCollapsed
    ? 'rgba(255, 59, 48, 0.3)'
    : 'rgba(255, 165, 0, 0.2)'
  };
  border-radius: 8px;
  transition: all ${props => props.theme.animation.durations.normal} ${props => props.theme.animation.easings.default};
`;

const OutcomeInfo = styled.div`
  display: flex;
  align-items: center;
  gap: 12px;
`;

const WaveFunction = styled.div`
  width: 40px;
  height: 40px;
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
`;

const WaveCircle = styled.div<{ amplitude: number; phase: number }>`
  position: absolute;
  width: ${props => props.amplitude * 40}px;
  height: ${props => props.amplitude * 40}px;
  border: 2px solid ${props => props.theme.colors.quantum.superposition};
  border-radius: 50%;
  transform: rotate(${props => props.phase}deg);
  opacity: ${props => props.amplitude};
  animation: quantumWave 3s ease-in-out infinite;
  
  @keyframes quantumWave {
    0%, 100% {
      transform: scale(1) rotate(${props => props.phase}deg);
    }
    50% {
      transform: scale(1.2) rotate(${props => props.phase + 180}deg);
    }
  }
`;

const WaveCenter = styled.div`
  width: 8px;
  height: 8px;
  background: ${props => props.theme.colors.accent.primary};
  border-radius: 50%;
  z-index: 1;
`;

const OutcomeName = styled.div`
  font-size: 14px;
  font-weight: 500;
  color: ${props => props.theme.colors.text.primary};
`;

const AllocationInfo = styled.div`
  text-align: right;
`;

const Percentage = styled.div`
  font-size: 16px;
  font-weight: 600;
  color: ${props => props.theme.colors.accent.secondary};
`;

const Amount = styled.div`
  font-size: 12px;
  color: ${props => props.theme.colors.text.secondary};
`;

const StateVector = styled.div`
  margin-top: 16px;
  padding: 12px;
  background: rgba(0, 0, 0, 0.3);
  border-radius: 8px;
  font-family: ${props => props.theme.typography.fonts.mono};
  font-size: 12px;
  color: ${props => props.theme.colors.text.secondary};
  text-align: center;
  overflow-x: auto;
  white-space: nowrap;
`;

const EntanglementIndicator = styled.div`
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  margin-top: 12px;
  padding: 8px;
  background: ${props => props.theme.colors.quantum.entangled};
  border-radius: 6px;
  font-size: 12px;
  color: ${props => props.theme.colors.text.primary};
`;

const CollapseProbability = styled.div`
  margin-top: 12px;
  padding: 12px;
  background: rgba(255, 255, 255, 0.03);
  border-radius: 8px;
`;

const ProbabilityBar = styled.div`
  display: flex;
  height: 8px;
  background: rgba(255, 255, 255, 0.1);
  border-radius: 4px;
  overflow: hidden;
  margin-top: 8px;
`;

const ProbabilitySegment = styled.div<{ width: number; color: string }>`
  width: ${props => props.width}%;
  background: ${props => props.color};
  transition: width ${props => props.theme.animation.durations.normal} ${props => props.theme.animation.easings.default};
`;

const generateStateVector = (states: QuantumState[]): string => {
  const terms = states.map(state => {
    const amp = state.amplitude.toFixed(3);
    const phaseStr = state.phase !== 0 ? `e^(i${state.phase.toFixed(2)})` : '';
    return `${amp}${phaseStr}|${state.outcome}⟩`;
  });
  return `|Ψ⟩ = ${terms.join(' + ')}`;
};

export default function QuantumStateDisplay({ 
  states, 
  totalAmount, 
  isActive 
}: QuantumStateDisplayProps) {
  if (!isActive || states.length === 0) {
    return null;
  }

  const colors = ['#4CD964', '#FFD60A', '#FF3B30', '#007AFF', '#FF9500'];

  return (
    <Container>
      <Title>Quantum Distribution</Title>
      
      <StateGrid>
        {states.map((state, index) => (
          <StateItem key={state.outcome}>
            <OutcomeInfo>
              <WaveFunction>
                <WaveCircle 
                  amplitude={state.amplitude} 
                  phase={state.phase} 
                />
                <WaveCenter />
              </WaveFunction>
              <OutcomeName>{state.outcome}</OutcomeName>
            </OutcomeInfo>
            
            <AllocationInfo>
              <Percentage>{(state.probability * 100).toFixed(1)}%</Percentage>
              <Amount>{(totalAmount * state.allocation).toFixed(2)} SOL</Amount>
            </AllocationInfo>
          </StateItem>
        ))}
      </StateGrid>

      <StateVector>
        {generateStateVector(states)}
      </StateVector>

      <CollapseProbability>
        <Title style={{ marginBottom: '8px' }}>Collapse Probabilities</Title>
        <ProbabilityBar>
          {states.map((state, index) => (
            <ProbabilitySegment
              key={state.outcome}
              width={state.probability * 100}
              color={colors[index % colors.length]}
            />
          ))}
        </ProbabilityBar>
      </CollapseProbability>

      {states.length > 1 && (
        <EntanglementIndicator>
          <span>🔗</span>
          <span>States are entangled</span>
        </EntanglementIndicator>
      )}
    </Container>
  );
}