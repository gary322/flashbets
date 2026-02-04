// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

import "@openzeppelin/contracts/access/Ownable.sol";
import "@openzeppelin/contracts/security/ReentrancyGuard.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import "@openzeppelin/contracts/token/ERC1155/IERC1155.sol";

/**
 * @title PolymarketIntegration
 * @notice Integrates with Polymarket's CTF Exchange for market data and trading
 * @dev Interfaces with CTF Exchange at 0x4bFb41d5B3570DeFd03C39a9A4D8dE6Bd8B8982E
 */
contract PolymarketIntegration is Ownable, ReentrancyGuard {
    using SafeERC20 for IERC20;

    // Polymarket CTF Exchange address on Polygon
    address public constant CTF_EXCHANGE = 0x4bFb41d5B3570DeFd03C39a9A4D8dE6Bd8B8982E;
    address public constant CTF_TOKEN = 0x4D97DCd97eC945f40cF65F87097ACe5EA0476045; // Conditional Token Framework
    
    // USDC on Polygon
    address public constant USDC = 0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174;
    
    // Structs
    struct PolymarketOrder {
        address maker;
        address taker;
        uint256 makerAmount;
        uint256 takerAmount;
        uint256 makerNonce;
        uint256 takerNonce;
        uint256 expiration;
        uint256 salt;
        uint256 feeRateBps;
        uint8 side; // 0 = BUY, 1 = SELL
        uint8 signatureType;
        bytes signature;
    }
    
    struct MarketMapping {
        bytes32 conditionId;
        address oracle;
        uint256 questionId;
        uint256[] outcomeTokenIds;
        bool isActive;
        string description;
    }
    
    // State
    mapping(bytes32 => MarketMapping) public marketMappings;
    mapping(bytes32 => uint256) public marketPrices; // Cached prices
    mapping(address => bool) public authorizedCallers;
    
    address public bettingPlatform;
    uint256 public priceUpdateInterval = 60; // seconds
    mapping(bytes32 => uint256) public lastPriceUpdate;
    
    // Events
    event MarketMapped(bytes32 indexed marketId, bytes32 indexed conditionId);
    event OrderPlaced(bytes32 indexed marketId, address indexed trader, uint256 amount, uint8 side);
    event PriceUpdated(bytes32 indexed marketId, uint256 price);
    event MarketSettled(bytes32 indexed marketId, uint256[] payouts);
    
    modifier onlyAuthorized() {
        require(
            msg.sender == owner() || 
            msg.sender == bettingPlatform || 
            authorizedCallers[msg.sender],
            "Not authorized"
        );
        _;
    }
    
    constructor(address _bettingPlatform) {
        bettingPlatform = _bettingPlatform;
    }
    
    // ============ Market Management ============
    
    /**
     * @notice Maps an internal market ID to a Polymarket condition
     * @param marketId Internal market identifier
     * @param conditionId Polymarket condition ID
     * @param oracle Oracle address for the condition
     * @param questionId Question ID for resolution
     * @param outcomeTokenIds Token IDs for outcomes
     */
    function mapMarket(
        bytes32 marketId,
        bytes32 conditionId,
        address oracle,
        uint256 questionId,
        uint256[] calldata outcomeTokenIds,
        string calldata description
    ) external onlyOwner {
        require(marketMappings[marketId].conditionId == bytes32(0), "Market already mapped");
        
        marketMappings[marketId] = MarketMapping({
            conditionId: conditionId,
            oracle: oracle,
            questionId: questionId,
            outcomeTokenIds: outcomeTokenIds,
            isActive: true,
            description: description
        });
        
        emit MarketMapped(marketId, conditionId);
    }
    
    // ============ Price Discovery ============
    
    /**
     * @notice Gets the current market price from Polymarket
     * @param marketId Internal market identifier
     * @return Current price in basis points (0-10000)
     */
    function getMarketPrice(bytes32 marketId) external view returns (uint256) {
        MarketMapping memory marketMapping = marketMappings[marketId];
        require(marketMapping.isActive, "Market not active");
        
        // Return cached price if recent
        if (block.timestamp - lastPriceUpdate[marketId] < priceUpdateInterval) {
            return marketPrices[marketId];
        }
        
        // In production, this would query Polymarket's orderbook
        // For now, return the cached price
        return marketPrices[marketId];
    }
    
    /**
     * @notice Updates market price from Polymarket orderbook
     * @param marketId Internal market identifier
     */
    function updateMarketPrice(bytes32 marketId) external onlyAuthorized {
        MarketMapping memory marketMapping = marketMappings[marketId];
        require(marketMapping.isActive, "Market not active");
        
        // In production, this would:
        // 1. Query Polymarket's GraphQL API for orderbook
        // 2. Calculate mid-price from best bid/ask
        // 3. Store the price
        
        // For demonstration, using a simulated price
        uint256 price = _fetchPolymarketPrice(marketMapping.conditionId);
        
        marketPrices[marketId] = price;
        lastPriceUpdate[marketId] = block.timestamp;
        
        emit PriceUpdated(marketId, price);
    }
    
    // ============ Trading ============
    
    /**
     * @notice Places an order on Polymarket
     * @param marketId Internal market identifier
     * @param amount Amount in USDC
     * @param side 0 for BUY, 1 for SELL
     * @param price Price in basis points
     */
    function placeOrder(
        bytes32 marketId,
        uint256 amount,
        uint8 side,
        uint256 price
    ) external onlyAuthorized nonReentrant {
        MarketMapping memory marketMapping = marketMappings[marketId];
        require(marketMapping.isActive, "Market not active");
        
        // Transfer USDC from caller
        IERC20(USDC).safeTransferFrom(msg.sender, address(this), amount);
        
        // Approve CTF Exchange
        IERC20(USDC).safeApprove(CTF_EXCHANGE, amount);
        
        // Create order for CTF Exchange
        PolymarketOrder memory order = _createOrder(
            marketMapping.conditionId,
            amount,
            price,
            side
        );
        
        // Execute order on CTF Exchange
        _executeOrder(order);
        
        emit OrderPlaced(marketId, msg.sender, amount, side);
    }
    
    /**
     * @notice Claims winnings from a resolved market
     * @param marketId Internal market identifier
     */
    function claimWinnings(bytes32 marketId) external nonReentrant {
        MarketMapping memory marketMapping = marketMappings[marketId];
        require(!marketMapping.isActive, "Market still active");
        
        // Get outcome tokens balance
        uint256 balance = _getOutcomeTokenBalance(marketMapping.conditionId, msg.sender);
        
        if (balance > 0) {
            // Redeem tokens for USDC through CTF
            _redeemOutcomeTokens(marketMapping.conditionId, balance);
            
            // Transfer USDC to user
            uint256 usdcBalance = IERC20(USDC).balanceOf(address(this));
            IERC20(USDC).safeTransfer(msg.sender, usdcBalance);
        }
    }
    
    // ============ Settlement ============
    
    /**
     * @notice Settles a market based on Polymarket resolution
     * @param marketId Internal market identifier
     */
    function settleMarket(bytes32 marketId) external onlyAuthorized {
        MarketMapping storage marketMapping = marketMappings[marketId];
        require(marketMapping.isActive, "Market not active");
        
        // Get resolution from Polymarket oracle
        uint256[] memory payouts = _getResolution(marketMapping.conditionId, marketMapping.oracle);
        
        // Mark as settled
        marketMapping.isActive = false;
        
        // Notify betting platform
        IBettingPlatform(bettingPlatform).resolveMarket(marketId, _calculateSettlementPrice(payouts));
        
        emit MarketSettled(marketId, payouts);
    }
    
    // ============ Internal Functions ============
    
    function _createOrder(
        bytes32 conditionId,
        uint256 amount,
        uint256 price,
        uint8 side
    ) internal view returns (PolymarketOrder memory) {
        return PolymarketOrder({
            maker: address(this),
            taker: address(0),
            makerAmount: side == 0 ? amount : amount * price / 10000,
            takerAmount: side == 0 ? amount * price / 10000 : amount,
            makerNonce: block.timestamp,
            takerNonce: 0,
            expiration: block.timestamp + 3600, // 1 hour
            salt: uint256(keccak256(abi.encodePacked(block.timestamp, conditionId))),
            feeRateBps: 0,
            side: side,
            signatureType: 0,
            signature: ""
        });
    }
    
    function _executeOrder(PolymarketOrder memory order) internal {
        // In production, this would call CTF Exchange's fillOrder function
        // bytes memory data = abi.encodeWithSignature(
        //     "fillOrder((address,address,uint256,uint256,uint256,uint256,uint256,uint256,uint256,uint8,uint8,bytes))",
        //     order
        // );
        // (bool success,) = CTF_EXCHANGE.call(data);
        // require(success, "Order execution failed");
    }
    
    function _fetchPolymarketPrice(bytes32 conditionId) internal view returns (uint256) {
        // In production, this would query Polymarket's API
        // For now, return a simulated price based on block data
        return 5000 + (uint256(keccak256(abi.encodePacked(block.timestamp, conditionId))) % 5000);
    }
    
    function _getOutcomeTokenBalance(bytes32 conditionId, address user) internal view returns (uint256) {
        // Query CTF token balance
        // In production: IERC1155(CTF_TOKEN).balanceOf(user, tokenId)
        return 0;
    }
    
    function _redeemOutcomeTokens(bytes32 conditionId, uint256 amount) internal {
        // Redeem outcome tokens for USDC through CTF
        // In production, would call CTF's redeemPositions
    }
    
    function _getResolution(bytes32 conditionId, address oracle) internal view returns (uint256[] memory) {
        // Get resolution from oracle
        // In production: IOracle(oracle).getPayouts(questionId)
        uint256[] memory payouts = new uint256[](2);
        payouts[0] = 10000; // Example: outcome 0 wins
        payouts[1] = 0;
        return payouts;
    }
    
    function _calculateSettlementPrice(uint256[] memory payouts) internal pure returns (uint256) {
        // Calculate settlement price from payouts
        if (payouts.length == 0) return 5000;
        return payouts[0] * 10000 / (payouts[0] + (payouts.length > 1 ? payouts[1] : 0));
    }
    
    // ============ Admin Functions ============
    
    function setBettingPlatform(address _platform) external onlyOwner {
        bettingPlatform = _platform;
    }
    
    function setAuthorizedCaller(address caller, bool authorized) external onlyOwner {
        authorizedCallers[caller] = authorized;
    }
    
    function setPriceUpdateInterval(uint256 interval) external onlyOwner {
        priceUpdateInterval = interval;
    }
    
    function emergencyWithdraw(address token, uint256 amount) external onlyOwner {
        IERC20(token).safeTransfer(owner(), amount);
    }
    
    // ============ View Functions ============
    
    function getMarketMapping(bytes32 marketId) external view returns (MarketMapping memory) {
        return marketMappings[marketId];
    }
    
    function isMarketActive(bytes32 marketId) external view returns (bool) {
        return marketMappings[marketId].isActive;
    }
    
    function getLastPriceUpdate(bytes32 marketId) external view returns (uint256) {
        return lastPriceUpdate[marketId];
    }
}

// Interface for BettingPlatform callback
interface IBettingPlatform {
    function resolveMarket(bytes32 marketId, uint256 settlementPrice) external;
}