import React, { useState, useEffect } from 'react';
import styled from '@emotion/styled';
import { BlurCard } from '../core/BlurCard';

// Types
interface TPSMetrics {
  currentTPS: number;
  maxTPS: number;
  userArbSpeed: string;
  systemLoad: number;
}

// Styled Components
const TPSContainer = styled(BlurCard)`
  padding: 16px 24px;
  display: flex;
  align-items: center;
  gap: 24px;
  background: ${props => props.theme.colors.background.secondary};
  border: 1px solid rgba(255, 255, 255, 0.05);
`;

const MetricGroup = styled.div`
  display: flex;
  flex-direction: column;
  gap: 4px;
`;

const MetricLabel = styled.span`
  font-size: 12px;
  color: ${props => props.theme.colors.text.secondary};
  text-transform: uppercase;
  letter-spacing: 0.5px;
`;

const MetricValue = styled.span`
  font-size: 20px;
  font-weight: 700;
  font-family: ${props => props.theme.typography.fonts.mono};
  color: ${props => props.theme.colors.text.primary};
`;

const TPSIndicator = styled.div<{ load: number }>`
  width: 120px;
  height: 6px;
  background: ${props => props.theme.colors.background.tertiary};
  border-radius: 3px;
  position: relative;
  overflow: hidden;
  
  &::after {
    content: '';
    position: absolute;
    left: 0;
    top: 0;
    height: 100%;
    width: ${props => props.load}%;
    background: ${props => 
      props.load > 80 ? props.theme.colors.accent.secondary :
      props.load > 50 ? props.theme.colors.status.warning :
      props.theme.colors.accent.primary
    };
    transition: width 0.3s ease, background 0.3s ease;
  }
`;

const StatusDot = styled.div<{ status: 'active' | 'warning' | 'error' }>`
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: ${props => 
    props.status === 'active' ? '#00ff88' :
    props.status === 'warning' ? '#ff9500' :
    '#ff3b30'
  };
  animation: pulse 2s infinite;
  
  @keyframes pulse {
    0% { opacity: 1; }
    50% { opacity: 0.5; }
    100% { opacity: 1; }
  }
`;

const ArbSpeedBadge = styled.div<{ rank: string }>`
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 4px 12px;
  background: ${props => 
    props.rank.includes('Top 10%') ? 'rgba(0, 255, 136, 0.1)' :
    props.rank.includes('Top 25%') ? 'rgba(255, 149, 0, 0.1)' :
    'rgba(255, 255, 255, 0.05)'
  };
  border: 1px solid ${props => 
    props.rank.includes('Top 10%') ? 'rgba(0, 255, 136, 0.3)' :
    props.rank.includes('Top 25%') ? 'rgba(255, 149, 0, 0.3)' :
    'rgba(255, 255, 255, 0.1)'
  };
  border-radius: 16px;
  font-size: 12px;
  font-weight: 600;
  color: ${props => 
    props.rank.includes('Top 10%') ? '#00ff88' :
    props.rank.includes('Top 25%') ? '#ff9500' :
    props.theme.colors.text.secondary
  };
`;

// Component
export const TPSDisplay: React.FC = () => {
  const [metrics, setMetrics] = useState<TPSMetrics>({
    currentTPS: 0,
    maxTPS: 5000,
    userArbSpeed: 'Calculating...',
    systemLoad: 0,
  });

  useEffect(() => {
    // Simulate real-time TPS updates
    const updateMetrics = () => {
      setMetrics(prev => {
        const baseTPSVariation = Math.random() * 1000;
        const currentTPS = Math.floor(4000 + baseTPSVariation);
        const systemLoad = (currentTPS / prev.maxTPS) * 100;
        
        // Determine user arbitrage speed ranking based on stake
        const userStake = Math.random(); // Simulate user stake percentage
        let userArbSpeed = 'Average';
        if (userStake > 0.9) {
          userArbSpeed = 'Top 10% from stake';
        } else if (userStake > 0.75) {
          userArbSpeed = 'Top 25% from stake';
        } else if (userStake > 0.5) {
          userArbSpeed = 'Above Average';
        }
        
        return {
          currentTPS,
          maxTPS: prev.maxTPS,
          userArbSpeed,
          systemLoad,
        };
      });
    };

    // Update every 2 seconds
    const interval = setInterval(updateMetrics, 2000);
    updateMetrics(); // Initial update

    return () => clearInterval(interval);
  }, []);

  const getSystemStatus = (): 'active' | 'warning' | 'error' => {
    if (metrics.systemLoad > 90) return 'error';
    if (metrics.systemLoad > 75) return 'warning';
    return 'active';
  };

  return (
    <TPSContainer>
      <StatusDot status={getSystemStatus()} />
      
      <MetricGroup>
        <MetricLabel>System TPS</MetricLabel>
        <MetricValue>{metrics.currentTPS.toLocaleString()}</MetricValue>
      </MetricGroup>
      
      <TPSIndicator load={metrics.systemLoad} />
      
      <MetricGroup>
        <MetricLabel>Max TPS</MetricLabel>
        <MetricValue>{metrics.maxTPS.toLocaleString()}</MetricValue>
      </MetricGroup>
      
      <MetricGroup style={{ marginLeft: 'auto' }}>
        <MetricLabel>Your Arb Speed</MetricLabel>
        <ArbSpeedBadge rank={metrics.userArbSpeed}>
          {metrics.userArbSpeed}
        </ArbSpeedBadge>
      </MetricGroup>
    </TPSContainer>
  );
};