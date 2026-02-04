import React from 'react';
import { motion } from 'framer-motion';
import styled from '@emotion/styled';

interface BlurCardProps {
  elevation?: 'low' | 'medium' | 'high';
  interactive?: boolean;
  danger?: boolean;
  children: React.ReactNode;
}

const StyledCard = styled(motion.div)<BlurCardProps>`
  background: ${props => props.danger ? 
    'rgba(239, 68, 68, 0.1)' : 
    props.theme?.colors?.background?.secondary || 'rgba(255, 255, 255, 0.05)'};
  border: 1px solid ${props => props.danger ?
    'rgba(239, 68, 68, 0.3)' :
    'rgba(255, 255, 255, 0.05)'};
  border-radius: ${props => props.theme?.components?.marketCard?.borderRadius || '12px'};
  padding: ${props => props.theme?.spacing?.lg || '24px'};
  backdrop-filter: blur(12px);
  
  ${props => props.interactive && `
    cursor: pointer;
    transition: all ${props?.theme?.animation?.durations?.fast || '150ms'} ${props?.theme?.animation?.easings?.default || 'ease'};
    
    &:hover {
      border-color: rgba(255, 255, 255, 0.1);
      transform: translateY(-2px);
      box-shadow: ${props?.theme?.components?.marketCard?.shadowElevation?.hover || '0 8px 32px rgba(0, 0, 0, 0.3)'};
    }
    
    &:active {
      transform: translateY(0);
      box-shadow: ${props?.theme?.components?.marketCard?.shadowElevation?.active || '0 4px 16px rgba(0, 0, 0, 0.2)'};
    }
  `}
`;

export const BlurCard: React.FC<BlurCardProps> = ({ 
  children, 
  elevation = 'medium',
  interactive = false,
  danger = false 
}) => {
  return (
    <StyledCard
      elevation={elevation}
      interactive={interactive}
      danger={danger}
      whileHover={interactive ? { scale: 1.01 } : undefined}
      whileTap={interactive ? { scale: 0.99 } : undefined}
    >
      {children}
    </StyledCard>
  );
};