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

export class BetMGMAdapter extends ProviderAdapter {
    private client: AxiosInstance;
    
    constructor() {
        super('BetMGM', 'https://api.betmgm.com');
        
        this.client = axios.create({
            baseURL: this.baseUrl,
            timeout: 5000,
            headers: {
                'Accept': 'application/json',
                'User-Agent': 'FlashBets/1.0'
            }
        });
        
        // BetMGM rate limit: 100/s
        this.rateLimit = { requests: 100, window: 1000 };
    }
    
    async getLiveOdds(gameId: string): Promise<LiveOdds> {
        const response = await backOff(
            () => this.client.get(`/events/${gameId}/live`),
            { numOfAttempts: 3, startingDelay: 100 }
        );
        
        const event = response.data;
        return {
            probability: this.normalizeOdds(event.odds, 'decimal'),
            timestamp: Date.now(),
            volume: event.matched || 0,
            liquidity: event.available || 0
        };
    }
    
    async getEvents(sport: string, live: boolean = false): Promise<SportEvent[]> {
        const response = await this.client.get('/events', {
            params: { 
                sport, 
                in_play: live,
                limit: 100 
            }
        });
        
        return response.data.events.map((event: any) => ({
            id: event.event_id,
            sport: event.sport_name,
            title: event.name,
            startTime: new Date(event.start_time),
            timeRemaining: event.seconds_to_start,
            outcomes: this.extractOutcomes(event),
            status: event.in_play ? 'live' : 'pre_game'
        }));
    }
    
    async placeBet(bet: BetRequest): Promise<BetResponse> {
        const response = await this.client.post('/bets/place', {
            event_id: bet.gameId,
            selection_id: bet.outcome,
            stake: bet.amount,
            price: bet.odds
        });
        
        return {
            betId: response.data.bet_reference,
            status: response.data.bet_status,
            finalOdds: response.data.price_taken,
            potentialPayout: response.data.potential_return
        };
    }
    
    async getResolution(gameId: string): Promise<Resolution> {
        const response = await this.client.get(`/events/${gameId}/results`);
        
        return {
            gameId,
            outcome: response.data.winning_selection_name,
            timestamp: Date.now(),
            verified: response.data.official || false
        };
    }
    
    private extractOutcomes(event: any): Outcome[] {
        return event.markets?.[0]?.runners?.map((runner: any) => ({
            name: runner.name,
            probability: this.normalizeOdds(runner.price, 'decimal'),
            odds: runner.price
        })) || [];
    }
}