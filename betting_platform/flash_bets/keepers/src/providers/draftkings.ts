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

interface DKContest {
    contest_id: number;
    sport: string;
    name: string;
    entry_fee: number;
    maximum_entries: number;
    total_entries: number;
    starts_at: string;
    draft_groups: DKDraftGroup[];
}

interface DKDraftGroup {
    draft_group_id: number;
    sport: string;
    players: DKPlayer[];
    points_required: number;
}

interface DKPlayer {
    player_id: number;
    name: string;
    salary: number;
    projected_points: number;
    position: string;
}

export class DraftKingsAdapter extends ProviderAdapter {
    private client: AxiosInstance;
    private circuitOpen: boolean = false;
    private failureCount: number = 0;
    private lastFailure: number = 0;
    private authToken: string | null = null;
    private tokenExpiry: number = 0;
    
    constructor() {
        super('DraftKings', process.env.DRAFTKINGS_BASE_URL || 'https://api.draftkings.com');
        
        this.client = axios.create({
            baseURL: this.baseUrl,
            timeout: 5000,
            headers: {
                'Accept': 'application/json',
                'User-Agent': 'FlashBets/1.0',
                'X-API-Key': process.env.DRAFTKINGS_API_KEY || '',
                'Authorization': `Bearer ${this.getAuthToken()}`
            }
        });
        
        // Set rate limit for DraftKings (60/min based on research)
        this.rateLimit = { requests: 60, window: 60000 };
    }
    
    async getLiveOdds(gameId: string): Promise<LiveOdds> {
        return this.withCircuitBreaker(async () => {
            const response = await backOff(
                () => this.client.get(`/v1/contests/${gameId}/live`),
                {
                    numOfAttempts: 5,
                    startingDelay: 100,
                    maxDelay: 10000,
                    jitter: 'full'
                }
            );
            
            const contest = response.data as DKContest;
            return this.contestToOdds(contest);
        });
    }
    
    async getEvents(sport: string, live: boolean = false): Promise<SportEvent[]> {
        return this.withCircuitBreaker(async () => {
            const response = await this.client.get('/v1/contests', {
                params: { sport, live }
            });
            
            const contests = response.data.contests as DKContest[];
            return contests.map(c => this.contestToEvent(c));
        });
    }
    
    async placeBet(bet: BetRequest): Promise<BetResponse> {
        // DraftKings doesn't have direct betting API - this would integrate
        // with their contest entry system or partner API
        return this.withCircuitBreaker(async () => {
            // In production, this would:
            // 1. Create lineup entry based on bet
            // 2. Submit to contest
            // 3. Return confirmation
            
            // Mock implementation for now
            const response = await this.client.post('/v1/entries', {
                contest_id: bet.gameId,
                lineup: this.createLineupFromBet(bet),
                entry_fee: bet.amount
            });
            
            return {
                betId: response.data.entry_id,
                status: 'accepted',
                finalOdds: bet.odds,
                potentialPayout: bet.amount * bet.odds
            };
        });
    }
    
    async getResolution(gameId: string): Promise<Resolution> {
        return this.withCircuitBreaker(async () => {
            const response = await this.client.get(`/v1/contests/${gameId}/results`);
            
            return {
                gameId,
                outcome: response.data.winning_lineup?.outcome || 'unknown',
                timestamp: Date.now(),
                verified: true
            };
        });
    }
    
    private contestToOdds(contest: DKContest): LiveOdds {
        // Calculate implied probability from contest data
        const avgSalary = 50000; // DraftKings typical salary cap
        const totalProjected = contest.draft_groups.reduce(
            (sum, group) => sum + group.players.reduce(
                (playerSum, player) => playerSum + player.projected_points,
                0
            ),
            0
        );
        
        const probability = Math.min(
            totalProjected / (contest.draft_groups.length * 100),
            0.99
        );
        
        return {
            probability,
            timestamp: Date.now(),
            volume: contest.total_entries * contest.entry_fee,
            liquidity: (contest.maximum_entries - contest.total_entries) * contest.entry_fee
        };
    }
    
    private contestToEvent(contest: DKContest): SportEvent {
        const outcomes = this.extractOutcomes(contest);
        const startTime = new Date(contest.starts_at);
        const now = new Date();
        const timeRemaining = Math.max(0, (startTime.getTime() - now.getTime()) / 1000);
        
        return {
            id: contest.contest_id.toString(),
            sport: contest.sport,
            title: contest.name,
            startTime,
            timeRemaining: timeRemaining > 0 ? timeRemaining : undefined,
            outcomes,
            status: timeRemaining > 0 ? 'pre_game' : 'live'
        };
    }
    
    private extractOutcomes(contest: DKContest): Outcome[] {
        // Convert player props to betting outcomes
        const outcomes: Outcome[] = [];
        
        for (const group of contest.draft_groups) {
            for (const player of group.players) {
                const probability = player.projected_points / group.points_required / 2;
                outcomes.push({
                    name: `${player.name} scores ${group.points_required}+`,
                    probability: Math.min(Math.max(probability, 0.01), 0.99),
                    odds: 1 / probability
                });
            }
        }
        
        // Add binary outcomes if applicable
        if (outcomes.length === 0) {
            outcomes.push(
                { name: 'Yes', probability: 0.5, odds: 2.0 },
                { name: 'No', probability: 0.5, odds: 2.0 }
            );
        }
        
        return outcomes;
    }
    
    private createLineupFromBet(bet: BetRequest): any {
        // Convert bet to DraftKings lineup format
        // This would be implemented based on actual DraftKings API requirements
        return {
            players: [],
            outcome: bet.outcome,
            amount: bet.amount
        };
    }
    
    private async withCircuitBreaker<T>(fn: () => Promise<T>): Promise<T> {
        // Check circuit breaker
        if (this.circuitOpen && Date.now() - this.lastFailure < 60000) {
            throw new Error('Circuit breaker open - DraftKings unavailable');
        }
        
        try {
            const result = await fn();
            this.failureCount = 0;
            this.circuitOpen = false;
            return result;
        } catch (error) {
            this.failureCount++;
            this.lastFailure = Date.now();
            
            if (this.failureCount >= 5) {
                this.circuitOpen = true;
            }
            
            throw error;
        }
    }
    
    private async getAuthToken(): Promise<string> {
        // Check if token is still valid
        if (this.authToken && Date.now() < this.tokenExpiry) {
            return this.authToken;
        }
        
        // Request new token
        try {
            const response = await axios.post(
                `${this.baseUrl}/auth/token`,
                {
                    api_key: process.env.DRAFTKINGS_API_KEY,
                    api_secret: process.env.DRAFTKINGS_API_SECRET
                }
            );
            
            this.authToken = response.data.access_token;
            this.tokenExpiry = Date.now() + (response.data.expires_in * 1000);
            
            return this.authToken;
        } catch (error) {
            console.error('Failed to get DraftKings auth token:', error);
            return '';
        }
    }
}