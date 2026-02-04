import { designTokens, componentTokens } from './tokens';

export interface Theme {
  colors: typeof designTokens.colors;
  typography: typeof designTokens.typography;
  spacing: typeof designTokens.spacing;
  animation: typeof designTokens.animation;
  breakpoints: typeof designTokens.breakpoints;
  components: typeof componentTokens;
}

export const theme: Theme = {
  colors: designTokens.colors,
  typography: designTokens.typography,
  spacing: designTokens.spacing,
  animation: designTokens.animation,
  breakpoints: designTokens.breakpoints,
  components: componentTokens
};

// Type augmentation for emotion theme
declare module '@emotion/react' {
  export interface Theme {
    colors: typeof designTokens.colors;
    typography: typeof designTokens.typography;
    spacing: typeof designTokens.spacing;
    animation: typeof designTokens.animation;
    breakpoints: typeof designTokens.breakpoints;
    components: typeof componentTokens;
  }
}