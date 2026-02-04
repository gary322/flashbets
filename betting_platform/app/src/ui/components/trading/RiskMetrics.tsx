import React from 'react';
import styled from '@emotion/styled';
import { motion } from 'framer-motion';

interface RiskMetricsProps {
  leverage: number;
  liquidationPrice: number | null;
  entryPrice?: number;
  marketVolatility?: number;
}

const Container = styled.div`
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 16px;
  margin-top: 16px;
`;

const MetricCard = styled(motion.div)<{ danger?: boolean }>`
  background: ${props => props.danger ? 
    'rgba(220, 38, 38, 0.1)' : 
    props.theme.colors.background.secondary};
  border: 1px solid ${props => props.danger ?
    'rgba(220, 38, 38, 0.3)' :
    'rgba(255, 255, 255, 0.05)'};
  border-radius: 8px;
  padding: 16px;
`;

const MetricLabel = styled.div`
  font-size: 12px;
  color: ${props => props.theme.colors.text.secondary};
  margin-bottom: 8px;
`;

const MetricValue = styled.div<{ size?: 'normal' | 'large'; color?: string }>`
  font-size: ${props => props.size === 'large' ? '24px' : '18px'};
  font-weight: 700;
  font-family: ${props => props.theme.typography.fonts.mono};
  color: ${props => props.color || props.theme.colors.text.primary};
`;

const WarningIcon = styled.span`
  margin-left: 8px;
  color: ${props => props.theme.colors.status.warning};
`;

const ProgressBar = styled.div`
  width: 100%;
  height: 4px;
  background: ${props => props.theme.colors.background.primary};
  border-radius: 2px;
  margin-top: 8px;
  overflow: hidden;
`;

const ProgressFill = styled(motion.div)<{ percentage: number; danger: boolean }>`
  height: 100%;
  background: ${props => props.danger ? 
    props.theme.colors.accent.secondary : 
    props.theme.colors.accent.primary};
  border-radius: 2px;
`;

export const RiskMetrics: React.FC<RiskMetricsProps> = ({
  leverage,
  liquidationPrice,
  entryPrice,
  marketVolatility = 0.05
}) => {
  const calculateLiquidationDistance = () => {
    if (!liquidationPrice || !entryPrice) return null;
    const distance = Math.abs(entryPrice - liquidationPrice) / entryPrice * 100;
    return distance;
  };

  const calculateMaxLoss = () => {
    // Max loss based on leverage and volatility
    return leverage * marketVolatility * 100;
  };

  const liquidationDistance = calculateLiquidationDistance();
  const maxLoss = calculateMaxLoss();
  const riskLevel = leverage > 100 ? 'high' : leverage > 50 ? 'medium' : 'low';

  const formatPrice = (price: number | null) => {
    if (!price) return '--';
    return `${(price * 100).toFixed(2)}%`;
  };

  return (
    <Container>
      <MetricCard
        danger={liquidationDistance !== null && liquidationDistance < 1}
        whileHover={{ scale: 1.02 }}
      >
        <MetricLabel>
          Liquidation Price
          {liquidationDistance !== null && liquidationDistance < 2 && (
            <WarningIcon>⚠️</WarningIcon>
          )}
        </MetricLabel>
        <MetricValue color="#DC2626" size="large">
          {formatPrice(liquidationPrice)}
        </MetricValue>
        {liquidationDistance !== null && (
          <>
            <div style={{ fontSize: '11px', color: '#6B7280', marginTop: '4px' }}>
              {liquidationDistance.toFixed(2)}% from current
            </div>
            <ProgressBar>
              <ProgressFill
                percentage={Math.min(liquidationDistance, 10) * 10}
                danger={liquidationDistance < 2}
                initial={{ width: 0 }}
                animate={{ width: `${Math.min(liquidationDistance, 10) * 10}%` }}
                transition={{ duration: 0.5, ease: 'easeOut' }}
              />
            </ProgressBar>
          </>
        )}
      </MetricCard>

      <MetricCard whileHover={{ scale: 1.02 }}>
        <MetricLabel>Max Loss (1σ move)</MetricLabel>
        <MetricValue color={maxLoss > 50 ? '#DC2626' : undefined}>
          ${maxLoss.toFixed(2)}
        </MetricValue>
        <div style={{ fontSize: '11px', color: '#6B7280', marginTop: '4px' }}>
          per $100 position
        </div>
      </MetricCard>

      <MetricCard whileHover={{ scale: 1.02 }}>
        <MetricLabel>Risk Level</MetricLabel>
        <MetricValue 
          color={
            riskLevel === 'high' ? '#DC2626' : 
            riskLevel === 'medium' ? '#FFB800' : 
            '#00FF88'
          }
        >
          {riskLevel.toUpperCase()}
        </MetricValue>
        <div style={{ fontSize: '11px', color: '#6B7280', marginTop: '4px' }}>
          Based on {leverage}x leverage
        </div>
      </MetricCard>

      <MetricCard whileHover={{ scale: 1.02 }}>
        <MetricLabel>Market Volatility</MetricLabel>
        <MetricValue>
          {(marketVolatility * 100).toFixed(1)}%
        </MetricValue>
        <div style={{ fontSize: '11px', color: '#6B7280', marginTop: '4px' }}>
          24h average
        </div>
      </MetricCard>
    </Container>
  );
};