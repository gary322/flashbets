export interface Market {
    id: string;
    question: string;
    outcomes: string[];
    volume: number;
    liquidity: number;
    yes_price: number;
    last_price: number;
    resolved: boolean;
    resolution: string | null;
    created_at: string;
    updated_at: string;
}

export interface PriceUpdate {
    marketId: string;
    yesPrice: number;
    timestamp: number;
}

export interface CachedPrice {
    price: number;
    timestamp: number;
}

export interface Resolution {
    marketId: string;
    resolution: string;
    timestamp: number;
}

export interface Request {
    execute: () => Promise<any>;
    resolve: (value: any) => void;
    reject: (reason?: any) => void;
    priority: number;
    endpoint: string;
    timestamp: number;
}

export interface BatchRequest {
    params: any;
    resolve: (value: any) => void;
    reject: (reason?: any) => void;
    priority: number;
}

export interface KeeperMetrics {
    processed: number;
    errors: number;
    queueDepth: number;
}

export interface HealthReport {
    timestamp: Date;
    totalKeepers: number;
    healthyKeepers: number;
    totalMarketsTracked: number;
    averageLatency: number;
    errorRate: number;
    workDistribution: Record<string, number>;
    alerts: Alert[];
}

export interface Alert {
    severity: 'high' | 'medium' | 'low';
    message: string;
    details?: any;
    keepers?: string[];
    recommendation?: string;
}

export interface RequestLogEntry {
    timestamp: number;
    endpoint: string;
    duration: number;
    status: number;
}