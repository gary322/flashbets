"use strict";
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.MarketSelector = void 0;
const react_1 = __importStar(require("react"));
const styled_1 = __importDefault(require("@emotion/styled"));
const framer_motion_1 = require("framer-motion");
const Container = styled_1.default.div `
  height: 100%;
  display: flex;
  flex-direction: column;
`;
const SearchInput = styled_1.default.input `
  background: ${props => props.theme.colors.background.primary};
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 8px;
  padding: 12px 16px;
  color: ${props => props.theme.colors.text.primary};
  font-size: 14px;
  margin-bottom: 16px;
  transition: all 200ms ease;

  &:focus {
    outline: none;
    border-color: ${props => props.theme.colors.accent.primary};
  }

  &::placeholder {
    color: ${props => props.theme.colors.text.tertiary};
  }
`;
const MarketList = styled_1.default.div `
  flex: 1;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 8px;
`;
const MarketItem = (0, styled_1.default)(framer_motion_1.motion.div) `
  padding: 16px;
  cursor: pointer;
  background: ${props => props.selected ?
    props.theme.colors.background.tertiary :
    'transparent'};
  border: 1px solid ${props => props.selected ?
    props.theme.colors.accent.primary :
    'transparent'};
  border-radius: 8px;
  transition: all 200ms ease;

  &:hover {
    background: ${props => props.theme.colors.background.tertiary};
  }
`;
const MarketName = styled_1.default.div `
  font-size: 14px;
  font-weight: 600;
  color: ${props => props.theme.colors.text.primary};
  margin-bottom: 4px;
`;
const MarketPrice = styled_1.default.div `
  display: flex;
  justify-content: space-between;
  align-items: center;
`;
const Price = styled_1.default.span `
  font-family: ${props => props.theme.typography.fonts.mono};
  font-size: 16px;
  font-weight: 700;
`;
const PriceChange = styled_1.default.span `
  font-size: 12px;
  color: ${props => props.positive ?
    props.theme.colors.accent.primary :
    props.theme.colors.accent.secondary};
`;
const MarketSelector = ({ markets, selectedMarket, onSelect, prices, searchPlaceholder = "Search markets..." }) => {
    const [search, setSearch] = (0, react_1.useState)('');
    const filteredMarkets = (0, react_1.useMemo)(() => {
        const searchLower = search.toLowerCase();
        return markets.filter(market => market.name.toLowerCase().includes(searchLower));
    }, [markets, search]);
    const formatPrice = (price) => {
        return `${(price * 100).toFixed(1)}%`;
    };
    const formatChange = (change) => {
        const sign = change >= 0 ? '+' : '';
        return `${sign}${change.toFixed(2)}%`;
    };
    return (<Container>
      <SearchInput type="text" value={search} onChange={(e) => setSearch(e.target.value)} placeholder={searchPlaceholder}/>
      
      <MarketList>
        {filteredMarkets.map(market => {
            const priceData = prices.get(market.id);
            const currentPrice = (priceData === null || priceData === void 0 ? void 0 : priceData.price) || market.lastPrice;
            const change = (priceData === null || priceData === void 0 ? void 0 : priceData.changePercent) || market.change24h;
            return (<MarketItem key={market.id} selected={(selectedMarket === null || selectedMarket === void 0 ? void 0 : selectedMarket.id) === market.id} onClick={() => onSelect(market)} whileHover={{ scale: 1.02 }} whileTap={{ scale: 0.98 }}>
              <MarketName>{market.name}</MarketName>
              <MarketPrice>
                <Price>{formatPrice(currentPrice)}</Price>
                <PriceChange positive={change >= 0}>
                  {formatChange(change)}
                </PriceChange>
              </MarketPrice>
            </MarketItem>);
        })}
      </MarketList>
    </Container>);
};
exports.MarketSelector = MarketSelector;
