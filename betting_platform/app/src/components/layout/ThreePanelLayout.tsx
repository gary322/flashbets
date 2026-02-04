import React from 'react';
import styled from '@emotion/styled';

interface ThreePanelLayoutProps {
  leftPanel: React.ReactNode;
  mainContent: React.ReactNode;
  rightPanel: React.ReactNode;
}

const LayoutContainer = styled.div`
  display: grid;
  grid-template-columns: ${props => props.theme?.components?.panel?.leftWidth || '320px'} 1fr ${props => props.theme?.components?.panel?.rightWidth || '360px'};
  min-height: 100vh;
  gap: ${props => props.theme?.components?.panel?.gap || '0px'};
  background: ${props => props.theme?.colors?.text?.tertiary || '#000'};
  
  @media (max-width: ${props => props.theme?.breakpoints?.wide || '1400px'}) {
    grid-template-columns: 280px 1fr 320px;
  }
  
  @media (max-width: ${props => props.theme?.breakpoints?.desktop || '1024px'}) {
    grid-template-columns: 1fr;
  }
`;

const Panel = styled.div<{ variant?: 'left' | 'main' | 'right' }>`
  background: ${props => 
    props.variant === 'main' 
      ? props.theme?.colors?.background?.tertiary || '#111'
      : props.theme?.colors?.background?.secondary || '#000'
  };
  overflow: hidden;
  display: flex;
  flex-direction: column;
  
  @media (max-width: ${props => props.theme?.breakpoints?.desktop || '1024px'}) {
    display: ${props => props.variant !== 'main' ? 'none' : 'block'};
  }
`;

const MobileToggle = styled.button<{ position: 'left' | 'right' }>`
  display: none;
  position: fixed;
  ${props => props.position}: 16px;
  bottom: 16px;
  width: 56px;
  height: 56px;
  border-radius: 50%;
  background: ${props => props.theme?.colors?.accent?.primary || '#ffd60a'};
  border: none;
  color: ${props => props.theme?.colors?.text?.inverse || '#000'};
  font-size: 24px;
  cursor: pointer;
  z-index: 100;
  box-shadow: 0 4px 12px rgba(255, 214, 10, 0.3);
  transition: all ${props => props.theme?.animation?.durations?.fast || '200ms'} ${props => props.theme?.animation?.easings?.default || 'ease'};
  
  &:hover {
    transform: scale(1.05);
    box-shadow: 0 6px 20px rgba(255, 214, 10, 0.4);
  }
  
  &:active {
    transform: scale(0.95);
  }
  
  @media (max-width: ${props => props.theme?.breakpoints?.desktop || '1024px'}) {
    display: flex;
    align-items: center;
    justify-content: center;
  }
`;

const MobilePanel = styled.div<{ isOpen: boolean; position: 'left' | 'right' }>`
  display: none;
  position: fixed;
  top: 0;
  ${props => props.position}: 0;
  width: ${props => props.position === 'left' ? '320px' : '360px'};
  height: 100vh;
  background: ${props => props.theme?.colors?.background?.secondary || '#000'};
  z-index: 200;
  transform: translateX(${props => 
    props.isOpen 
      ? '0' 
      : props.position === 'left' 
        ? '-100%' 
        : '100%'
  });
  transition: transform ${props => props.theme?.animation?.durations?.normal || '300ms'} ${props => props.theme?.animation?.easings?.smooth || 'ease-in-out'};
  box-shadow: ${props => 
    props.isOpen 
      ? props.position === 'left'
        ? '4px 0 24px rgba(0, 0, 0, 0.8)'
        : '-4px 0 24px rgba(0, 0, 0, 0.8)'
      : 'none'
  };
  
  @media (max-width: ${props => props.theme?.breakpoints?.desktop || '1024px'}) {
    display: block;
  }
  
  @media (max-width: ${props => props.theme?.breakpoints?.mobile || '768px'}) {
    width: 100%;
  }
`;

const MobileOverlay = styled.div<{ isOpen: boolean }>`
  display: none;
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.6);
  backdrop-filter: blur(10px);
  z-index: 150;
  opacity: ${props => props.isOpen ? 1 : 0};
  pointer-events: ${props => props.isOpen ? 'auto' : 'none'};
  transition: opacity ${props => props.theme?.animation?.durations?.normal || '300ms'} ${props => props.theme?.animation?.easings?.default || 'ease'};
  
  @media (max-width: ${props => props.theme?.breakpoints?.desktop || '1024px'}) {
    display: block;
  }
`;

export default function ThreePanelLayout({ 
  leftPanel, 
  mainContent, 
  rightPanel 
}: ThreePanelLayoutProps) {
  const [leftPanelOpen, setLeftPanelOpen] = React.useState(false);
  const [rightPanelOpen, setRightPanelOpen] = React.useState(false);

  const handleOverlayClick = () => {
    setLeftPanelOpen(false);
    setRightPanelOpen(false);
  };

  return (
    <>
      <LayoutContainer>
        <Panel variant="left">
          {leftPanel}
        </Panel>
        
        <Panel variant="main">
          {mainContent}
        </Panel>
        
        <Panel variant="right">
          {rightPanel}
        </Panel>
      </LayoutContainer>

      {/* Mobile UI */}
      <MobileToggle 
        position="left" 
        onClick={() => setLeftPanelOpen(true)}
        aria-label="Open navigation"
      >
        ☰
      </MobileToggle>
      
      <MobileToggle 
        position="right" 
        onClick={() => setRightPanelOpen(true)}
        aria-label="Open trading panel"
      >
        ⚛️
      </MobileToggle>

      <MobileOverlay 
        isOpen={leftPanelOpen || rightPanelOpen} 
        onClick={handleOverlayClick}
      />
      
      <MobilePanel 
        isOpen={leftPanelOpen} 
        position="left"
      >
        {leftPanel}
      </MobilePanel>
      
      <MobilePanel 
        isOpen={rightPanelOpen} 
        position="right"
      >
        {rightPanel}
      </MobilePanel>
    </>
  );
}