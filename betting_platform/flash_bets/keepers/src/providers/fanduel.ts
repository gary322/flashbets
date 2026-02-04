import axios, { AxiosInstance } from 'axios';
import { backOff } from 'exponential-backoff';
import { 
    ProviderAdapter, 
    LiveOdds, 
    SportEvent, 
    BetRequest, 
    BetResponse, 
    Resolution,
    Outcome 
} from './adapter';

export class FanDuelAdapter extends ProviderAdapter {
    private client: AxiosInstance;
    
    constructor() {
        super('FanDuel', 'https://api.fanduel.com');
        
        this.client = axios.create({
            baseURL: this.baseUrl,
            timeout: 5000,
            headers: {
                'Accept': 'application/json',
                'User-Agent': 'FlashBets/1.0'
            }
        });
        
        // FanDuel rate limit: 500 requests per 10 seconds
        this.rateLimit = { requests: 50, window: 1000 }; // Safer: 50/s
    }
    
    async getLiveOdds(gameId: string): Promise<LiveOdds> {
        const response = await backOff(
            () => this.client.get(`/fixtures/${gameId}/live`),
            { numOfAttempts: 3, startingDelay: 100 }
        );
        
        const odds = response.data;
        return {
            probability: this.normalizeOdds(odds.american_odds, 'american'),
            timestamp: Date.now(),
            volume: odds.volume || 0,
            liquidity: odds.liquidity || 0
        };
    }
    
    async getEvents(sport: string, live: boolean = false): Promise<SportEvent[]> {
        const response = await this.client.get('/fixtures', {
            params: { sport, live, limit: 50 }
        });
        
        return response.data.fixtures.map((fixture: any) => ({
            id: fixture.fixture_id,
            sport: fixture.sport,
            title: fixture.title,
            startTime: new Date(fixture.start_time),
            timeRemaining: fixture.time_remaining,
            outcomes: this.extractOutcomes(fixture),
            status: fixture.status
        }));
    }
    
    async placeBet(bet: BetRequest): Promise<BetResponse> {
        const response = await this.client.post('/bets', {
            fixture_id: bet.gameId,
            selection: bet.outcome,
            stake: bet.amount,
            odds: bet.odds
        });
        
        return {
            betId: response.data.bet_id,
            status: response.data.status,
            finalOdds: response.data.final_odds,
            potentialPayout: response.data.potential_payout
        };
    }
    
    async getResolution(gameId: string): Promise<Resolution> {
        const response = await this.client.get(`/fixtures/${gameId}/result`);
        
        return {
            gameId,
            outcome: response.data.winning_selection,
            timestamp: Date.now(),
            verified: response.data.verified || false
        };
    }
    
    private extractOutcomes(fixture: any): Outcome[] {
        return fixture.markets?.[0]?.selections?.map((sel: any) => ({
            name: sel.name,
            probability: this.normalizeOdds(sel.odds, 'american'),
            odds: sel.odds
        })) || [];
    }
}