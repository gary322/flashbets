import React from 'react';
import styled from '@emotion/styled';
import { useWallet } from '@solana/wallet-adapter-react';

interface RightPanelProps {
  selectedMarket?: any;
  selectedOutcome?: number;
  onOutcomeSelect?: (outcome: number) => void;
  quantumToggle?: React.ReactNode;
  quantumStates?: React.ReactNode;
  onExecuteTrade?: () => void;
}

const PanelContainer = styled.div`
  display: flex;
  flex-direction: column;
  height: 100vh;
  overflow: hidden;
`;

const TradingHeader = styled.div`
  padding: 24px;
  border-bottom: 1px solid ${props => props.theme.colors.text.tertiary};
`;

const TradingContent = styled.div`
  flex: 1;
  overflow-y: auto;
  padding: 24px;
`;

const OrderTabs = styled.div`
  display: flex;
  gap: 1px;
  background: rgba(255, 255, 255, 0.05);
  border-radius: 8px;
  padding: 2px;
  margin-bottom: 24px;
`;

const OrderTab = styled.button<{ active?: boolean }>`
  flex: 1;
  padding: 10px;
  background: ${props => props.active ? props.theme.colors.text.primary : 'transparent'};
  border: none;
  border-radius: 6px;
  color: ${props => props.active ? props.theme.colors.text.inverse : props.theme.colors.text.secondary};
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  transition: all ${props => props.theme.animation.durations.fast} ${props => props.theme.animation.easings.default};
  
  &:hover:not(.active) {
    color: ${props => props.theme.colors.text.primary};
  }
`;

const PositionSection = styled.div`
  margin-bottom: 24px;
`;

const InputGroup = styled.div`
  margin-bottom: 16px;
`;

const InputLabel = styled.label`
  font-size: 12px;
  color: ${props => props.theme.colors.text.secondary};
  margin-bottom: 8px;
  display: block;
`;

const InputContainer = styled.div`
  position: relative;
`;

const AmountInput = styled.input`
  width: 100%;
  padding: 12px 16px;
  padding-right: 48px;
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 8px;
  color: ${props => props.theme.colors.text.primary};
  font-size: 16px;
  font-weight: 500;
  transition: all ${props => props.theme.animation.durations.fast} ${props => props.theme.animation.easings.default};
  
  &:focus {
    outline: none;
    border-color: ${props => props.theme.colors.accent.primary};
    background: rgba(255, 255, 255, 0.08);
  }
  
  &::placeholder {
    color: ${props => props.theme.colors.text.tertiary};
  }
`;

const InputSuffix = styled.span`
  position: absolute;
  right: 16px;
  top: 50%;
  transform: translateY(-50%);
  color: ${props => props.theme.colors.text.tertiary};
  font-size: 14px;
`;

const LeverageDisplay = styled.div`
  background: rgba(255, 255, 255, 0.03);
  border-radius: 12px;
  padding: 16px;
  margin-bottom: 24px;
`;

const LeverageHeader = styled.div`
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 12px;
`;

const LeverageTitle = styled.span`
  font-size: 12px;
  color: ${props => props.theme.colors.text.secondary};
`;

const LeverageTotal = styled.span`
  font-size: 24px;
  font-weight: 600;
  color: ${props => props.theme.colors.accent.primary};
`;

const LeverageBreakdown = styled.div`
  font-size: 12px;
  color: ${props => props.theme.colors.text.secondary};
  line-height: 1.6;
`;

const LeverageItem = styled.div`
  display: flex;
  justify-content: space-between;
  padding: 4px 0;
`;

const RiskSection = styled.div`
  background: rgba(255, 255, 255, 0.03);
  border-radius: 12px;
  padding: 16px;
  margin-bottom: 24px;
`;

const RiskControl = styled.div`
  margin-bottom: 16px;
  
  &:last-child {
    margin-bottom: 0;
  }
`;

