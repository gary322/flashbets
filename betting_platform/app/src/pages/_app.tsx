import type { AppProps } from 'next/app';
import { ThemeProvider } from '@emotion/react';
import { Global, css } from '@emotion/react';
import { theme } from '../ui/theme/theme';
import { Layout } from '../components/layout/Layout';
import { MetaMaskProvider } from '../hooks/useMetaMask';

const globalStyles = css`
  * {
    box-sizing: border-box;
    margin: 0;
    padding: 0;
  }

  html,
  body {
    font-family: ${theme.typography.fonts.sans};
    background: ${theme.colors.background.primary};
    color: ${theme.colors.text.primary};
    font-size: ${theme.typography.sizes.base};
    line-height: 1.5;
    -webkit-font-smoothing: antialiased;
    -moz-osx-font-smoothing: grayscale;
  }

  a {
    color: inherit;
    text-decoration: none;
  }

  button {
    font-family: inherit;
  }

  /* Custom scrollbar */
  ::-webkit-scrollbar {
    width: 8px;
    height: 8px;
  }

  ::-webkit-scrollbar-track {
    background: ${theme.colors.background.primary};
  }

  ::-webkit-scrollbar-thumb {
    background: ${theme.colors.background.tertiary};
    border-radius: 4px;
  }

  ::-webkit-scrollbar-thumb:hover {
    background: ${theme.colors.text.tertiary};
  }
`;

export default function App({ Component, pageProps }: AppProps) {
  return (
    <MetaMaskProvider>
      <ThemeProvider theme={theme}>
        <Global styles={globalStyles} />
        <Layout>
          <Component {...pageProps} />
        </Layout>
      </ThemeProvider>
    </MetaMaskProvider>
  );
}