"use strict";
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.LeverageSlider = void 0;
const react_1 = __importStar(require("react"));
const styled_1 = __importDefault(require("@emotion/styled"));
const framer_motion_1 = require("framer-motion");
const SliderContainer = styled_1.default.div `
  position: relative;
  padding: 24px 0;
`;
const SliderTrack = styled_1.default.div `
  height: 8px;
  background: ${props => props.extreme ? 'linear-gradient(90deg, #1F2937 0%, #DC2626 100%)' :
    props.danger ? 'linear-gradient(90deg, #1F2937 0%, #F59E0B 100%)' :
        'linear-gradient(90deg, #1F2937 0%, #10B981 100%)'};
  border-radius: 4px;
  position: relative;
  cursor: pointer;
`;
const SliderThumb = (0, styled_1.default)(framer_motion_1.motion.div) `
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
const LeverageDisplay = styled_1.default.div `
  display: flex;
  justify-content: space-between;
  align-items: baseline;
  margin-bottom: 16px;
`;
const CurrentLeverage = styled_1.default.div `
  font-size: ${props => props.size === 'large' ? '48px' : '32px'};
  font-weight: 900;
  font-family: ${props => props.theme.typography.fonts.mono};
  color: ${props => props.theme.colors.text.primary};
  
  span {
    font-size: 24px;
    color: ${props => props.theme.colors.text.secondary};
  }
`;
const EffectiveLeverage = (0, styled_1.default)(framer_motion_1.motion.div) `
  font-size: 18px;
  color: ${props => props.theme.colors.accent.warning};
  font-family: ${props => props.theme.typography.fonts.mono};
`;
const WarningMessage = (0, styled_1.default)(framer_motion_1.motion.div) `
  margin-top: 16px;
  padding: 12px 16px;
  background: ${props => props.severity === 'danger' ?
    'rgba(220, 38, 38, 0.1)' :
    'rgba(245, 158, 11, 0.1)'};
  border: 1px solid ${props => props.severity === 'danger' ?
    'rgba(220, 38, 38, 0.3)' :
    'rgba(245, 158, 11, 0.3)'};
  border-radius: 8px;
  font-size: 13px;
  color: ${props => props.severity === 'danger' ?
    '#EF4444' :
    '#F59E0B'};
`;
const PresetButtons = styled_1.default.div `
  display: flex;
  gap: 8px;
  margin-top: 16px;
`;
const PresetButton = styled_1.default.button `
  padding: 8px 16px;
  background: ${props => props.active ?
    props.theme.colors.accent.primary :
    'transparent'};
  color: ${props => props.active ?
    props.theme.colors.background.primary :
    props.theme.colors.text.secondary};
  border: 1px solid ${props => props.active ?
    props.theme.colors.accent.primary :
    'rgba(255, 255, 255, 0.1)'};
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
const LeverageSlider = ({ value, onChange, max, effectiveLeverage, showWarnings, coverage }) => {
    const presets = [1, 10, 25, 50, 100];
    const warningLevel = (0, react_1.useMemo)(() => {
        if (effectiveLeverage >= 300)
            return 'extreme';
        if (effectiveLeverage >= 100)
            return 'danger';
        if (effectiveLeverage >= 50)
            return 'warning';
        return 'safe';
    }, [effectiveLeverage]);
    const liquidationBuffer = (0, react_1.useMemo)(() => {
        return (1 / effectiveLeverage) * 100;
    }, [effectiveLeverage]);
    const handleSliderChange = (e) => {
        const rect = e.currentTarget.getBoundingClientRect();
        const x = e.clientX - rect.left;
        const percentage = Math.max(0, Math.min(1, x / rect.width));
        const newValue = Math.round(percentage * max);
        onChange(newValue);
    };
    return (<SliderContainer>
      <LeverageDisplay>
        <div>
          <div style={{ fontSize: '13px', color: '#6B7280', marginBottom: '4px' }}>
            Base Leverage
          </div>
          <CurrentLeverage size="normal">
            {value}<span>x</span>
          </CurrentLeverage>
        </div>

        {effectiveLeverage !== value && (<div style={{ textAlign: 'right' }}>
            <div style={{ fontSize: '13px', color: '#6B7280', marginBottom: '4px' }}>
              Effective (with chain)
            </div>
            <EffectiveLeverage initial={{ opacity: 0, y: -10 }} animate={{ opacity: 1, y: 0 }} transition={{ duration: 0.3 }}>
              {effectiveLeverage.toFixed(1)}x
            </EffectiveLeverage>
          </div>)}
      </LeverageDisplay>

      <SliderTrack danger={warningLevel === 'danger' || warningLevel === 'extreme'} extreme={warningLevel === 'extreme'} onClick={handleSliderChange}>
        <SliderThumb danger={warningLevel !== 'safe'} style={{ left: `${(value / max) * 100}%` }} drag="x" dragConstraints={{ left: 0, right: 0 }} dragElastic={0} dragMomentum={false} onDrag={(e, info) => {
            var _a;
            const target = e.target;
            const rect = (_a = target.parentElement) === null || _a === void 0 ? void 0 : _a.getBoundingClientRect();
            if (!rect)
                return;
            const percentage = Math.max(0, Math.min(1, (info.point.x - rect.left) / rect.width));
            onChange(Math.round(percentage * max));
        }} whileHover={{ scale: 1.1 }} whileDrag={{ scale: 1.2 }}/>
      </SliderTrack>

      <PresetButtons>
        {presets.map(preset => (<PresetButton key={preset} active={value === preset} onClick={() => onChange(preset)}>
            {preset}x
          </PresetButton>))}
        <PresetButton active={value === max} onClick={() => onChange(max)} style={{ marginLeft: 'auto' }}>
          MAX
        </PresetButton>
      </PresetButtons>

      <framer_motion_1.AnimatePresence>
        {showWarnings && warningLevel !== 'safe' && (<WarningMessage severity={warningLevel === 'extreme' ? 'danger' : 'warning'} initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }} exit={{ opacity: 0, height: 0 }}>
            {warningLevel === 'extreme' ? (<>
                ⚠️ EXTREME LEVERAGE: {effectiveLeverage.toFixed(1)}x effective
                <br />
                You will be liquidated on a {liquidationBuffer.toFixed(2)}% adverse move
              </>) : warningLevel === 'danger' ? (<>
                ⚠️ HIGH LEVERAGE: {effectiveLeverage.toFixed(1)}x effective
                <br />
                Liquidation on {liquidationBuffer.toFixed(2)}% move. Use with caution.
              </>) : (<>
                Moderate leverage. Liquidation buffer: {liquidationBuffer.toFixed(2)}%
              </>)}
          </WarningMessage>)}
      </framer_motion_1.AnimatePresence>
    </SliderContainer>);
};
exports.LeverageSlider = LeverageSlider;
