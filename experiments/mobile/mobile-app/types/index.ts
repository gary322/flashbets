export interface Market {
  id: string;
  name: string;
  price?: number;
  change24h: number;
  volume24h: number;
  liquidity: number;
  resolutionTime: number;
}

export interface SwipeAction {
  type: 'buy' | 'sell';
  marketId: string;
  timestamp: number;
}

export interface GestureState {
  dx: number;
  dy: number;
  vx: number;
  vy: number;
  numberActiveTouches: number;
}