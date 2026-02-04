"use strict";
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.ChainBuilder = void 0;
const react_1 = __importDefault(require("react"));
const styled_1 = __importDefault(require("@emotion/styled"));
const framer_motion_1 = require("framer-motion");
const Container = styled_1.default.div `
  padding: 16px;
  background: ${props => props.theme.colors.background.primary};
  border: 1px solid rgba(255, 255, 255, 0.05);
  border-radius: 8px;
  margin-top: 16px;
`;
const Header = styled_1.default.div `
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 16px;
`;
const Title = styled_1.default.h4 `
  font-size: 14px;
  font-weight: 600;
  color: ${props => props.theme.colors.text.primary};
  margin: 0;
`;
const MultiplierBadge = styled_1.default.div `
  background: ${props => props.theme.colors.accent.leverage}20;
  color: ${props => props.theme.colors.accent.leverage};
  padding: 4px 12px;
  border-radius: 4px;
  font-size: 14px;
  font-weight: 700;
  font-family: ${props => props.theme.typography.fonts.mono};
`;
const StepsList = styled_1.default.div `
  display: flex;
  flex-direction: column;
  gap: 8px;
`;
const StepItem = (0, styled_1.default)(framer_motion_1.motion.div) `
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px;
  background: ${props => props.theme.colors.background.secondary};
  border: 1px solid rgba(255, 255, 255, 0.05);
  border-radius: 6px;
`;
const StepIcon = styled_1.default.div `
  width: 32px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 50%;
  font-size: 14px;
  background: ${props => {
    const colors = {
        borrow: '#3B82F6',
        liquidity: '#00FF88',
        hedge: '#FFB800',
        arbitrage: '#FF3333'
    };
    return colors[props.type] + '20';
}};
  color: ${props => {
    const colors = {
        borrow: '#3B82F6',
        liquidity: '#00FF88',
        hedge: '#FFB800',
        arbitrage: '#FF3333'
    };
    return colors[props.type];
}};
`;
const StepInfo = styled_1.default.div `
  flex: 1;
`;
const StepType = styled_1.default.div `
  font-size: 12px;
  font-weight: 600;
  color: ${props => props.theme.colors.text.primary};
  text-transform: capitalize;
`;
const StepMultiplier = styled_1.default.div `
  font-size: 11px;
  color: ${props => props.theme.colors.text.secondary};
  font-family: ${props => props.theme.typography.fonts.mono};
`;
const RemoveButton = styled_1.default.button `
  background: transparent;
  border: none;
  color: ${props => props.theme.colors.text.tertiary};
  cursor: pointer;
  padding: 4px;
  transition: color 200ms ease;
  
  &:hover {
    color: ${props => props.theme.colors.accent.secondary};
  }
`;
const AddStepButton = styled_1.default.button `
  width: 100%;
  padding: 12px;
  margin-top: 12px;
  background: transparent;
  border: 1px dashed rgba(255, 255, 255, 0.2);
  border-radius: 6px;
  color: ${props => props.theme.colors.text.secondary};
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  transition: all 200ms ease;
  
  &:hover {
    border-color: ${props => props.theme.colors.accent.primary};
    color: ${props => props.theme.colors.accent.primary};
  }
  
  &:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
`;
const StepTemplates = styled_1.default.div `
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 8px;
  margin-top: 8px;
`;
const TemplateButton = styled_1.default.button `
  padding: 8px 12px;
  background: ${props => props.theme.colors.background.secondary};
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 4px;
  color: ${props => props.theme.colors.text.primary};
  font-size: 11px;
  font-weight: 600;
  cursor: pointer;
  transition: all 200ms ease;
  
  &:hover {
    border-color: ${props => props.theme.colors.accent.primary};
  }
`;
const ChainBuilder = ({ steps, onChange, maxSteps, verseId }) => {
    const [showTemplates, setShowTemplates] = react_1.default.useState(false);
    const totalMultiplier = steps.reduce((total, step) => total * (1 + step.multiplier), 1);
    const stepTemplates = [
        { type: 'borrow', multiplier: 0.5 },
        { type: 'liquidity', multiplier: 0.3 },
        { type: 'hedge', multiplier: 0.2 },
        { type: 'arbitrage', multiplier: 0.4 }
    ];
    const addStep = (template) => {
        const newStep = {
            id: Date.now().toString(),
            type: template.type,
            multiplier: template.multiplier,
            params: {}
        };
        onChange([...steps, newStep]);
        setShowTemplates(false);
    };
    const removeStep = (stepId) => {
        onChange(steps.filter(step => step.id !== stepId));
    };
    const getStepIcon = (type) => {
        const icons = {
            borrow: '💰',
            liquidity: '💧',
            hedge: '🛡️',
            arbitrage: '⚡'
        };
        return icons[type];
    };
    return (<Container>
      <Header>
        <Title>Leverage Chain Builder</Title>
        <MultiplierBadge>
          {totalMultiplier.toFixed(2)}x Total
        </MultiplierBadge>
      </Header>

      <StepsList>
        <framer_motion_1.AnimatePresence>
          {steps.map((step, index) => (<StepItem key={step.id} initial={{ opacity: 0, x: -20 }} animate={{ opacity: 1, x: 0 }} exit={{ opacity: 0, x: 20 }} transition={{ duration: 0.2, delay: index * 0.05 }}>
              <StepIcon type={step.type}>
                {getStepIcon(step.type)}
              </StepIcon>
              <StepInfo>
                <StepType>{step.type}</StepType>
                <StepMultiplier>+{(step.multiplier * 100).toFixed(0)}% boost</StepMultiplier>
              </StepInfo>
              <RemoveButton onClick={() => removeStep(step.id)}>
                ✕
              </RemoveButton>
            </StepItem>))}
        </framer_motion_1.AnimatePresence>
      </StepsList>

      {steps.length < maxSteps && (<>
          <AddStepButton onClick={() => setShowTemplates(!showTemplates)} disabled={steps.length >= maxSteps}>
            + Add Chain Step
          </AddStepButton>

          <framer_motion_1.AnimatePresence>
            {showTemplates && (<framer_motion_1.motion.div initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }} exit={{ opacity: 0, height: 0 }}>
                <StepTemplates>
                  {stepTemplates.map((template, index) => (<TemplateButton key={index} onClick={() => addStep(template)}>
                      {template.type} (+{(template.multiplier * 100).toFixed(0)}%)
                    </TemplateButton>))}
                </StepTemplates>
              </framer_motion_1.motion.div>)}
          </framer_motion_1.AnimatePresence>
        </>)}
    </Container>);
};
exports.ChainBuilder = ChainBuilder;
