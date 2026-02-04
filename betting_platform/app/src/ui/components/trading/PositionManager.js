"use strict";
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.PositionManager = void 0;
const react_1 = __importDefault(require("react"));
const styled_1 = __importDefault(require("@emotion/styled"));
const framer_motion_1 = require("framer-motion");
const BlurCard_1 = require("../core/BlurCard");
const Container = styled_1.default.div `
  height: 100%;
  display: flex;
  flex-direction: column;
`;
const Header = styled_1.default.div `
  padding: 16px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.05);
`;
const Title = styled_1.default.h3 `
  font-size: 18px;
  font-weight: 600;
  color: ${props => props.theme.colors.text.primary};
  margin: 0;
`;
const PositionsList = styled_1.default.div `
  flex: 1;
  overflow-y: auto;
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 12px;
`;
const PositionCard = (0, styled_1.default)(BlurCard_1.BlurCard) `
  padding: 16px;
`;
const PositionHeader = styled_1.default.div `
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 12px;
`;
const PositionSide = styled_1.default.span `
  font-size: 12px;
  font-weight: 600;
  padding: 4px 8px;
  border-radius: 4px;
  background: ${props => props.side === 'long' ?
    props.theme.colors.accent.primary + '20' :
    props.theme.colors.accent.secondary + '20'};
  color: ${props => props.side === 'long' ?
    props.theme.colors.accent.primary :
    props.theme.colors.accent.secondary};
`;
const PnL = styled_1.default.div `
  font-size: 20px;
  font-weight: 700;
  font-family: ${props => props.theme.typography.fonts.mono};
  color: ${props => props.positive ?
    props.theme.colors.accent.primary :
    props.theme.colors.accent.secondary};
`;
const MetricsGrid = styled_1.default.div `
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 12px;
`;
const Metric = styled_1.default.div `
  display: flex;
  flex-direction: column;
`;
const MetricLabel = styled_1.default.span `
  font-size: 12px;
  color: ${props => props.theme.colors.text.tertiary};
  margin-bottom: 4px;
`;
const MetricValue = styled_1.default.span `
  font-size: 14px;
  font-weight: 600;
  color: ${props => props.theme.colors.text.primary};
  font-family: ${props => props.theme.typography.fonts.mono};
`;
const Actions = styled_1.default.div `
  display: flex;
  gap: 8px;
  margin-top: 12px;
`;
const ActionButton = styled_1.default.button `
  flex: 1;
  padding: 8px 12px;
  border: none;
  border-radius: 6px;
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  transition: all 200ms ease;
  
  ${props => props.variant === 'primary' ? `
    background: ${props.theme.colors.background.tertiary};
    color: ${props.theme.colors.text.primary};
    
    &:hover {
      background: ${props.theme.colors.background.primary};
    }
  ` : `
    background: ${props.theme.colors.accent.secondary}20;
    color: ${props.theme.colors.accent.secondary};
    
    &:hover {
      background: ${props.theme.colors.accent.secondary}30;
    }
  `}
`;
const PositionManager = ({ positions, prices, onClose, onModify }) => {
    const calculatePnL = (position, currentPrice) => {
        const priceDiff = position.side === 'long'
            ? currentPrice - position.entryPrice
            : position.entryPrice - currentPrice;
        return priceDiff * position.size * position.effectiveLeverage;
    };
    const formatCurrency = (value) => {
        return `$${Math.abs(value).toFixed(2)}`;
    };
    const formatPercentage = (value) => {
        return `${value.toFixed(2)}%`;
    };
    return (<Container>
      <Header>
        <Title>Active Positions ({positions.length})</Title>
      </Header>
      
      <PositionsList>
        <framer_motion_1.AnimatePresence>
          {positions.map(position => {
            var _a;
            const currentPrice = ((_a = prices.get(position.marketId)) === null || _a === void 0 ? void 0 : _a.price) || position.entryPrice;
            const pnl = calculatePnL(position, currentPrice);
            const pnlPercentage = (pnl / position.margin) * 100;
            return (<framer_motion_1.motion.div key={position.id} initial={{ opacity: 0, y: 20 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0, y: -20 }} transition={{ duration: 0.3 }}>
                <PositionCard danger={pnl < 0}>
                  <PositionHeader>
                    <PositionSide side={position.side}>
                      {position.side.toUpperCase()}
                    </PositionSide>
                    <PnL positive={pnl >= 0}>
                      {pnl >= 0 ? '+' : ''}{formatCurrency(pnl)}
                      <span style={{ fontSize: '12px', marginLeft: '4px' }}>
                        ({pnlPercentage >= 0 ? '+' : ''}{pnlPercentage.toFixed(1)}%)
                      </span>
                    </PnL>
                  </PositionHeader>
                  
                  <MetricsGrid>
                    <Metric>
                      <MetricLabel>Entry Price</MetricLabel>
                      <MetricValue>{formatPercentage(position.entryPrice * 100)}</MetricValue>
                    </Metric>
                    <Metric>
                      <MetricLabel>Current Price</MetricLabel>
                      <MetricValue>{formatPercentage(currentPrice * 100)}</MetricValue>
                    </Metric>
                    <Metric>
                      <MetricLabel>Effective Leverage</MetricLabel>
                      <MetricValue>{position.effectiveLeverage}x</MetricValue>
                    </Metric>
                    <Metric>
                      <MetricLabel>Liquidation Price</MetricLabel>
                      <MetricValue style={{ color: '#DC2626' }}>
                        {formatPercentage(position.liquidationPrice * 100)}
                      </MetricValue>
                    </Metric>
                  </MetricsGrid>
                  
                  <Actions>
                    <ActionButton variant="primary" onClick={() => onModify(position.id, {})}>
                      Modify
                    </ActionButton>
                    <ActionButton variant="danger" onClick={() => onClose(position.id)}>
                      Close Position
                    </ActionButton>
                  </Actions>
                </PositionCard>
              </framer_motion_1.motion.div>);
        })}
        </framer_motion_1.AnimatePresence>
        
        {positions.length === 0 && (<div style={{
                textAlign: 'center',
                padding: '48px',
                color: '#6B7280'
            }}>
            No active positions
          </div>)}
      </PositionsList>
    </Container>);
};
exports.PositionManager = PositionManager;
