import React from 'react';
import styled from '@emotion/styled';
import Link from 'next/link';
import { useRouter } from 'next/router';
import { WalletConnect } from '../wallet/WalletConnect';

const HeaderContainer = styled.header`
  background: ${props => props.theme.colors.background.secondary};
  border-bottom: 1px solid rgba(255, 255, 255, 0.1);
  position: sticky;
  top: 0;
  z-index: 100;
  backdrop-filter: blur(12px);
`;

const HeaderContent = styled.div`
  max-width: 1440px;
  margin: 0 auto;
  padding: 0 24px;
  height: 72px;
  display: flex;
  align-items: center;
  justify-content: space-between;
`;

const Logo = styled.div`
  font-size: 24px;
  font-weight: 800;
  color: ${props => props.theme.colors.text.primary};
  cursor: pointer;
  
  span {
    color: ${props => props.theme.colors.accent.primary};
  }
`;

const Nav = styled.nav`
  display: flex;
  align-items: center;
  gap: 32px;
  
  @media (max-width: 768px) {
    display: none;
  }
`;

const NavLink = styled.span<{ active?: boolean }>`
  font-size: 16px;
  font-weight: 500;
  color: ${props => props.active ? 
    props.theme.colors.accent.primary : 
    props.theme.colors.text.secondary};
  text-decoration: none;
  transition: color 0.2s ease;
  cursor: pointer;
  
  &:hover {
    color: ${props => props.theme.colors.accent.primary};
  }
`;

const RightSection = styled.div`
  display: flex;
  align-items: center;
  gap: 16px;
`;

const MobileMenuButton = styled.button`
  display: none;
  background: none;
  border: none;
  color: ${props => props.theme.colors.text.primary};
  font-size: 24px;
  cursor: pointer;
  
  @media (max-width: 768px) {
    display: block;
  }
`;

export const Header: React.FC = () => {
  const router = useRouter();
  
  const navItems = [
    { label: 'Markets', href: '/markets', testId: 'nav-markets' },
    { label: 'Trade', href: '/trade', testId: 'nav-trade' },
    { label: 'Portfolio', href: '/portfolio', testId: 'nav-portfolio' },
    { label: 'Leaderboard', href: '/leaderboard', testId: 'nav-leaderboard' },
  ];
  
  return (
    <HeaderContainer>
      <HeaderContent>
        <Link href="/" passHref legacyBehavior>
          <a style={{ textDecoration: 'none' }}>
            <Logo>
              Betting<span>Platform</span>
            </Logo>
          </a>
        </Link>
        
        <Nav data-testid="navigation">
          {navItems.map(item => (
            <Link key={item.href} href={item.href} passHref legacyBehavior>
              <a style={{ textDecoration: 'none' }}>
                <NavLink 
                  active={router.pathname === item.href}
                  data-testid={item.testId}
                >
                  {item.label}
                </NavLink>
              </a>
            </Link>
          ))}
        </Nav>
        
        <RightSection>
          <WalletConnect />
          <MobileMenuButton aria-label="Toggle menu">
            ☰
          </MobileMenuButton>
        </RightSection>
      </HeaderContent>
    </HeaderContainer>
  );
};