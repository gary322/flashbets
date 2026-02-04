"use strict";
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.BlurCard = void 0;
const react_1 = __importDefault(require("react"));
const framer_motion_1 = require("framer-motion");
const styled_1 = __importDefault(require("@emotion/styled"));
const StyledCard = (0, styled_1.default)(framer_motion_1.motion.div) `
  background: ${props => props.danger ?
    'rgba(239, 68, 68, 0.1)' :
    props.theme.colors.background.secondary};
  border: 1px solid ${props => props.danger ?
    'rgba(239, 68, 68, 0.3)' :
    'rgba(255, 255, 255, 0.05)'};
  border-radius: ${props => props.theme.components.marketCard.borderRadius};
  padding: ${props => props.theme.spacing.lg};
  backdrop-filter: blur(12px);
  
  ${props => props.interactive && `
    cursor: pointer;
    transition: all ${props.theme.animation.durations.fast} ${props.theme.animation.easings.default};
    
    &:hover {
      border-color: rgba(255, 255, 255, 0.1);
      transform: translateY(-2px);
      box-shadow: ${props.theme.components.marketCard.shadowElevation.hover};
    }
    
    &:active {
      transform: translateY(0);
      box-shadow: ${props.theme.components.marketCard.shadowElevation.active};
    }
  `}
`;
const BlurCard = ({ children, elevation = 'medium', interactive = false, danger = false }) => {
    return (<StyledCard elevation={elevation} interactive={interactive} danger={danger} whileHover={interactive ? { scale: 1.01 } : undefined} whileTap={interactive ? { scale: 0.99 } : undefined}>
      {children}
    </StyledCard>);
};
exports.BlurCard = BlurCard;
