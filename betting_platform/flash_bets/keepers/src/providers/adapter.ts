export interface LiveOdds {
    probability: number;
    timestamp: number;
    volume?: number;
    liquidity?: number;
}

export interface BetRequest {
    gameId: string;
    outcome: string;
    amount: number;
    odds: number;
}

export interface BetResponse {
    betId: string;
    status: 'pending' | 'accepted' | 'rejected';
    finalOdds?: number;
    potentialPayout?: number;
}

export interface Resolution {
    gameId: string;
    outcome: string;
    timestamp: number;
    verified: boolean;
}

export interface SportEvent {
    id: string;
    sport: string;
    title: string;
    startTime: Date;
    timeRemaining?: number;
    outcomes: Outcome[];
    status: 'pre_game' | 'live' | 'finished';
}

export interface Outcome {
    name: string;
    probability: number;
    odds: number;
    volume?: number;
}

export abstract class ProviderAdapter {
    protected name: string;
    protected baseUrl: string;
    protected rateLimit: { requests: number; window: number };
    
    constructor(name: string, baseUrl: string) {
        this.name = name;
        this.baseUrl = baseUrl;
        this.rateLimit = { requests: 60, window: 60000 }; // Default 60/min
    }
    
    abstract async getLiveOdds(gameId: string): Promise<LiveOdds>;
    abstract async getEvents(sport: string, live: boolean): Promise<SportEvent[]>;
    abstract async placeBet(bet: BetRequest): Promise<BetResponse>;
    abstract async getResolution(gameId: string): Promise<Resolution>;
    
    // Universal ID generation
    generateUniversalId(eventId: string, marketId: string): string {
        const timestamp = Math.floor(Date.now() / 1000);
        return `${this.name.toUpperCase()}:${eventId}:${marketId}:${timestamp}`;
    }
    
    // Normalize odds to probability
    normalizeOdds(value: any, format: 'american' | 'decimal' | 'fractional'): number {
        switch (format) {
            case 'american':
                if (value > 0) {
                    return 100 / (value + 100);
                } else {
                    return Math.abs(value) / (Math.abs(value) + 100);
                }
            case 'decimal':
                return 1 / value;
            case 'fractional':
                const [numerator, denominator] = value.split('/').map(Number);
                return denominator / (numerator + denominator);
            default:
                return value; // Already probability
        }
    }
}