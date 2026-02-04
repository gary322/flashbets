"use strict";
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.RiskMetrics = void 0;
const react_1 = __importDefault(require("react"));
const styled_1 = __importDefault(require("@emotion/styled"));
const framer_motion_1 = require("framer-motion");
const Container = styled_1.default.div `
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 16px;
  margin-top: 16px;
`;
const MetricCard = (0, styled_1.default)(framer_motion_1.motion.div) `
  background: ${props => props.danger ?
    'rgba(220, 38, 38, 0.1)' :
    props.theme.colors.background.secondary};
  border: 1px solid ${props => props.danger ?
    'rgba(220, 38, 38, 0.3)' :
    'rgba(255, 255, 255, 0.05)'};
  border-radius: 8px;
  padding: 16px;
`;
const MetricLabel = styled_1.default.div `
  font-size: 12px;
  color: ${props => props.theme.colors.text.secondary};
  margin-bottom: 8px;
`;
const MetricValue = styled_1.default.div `
  font-size: ${props => props.size === 'large' ? '24px' : '18px'};
  font-weight: 700;
  font-family: ${props => props.theme.typography.fonts.mono};
  color: ${props => props.color || props.theme.colors.text.primary};
`;
const WarningIcon = styled_1.default.span `
  margin-left: 8px;
  color: ${props => props.theme.colors.status.warning};
`;
const ProgressBar = styled_1.default.div `
  width: 100%;
  height: 4px;
  background: ${props => props.theme.colors.background.primary};
  border-radius: 2px;
  margin-top: 8px;
  overflow: hidden;
`;
const ProgressFill = (0, styled_1.default)(framer_motion_1.motion.div) `
  height: 100%;
  background: ${props => props.danger ?
    props.theme.colors.accent.secondary :
    props.theme.colors.accent.primary};
  border-radius: 2px;
`;
const RiskMetrics = ({ leverage, liquidationPrice, entryPrice, marketVolatility = 0.05 }) => {
    const calculateLiquidationDistance = () => {
        if (!liquidationPrice || !entryPrice)
            return null;
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
    const formatPrice = (price) => {
        if (!price)
            return '--';
        return `${(price * 100).toFixed(2)}%`;
    };
    return (<Container>
      <MetricCard danger={liquidationDistance !== null && liquidationDistance < 1} whileHover={{ scale: 1.02 }}>
        <MetricLabel>
          Liquidation Price
          {liquidationDistance !== null && liquidationDistance < 2 && (<WarningIcon>⚠️</WarningIcon>)}
        </MetricLabel>
        <MetricValue color="#DC2626" size="large">
          {formatPrice(liquidationPrice)}
        </MetricValue>
        {liquidationDistance !== null && (<>
            <div style={{ fontSize: '11px', color: '#6B7280', marginTop: '4px' }}>
              {liquidationDistance.toFixed(2)}% from current
            </div>
            <ProgressBar>
              <ProgressFill percentage={Math.min(liquidationDistance, 10) * 10} danger={liquidationDistance < 2} initial={{ width: 0 }} animate={{ width: `${Math.min(liquidationDistance, 10) * 10}%` }} transition={{ duration: 0.5, ease: 'easeOut' }}/>
            </ProgressBar>
          </>)}
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
        <MetricValue color={riskLevel === 'high' ? '#DC2626' :
            riskLevel === 'medium' ? '#FFB800' :
                '#00FF88'}>
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
    </Container>);
};
exports.RiskMetrics = RiskMetrics;
