"use strict";
var __awaiter = (this && this.__awaiter) || function (thisArg, _arguments, P, generator) {
    function adopt(value) { return value instanceof P ? value : new P(function (resolve) { resolve(value); }); }
    return new (P || (P = Promise))(function (resolve, reject) {
        function fulfilled(value) { try { step(generator.next(value)); } catch (e) { reject(e); } }
        function rejected(value) { try { step(generator["throw"](value)); } catch (e) { reject(e); } }
        function step(result) { result.done ? resolve(result.value) : adopt(result.value).then(fulfilled, rejected); }
        step((generator = generator.apply(thisArg, _arguments || [])).next());
    });
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.useMarkets = useMarkets;
const react_1 = require("react");
const api_1 = require("../config/api");
function useMarkets() {
    const [markets, setMarkets] = (0, react_1.useState)([]);
    const [loading, setLoading] = (0, react_1.useState)(true);
    const [error, setError] = (0, react_1.useState)(null);
    (0, react_1.useEffect)(() => {
        fetchMarkets();
    }, []);
    const fetchMarkets = () => __awaiter(this, void 0, void 0, function* () {
        try {
            setLoading(true);
            const response = yield (0, api_1.apiCall)(api_1.API_CONFIG.endpoints.markets);
            // Transform API response to UI Market type
            const transformedMarkets = response.markets.map(market => {
                var _a, _b;
                // Calculate price from outcomes (simplified - in real app would be more complex)
                const totalStake = market.outcomes.reduce((sum, o) => sum + o.total_stake, 0);
                const yesStake = ((_a = market.outcomes.find(o => o.name.toLowerCase() === 'yes')) === null || _a === void 0 ? void 0 : _a.total_stake) ||
                    ((_b = market.outcomes[0]) === null || _b === void 0 ? void 0 : _b.total_stake) || 0;
                const lastPrice = totalStake > 0 ? yesStake / totalStake : 0.5;
                // Calculate 24h change (mock for now - would come from API)
                const change24h = (Math.random() - 0.5) * 10;
                return {
                    id: market.id.toString(),
                    name: market.title,
                    verseId: market.verse_id.toString(),
                    lastPrice,
                    volume24h: market.total_volume,
                    liquidity: market.total_liquidity,
                    change24h,
                    volatility: 0.1 + Math.random() * 0.2, // Mock volatility
                    resolutionTime: market.resolution_time * 1000, // Convert to milliseconds
                };
            });
            setMarkets(transformedMarkets);
            setError(null);
        }
        catch (err) {
            console.error('Failed to fetch markets:', err);
            setError(err instanceof Error ? err.message : 'Failed to fetch markets');
        }
        finally {
            setLoading(false);
        }
    });
    return {
        markets,
        loading,
        error,
        refetch: fetchMarkets,
    };
}
