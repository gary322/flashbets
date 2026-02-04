export interface Market {
  id: string;
  name: string;
  verseId?: string;
  lastPrice: number;
  volume24h: number;
  liquidity: number;
  change24h: number;
  volatility: number;
  resolutionTime: number;
}

export interface ChainStep {
  id: string;
  type: 'borrow' | 'liquidity' | 'hedge' | 'arbitrage';
  multiplier: number;
  params: Record<string, any>;
}

export interface Position {
  id: string;
  marketId: string;
  side: 'long' | 'short';
  size: number;
  entryPrice: number;
  leverage: number;
  effectiveLeverage: number;
  unrealizedPnL: number;
  margin: number;
  liquidationPrice: number;
  timestamp: number;
}

export interface PriceUpdate {
  marketId: string;
  price: number;
  timestamp: number;
  volume24h: number;
  changePercent: number;
}