const RiskLabel = styled.div`
  font-size: 12px;
  color: ${props => props.theme.colors.text.secondary};
  margin-bottom: 8px;
  display: flex;
  justify-content: space-between;
  align-items: center;
`;

const RiskValue = styled.span`
  color: ${props => props.theme.colors.text.primary};
  font-weight: 500;
`;

const Slider = styled.input`
  width: 100%;
  height: 4px;
  background: rgba(255, 255, 255, 0.1);
  border-radius: 2px;
  outline: none;
  -webkit-appearance: none;
  
  &::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: 16px;
    height: 16px;
    background: ${props => props.theme.colors.accent.primary};
    border-radius: 50%;
    cursor: pointer;
  }
  
  &::-moz-range-thumb {
    width: 16px;
    height: 16px;
    background: ${props => props.theme.colors.accent.primary};
    border-radius: 50%;
    cursor: pointer;
    border: none;
  }
`;

const ExecuteSection = styled.div`
  padding: 24px;
  border-top: 1px solid ${props => props.theme.colors.text.tertiary};
`;

const PositionSummary = styled.div`
  background: rgba(255, 255, 255, 0.03);
  border-radius: 12px;
  padding: 16px;
  margin-bottom: 16px;
`;

const SummaryRow = styled.div`
  display: flex;
  justify-content: space-between;
  padding: 8px 0;
  border-bottom: 1px solid rgba(255, 255, 255, 0.05);
  
  &:last-child {
    border-bottom: none;
  }
`;

const SummaryLabel = styled.span`
  font-size: 12px;
  color: ${props => props.theme.colors.text.secondary};
`;

const SummaryValue = styled.span`
  font-size: 14px;
  font-weight: 500;
  color: ${props => props.theme.colors.text.primary};
  
  &.highlight {
    color: ${props => props.theme.colors.accent.primary};
    font-size: 16px;
  }
`;

const ExecuteBtn = styled.button`
  width: 100%;
  padding: 16px;
  background: linear-gradient(135deg, ${props => props.theme.colors.accent.primary} 0%, ${props => props.theme.colors.accent.secondary} 100%);
  border: none;
  border-radius: 12px;
  color: ${props => props.theme.colors.text.inverse};
  font-size: 16px;
  font-weight: 600;
  cursor: pointer;
  transition: all ${props => props.theme.animation.durations.fast} ${props => props.theme.animation.easings.default};
  position: relative;
  overflow: hidden;
  
  &:hover {
    transform: translateY(-2px);
    box-shadow: 0 8px 24px rgba(255, 214, 10, 0.3);
  }
  
  &:active {
    transform: translateY(0);
  }
  
  &:disabled {
    opacity: 0.5;
    cursor: not-allowed;
    transform: none;
  }
`;

const OutcomeSelector = styled.div`
  display: flex;
  gap: 12px;
  margin-bottom: 24px;
`;

const OutcomeButton = styled.button<{ selected?: boolean }>`
  flex: 1;
  padding: 12px;
  border-radius: 8px;
  border: 2px solid ${props => props.selected 
    ? props.theme.colors.accent.primary 
    : 'rgba(255, 255, 255, 0.1)'
  };
  background: ${props => props.selected 
    ? 'rgba(255, 214, 10, 0.1)' 
    : 'transparent'
  };
  color: ${props => props.theme.colors.text.primary};
  font-weight: 500;
  cursor: pointer;
  transition: all ${props => props.theme.animation.durations.fast} ${props => props.theme.animation.easings.default};
  
  &:hover {
    border-color: ${props => props.theme.colors.accent.primary};
  }
`;

export default function RightPanel({
  selectedMarket,
  selectedOutcome = 0,
  onOutcomeSelect,
  quantumToggle,
  quantumStates,
  onExecuteTrade
}: RightPanelProps) {
  const { connected } = useWallet();
  const [orderType, setOrderType] = React.useState<'market' | 'limit'>('market');
  const [amount, setAmount] = React.useState('');
  const [stopLoss, setStopLoss] = React.useState(20);
  const [takeProfit, setTakeProfit] = React.useState(50);
  const [leverage, setLeverage] = React.useState(5);

  const calculateExposure = () => {
    const amountNum = parseFloat(amount) || 0;
    return (amountNum * leverage).toFixed(2);
  };

  return (
    <PanelContainer>
      <TradingHeader>
        {quantumToggle}
      </TradingHeader>

      <TradingContent>
        <OrderTabs>
          <OrderTab 
            active={orderType === 'market'} 
            onClick={() => setOrderType('market')}
          >
            Market
          </OrderTab>
          <OrderTab 
            active={orderType === 'limit'} 
            onClick={() => setOrderType('limit')}
          >
            Limit
          </OrderTab>
        </OrderTabs>

        {selectedMarket && (
          <>
            <OutcomeSelector>
              {selectedMarket.outcomes?.map((outcome: any, index: number) => (
                <OutcomeButton
                  key={index}
                  selected={selectedOutcome === index}
                  onClick={() => onOutcomeSelect?.(index)}
                >
                  {outcome.name}
                </OutcomeButton>
              ))}
            </OutcomeSelector>

            <PositionSection>
              <InputGroup>
                <InputLabel>Investment Amount</InputLabel>
                <InputContainer>
                  <AmountInput
                    type="number"
                    placeholder="0.00"
                    value={amount}
                    onChange={(e) => setAmount(e.target.value)}
                  />
                  <InputSuffix>SOL</InputSuffix>
                </InputContainer>
              </InputGroup>
            </PositionSection>

            <LeverageDisplay>
              <LeverageHeader>
                <LeverageTitle>Total Leverage</LeverageTitle>
                <LeverageTotal>{leverage}x</LeverageTotal>
              </LeverageHeader>
              <LeverageBreakdown>
                <LeverageItem>
                  <span>Base Leverage</span>
                  <span>5.0x</span>
                </LeverageItem>
                <LeverageItem>
                  <span>Verse Multiplier</span>
                  <span>1.0x</span>
                </LeverageItem>
              </LeverageBreakdown>
            </LeverageDisplay>

            {quantumStates}

            <RiskSection>
              <RiskControl>
                <RiskLabel>
                  <span>Stop Loss</span>
                  <RiskValue>-{stopLoss}%</RiskValue>
                </RiskLabel>
                <Slider
                  type="range"
                  min="5"
                  max="50"
                  value={stopLoss}
                  onChange={(e) => setStopLoss(Number(e.target.value))}
                />
              </RiskControl>
              <RiskControl>
                <RiskLabel>
                  <span>Take Profit</span>
                  <RiskValue>+{takeProfit}%</RiskValue>
                </RiskLabel>
                <Slider
                  type="range"
                  min="10"
                  max="200"
                  value={takeProfit}
                  onChange={(e) => setTakeProfit(Number(e.target.value))}
                />
              </RiskControl>
            </RiskSection>
          </>
        )}
      </TradingContent>

      <ExecuteSection>
        <PositionSummary>
          <SummaryRow>
            <SummaryLabel>Investment</SummaryLabel>
            <SummaryValue>{amount || '0'} SOL</SummaryValue>
          </SummaryRow>
          <SummaryRow>
            <SummaryLabel>Leverage</SummaryLabel>
            <SummaryValue>{leverage}x</SummaryValue>
          </SummaryRow>
          <SummaryRow>
            <SummaryLabel>Total Exposure</SummaryLabel>
            <SummaryValue className="highlight">{calculateExposure()} SOL</SummaryValue>
          </SummaryRow>
        </PositionSummary>
        
        <ExecuteBtn 
          onClick={onExecuteTrade}
          disabled={!connected || !selectedMarket || !amount}
        >
          {connected ? 'Execute Order' : 'Connect Wallet'}
        </ExecuteBtn>
      </ExecuteSection>
    </PanelContainer>
  );
}