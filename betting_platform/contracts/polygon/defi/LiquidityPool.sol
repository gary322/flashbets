// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

import "@openzeppelin/contracts/access/AccessControl.sol";
import "@openzeppelin/contracts/security/ReentrancyGuard.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import "@openzeppelin/contracts/utils/math/SafeMath.sol";

/**
 * @title LiquidityPool
 * @notice Provides liquidity for betting markets with multiple AMM models
 * @dev Implements LMSR, PM-AMM, L2-AMM, and Hybrid models
 */
contract LiquidityPool is ERC20, AccessControl, ReentrancyGuard {
    using SafeERC20 for IERC20;
    using SafeMath for uint256;
    
    // Roles
    bytes32 public constant OPERATOR_ROLE = keccak256("OPERATOR_ROLE");
    bytes32 public constant MARKET_MAKER_ROLE = keccak256("MARKET_MAKER_ROLE");
    
    // AMM Types
    enum AMMType {
        LMSR,      // Logarithmic Market Scoring Rule
        PM_AMM,    // Prediction Market AMM
        L2_AMM,    // Layer 2 optimized AMM
        HYBRID     // Combines multiple models
    }
    
    // Pool configuration
    struct PoolConfig {
        AMMType ammType;
        uint256 fee;                // Trading fee in basis points
        uint256 subsidyFactor;       // For LMSR
        uint256 liquidityParameter; // For PM-AMM
        uint256 maxSlippage;        // Maximum allowed slippage
        uint256 minLiquidity;       // Minimum liquidity requirement
        bool dynamicFees;           // Enable dynamic fee adjustment
    }
    
    // Market liquidity
    struct MarketLiquidity {
        bytes32 marketId;
        uint256 totalLiquidity;
        uint256 yesLiquidity;
        uint256 noLiquidity;
        uint256 volume24h;
        uint256 fees24h;
        uint256 lastUpdate;
        AMMType ammType;
    }
    
    // LP position
    struct LPPosition {
        address provider;
        uint256 liquidity;
        uint256 shares;
        uint256 depositTime;
        uint256 lastClaimTime;
        uint256 accruedFees;
    }
    
    // Constants
    uint256 public constant PRECISION = 10000;
    uint256 public constant MIN_LIQUIDITY = 1000 * 10**6; // 1000 USDC
    uint256 public constant LOCK_PERIOD = 24 hours;
    
    // State variables
    PoolConfig public poolConfig;
    mapping(bytes32 => MarketLiquidity) public marketLiquidity;
    mapping(address => LPPosition) public lpPositions;
    mapping(bytes32 => mapping(address => uint256)) public marketProviderLiquidity;
    
    IERC20 public liquidityToken;
    address public bettingPlatform;
    address public treasury;
    
    uint256 public totalLiquidity;
    uint256 public totalFees;
    uint256 public totalVolume;
    
    // Events
    event LiquidityAdded(
        address indexed provider,
        uint256 amount,
        uint256 shares
    );
    
    event LiquidityRemoved(
        address indexed provider,
        uint256 amount,
        uint256 shares
    );
    
    event MarketLiquidityUpdated(
        bytes32 indexed marketId,
        uint256 totalLiquidity,
        AMMType ammType
    );
    
    event TradeExecuted(
        bytes32 indexed marketId,
        address indexed trader,
        bool isBuy,
        uint256 amount,
        uint256 price,
        uint256 fee
    );
    
    event FeesDistributed(
        uint256 totalAmount,
        uint256 lpShare,
        uint256 treasuryShare
    );
    
    constructor(
        address _liquidityToken,
        address _treasury,
        string memory _name,
        string memory _symbol
    ) ERC20(_name, _symbol) {
        _setupRole(DEFAULT_ADMIN_ROLE, msg.sender);
        _setupRole(OPERATOR_ROLE, msg.sender);
        
        liquidityToken = IERC20(_liquidityToken);
        treasury = _treasury;
        
        // Initialize default pool config
        poolConfig = PoolConfig({
            ammType: AMMType.HYBRID,
            fee: 30,                    // 0.3%
            subsidyFactor: 100 * 10**6, // 100 USDC
            liquidityParameter: 1000,
            maxSlippage: 500,           // 5%
            minLiquidity: MIN_LIQUIDITY,
            dynamicFees: true
        });
    }
    
    // ============ Liquidity Provision ============
    
    /**
     * @notice Adds liquidity to the pool
     * @param amount Amount of liquidity tokens to add
     */
    function addLiquidity(uint256 amount) external nonReentrant returns (uint256) {
        require(amount >= poolConfig.minLiquidity, "Below minimum liquidity");
        
        // Transfer liquidity tokens
        liquidityToken.safeTransferFrom(msg.sender, address(this), amount);
        
        // Calculate LP shares
        uint256 shares;
        if (totalSupply() == 0) {
            shares = amount;
        } else {
            shares = amount.mul(totalSupply()).div(totalLiquidity);
        }
        
        // Mint LP tokens
        _mint(msg.sender, shares);
        
        // Update LP position
        LPPosition storage position = lpPositions[msg.sender];
        position.provider = msg.sender;
        position.liquidity = position.liquidity.add(amount);
        position.shares = position.shares.add(shares);
        position.depositTime = block.timestamp;
        
        // Update totals
        totalLiquidity = totalLiquidity.add(amount);
        
        emit LiquidityAdded(msg.sender, amount, shares);
        
        return shares;
    }
    
    /**
     * @notice Removes liquidity from the pool
     * @param shares Number of LP shares to burn
     */
    function removeLiquidity(uint256 shares) external nonReentrant returns (uint256) {
        require(shares > 0, "Zero shares");
        require(balanceOf(msg.sender) >= shares, "Insufficient shares");
        
        LPPosition storage position = lpPositions[msg.sender];
        require(block.timestamp >= position.depositTime + LOCK_PERIOD, "Still locked");
        
        // Calculate liquidity amount
        uint256 amount = shares.mul(totalLiquidity).div(totalSupply());
        
        // Burn LP tokens
        _burn(msg.sender, shares);
        
        // Update position
        position.shares = position.shares.sub(shares);
        position.liquidity = position.liquidity.sub(amount);
        
        // Claim accrued fees
        uint256 fees = position.accruedFees;
        if (fees > 0) {
            position.accruedFees = 0;
            amount = amount.add(fees);
        }
        
        // Update totals
        totalLiquidity = totalLiquidity.sub(amount);
        
        // Transfer liquidity
        liquidityToken.safeTransfer(msg.sender, amount);
        
        emit LiquidityRemoved(msg.sender, amount, shares);
        
        return amount;
    }
    
    /**
     * @notice Adds liquidity to a specific market
     * @param marketId Market identifier
     * @param amount Liquidity amount
     */
    function addMarketLiquidity(bytes32 marketId, uint256 amount) 
        external 
        onlyRole(MARKET_MAKER_ROLE) 
    {
        require(amount > 0, "Zero amount");
        
        MarketLiquidity storage market = marketLiquidity[marketId];
        
        // Initialize market if needed
        if (market.marketId == bytes32(0)) {
            market.marketId = marketId;
            market.ammType = poolConfig.ammType;
        }
        
        // Update liquidity
        market.totalLiquidity = market.totalLiquidity.add(amount);
        market.yesLiquidity = market.yesLiquidity.add(amount.div(2));
        market.noLiquidity = market.noLiquidity.add(amount.div(2));
        market.lastUpdate = block.timestamp;
        
        // Track provider liquidity
        marketProviderLiquidity[marketId][msg.sender] = 
            marketProviderLiquidity[marketId][msg.sender].add(amount);
        
        emit MarketLiquidityUpdated(marketId, market.totalLiquidity, market.ammType);
    }
    
    // ============ Trading Functions ============
    
    /**
     * @notice Gets the price for a trade
     * @param marketId Market identifier
     * @param isBuy True for buy, false for sell
     * @param outcome 0 for NO, 1 for YES
     * @param amount Trade amount
     */
    function getPrice(
        bytes32 marketId,
        bool isBuy,
        uint256 outcome,
        uint256 amount
    ) public view returns (uint256) {
        MarketLiquidity memory market = marketLiquidity[marketId];
        require(market.totalLiquidity > 0, "No liquidity");
        
        if (market.ammType == AMMType.LMSR) {
            return _getLMSRPrice(market, isBuy, outcome, amount);
        } else if (market.ammType == AMMType.PM_AMM) {
            return _getPMAMMPrice(market, isBuy, outcome, amount);
        } else if (market.ammType == AMMType.L2_AMM) {
            return _getL2AMMPrice(market, isBuy, outcome, amount);
        } else {
            return _getHybridPrice(market, isBuy, outcome, amount);
        }
    }
    
    /**
     * @notice Executes a trade
     * @param marketId Market identifier
     * @param isBuy True for buy, false for sell
     * @param outcome 0 for NO, 1 for YES
     * @param amount Trade amount
     * @param maxPrice Maximum acceptable price
     */
    function trade(
        bytes32 marketId,
        bool isBuy,
        uint256 outcome,
        uint256 amount,
        uint256 maxPrice
    ) external nonReentrant returns (uint256) {
        MarketLiquidity storage market = marketLiquidity[marketId];
        require(market.totalLiquidity > 0, "No liquidity");
        
        // Calculate price
        uint256 price = getPrice(marketId, isBuy, outcome, amount);
        require(price <= maxPrice, "Price exceeds max");
        
        // Calculate cost and fee
        uint256 cost = amount.mul(price).div(PRECISION);
        uint256 fee = _calculateFee(cost, market.volume24h);
        uint256 totalCost = cost.add(fee);
        
        // Execute trade
        if (isBuy) {
            liquidityToken.safeTransferFrom(msg.sender, address(this), totalCost);
            
            if (outcome == 1) {
                market.yesLiquidity = market.yesLiquidity.sub(amount);
                market.noLiquidity = market.noLiquidity.add(cost);
            } else {
                market.noLiquidity = market.noLiquidity.sub(amount);
                market.yesLiquidity = market.yesLiquidity.add(cost);
            }
        } else {
            // Selling logic
            if (outcome == 1) {
                market.yesLiquidity = market.yesLiquidity.add(amount);
                market.noLiquidity = market.noLiquidity.sub(cost);
            } else {
                market.noLiquidity = market.noLiquidity.add(amount);
                market.yesLiquidity = market.yesLiquidity.sub(cost);
            }
            
            liquidityToken.safeTransfer(msg.sender, cost.sub(fee));
        }
        
        // Update market stats
        market.volume24h = market.volume24h.add(amount);
        market.fees24h = market.fees24h.add(fee);
        market.lastUpdate = block.timestamp;
        
        // Update global stats
        totalVolume = totalVolume.add(amount);
        totalFees = totalFees.add(fee);
        
        // Distribute fees
        _distributeFees(fee);
        
        emit TradeExecuted(marketId, msg.sender, isBuy, amount, price, fee);
        
        return totalCost;
    }
    
    // ============ AMM Price Functions ============
    
    function _getLMSRPrice(
        MarketLiquidity memory market,
        bool isBuy,
        uint256 outcome,
        uint256 amount
    ) internal view returns (uint256) {
        uint256 b = poolConfig.subsidyFactor;
        uint256 q1 = outcome == 1 ? market.yesLiquidity : market.noLiquidity;
        uint256 q2 = outcome == 1 ? market.noLiquidity : market.yesLiquidity;
        
        // LMSR formula: C(q) = b * ln(exp(q1/b) + exp(q2/b))
        // Price = dC/dq
        
        uint256 exp1 = _exp(q1.mul(PRECISION).div(b));
        uint256 exp2 = _exp(q2.mul(PRECISION).div(b));
        uint256 sumExp = exp1.add(exp2);
        
        uint256 price;
        if (isBuy) {
            uint256 newQ1 = q1.add(amount);
            uint256 newExp1 = _exp(newQ1.mul(PRECISION).div(b));
            uint256 newSumExp = newExp1.add(exp2);
            price = newExp1.mul(PRECISION).div(newSumExp);
        } else {
            uint256 newQ1 = q1.sub(amount);
            uint256 newExp1 = _exp(newQ1.mul(PRECISION).div(b));
            uint256 newSumExp = newExp1.add(exp2);
            price = newExp1.mul(PRECISION).div(newSumExp);
        }
        
        return price;
    }
    
    function _getPMAMMPrice(
        MarketLiquidity memory market,
        bool isBuy,
        uint256 outcome,
        uint256 amount
    ) internal view returns (uint256) {
        // PM-AMM: Polynomial Market AMM
        uint256 k = market.yesLiquidity.mul(market.noLiquidity);
        uint256 price;
        
        if (outcome == 1) {
            if (isBuy) {
                uint256 newYes = market.yesLiquidity.sub(amount);
                uint256 newNo = k.div(newYes);
                price = newNo.sub(market.noLiquidity).mul(PRECISION).div(amount);
            } else {
                uint256 newYes = market.yesLiquidity.add(amount);
                uint256 newNo = k.div(newYes);
                price = market.noLiquidity.sub(newNo).mul(PRECISION).div(amount);
            }
        } else {
            if (isBuy) {
                uint256 newNo = market.noLiquidity.sub(amount);
                uint256 newYes = k.div(newNo);
                price = newYes.sub(market.yesLiquidity).mul(PRECISION).div(amount);
            } else {
                uint256 newNo = market.noLiquidity.add(amount);
                uint256 newYes = k.div(newNo);
                price = market.yesLiquidity.sub(newYes).mul(PRECISION).div(amount);
            }
        }
        
        return price;
    }
    
    function _getL2AMMPrice(
        MarketLiquidity memory market,
        bool isBuy,
        uint256 outcome,
        uint256 amount
    ) internal pure returns (uint256) {
        // L2-AMM: Optimized for Layer 2
        // Uses simplified constant product with gas optimization
        
        uint256 reserve1 = outcome == 1 ? market.yesLiquidity : market.noLiquidity;
        uint256 reserve2 = outcome == 1 ? market.noLiquidity : market.yesLiquidity;
        
        if (amount == 0) {
            return reserve1.mul(PRECISION).div(reserve1.add(reserve2));
        }
        
        uint256 price;
        if (isBuy) {
            // Price impact formula
            uint256 amountWithFee = amount.mul(997); // 0.3% fee built-in
            uint256 numerator = amountWithFee.mul(reserve2);
            uint256 denominator = reserve1.mul(1000).add(amountWithFee);
            price = numerator.div(denominator).mul(PRECISION).div(amount);
        } else {
            uint256 amountWithFee = amount.mul(997);
            uint256 numerator = amountWithFee.mul(reserve1);
            uint256 denominator = reserve2.mul(1000).add(amountWithFee);
            price = PRECISION.sub(numerator.div(denominator).mul(PRECISION).div(amount));
        }
        
        return price;
    }
    
    function _getHybridPrice(
        MarketLiquidity memory market,
        bool isBuy,
        uint256 outcome,
        uint256 amount
    ) internal view returns (uint256) {
        // Hybrid: Weighted average of different models
        uint256 lmsrPrice = _getLMSRPrice(market, isBuy, outcome, amount);
        uint256 pmammPrice = _getPMAMMPrice(market, isBuy, outcome, amount);
        uint256 l2ammPrice = _getL2AMMPrice(market, isBuy, outcome, amount);
        
        // Weight based on liquidity depth
        uint256 totalLiq = market.totalLiquidity;
        uint256 weight1 = totalLiq > 100000 * 10**6 ? 40 : 20; // LMSR weight
        uint256 weight2 = 40; // PM-AMM weight
        uint256 weight3 = 100 - weight1 - weight2; // L2-AMM weight
        
        return (lmsrPrice.mul(weight1)
            .add(pmammPrice.mul(weight2))
            .add(l2ammPrice.mul(weight3)))
            .div(100);
    }
    
    // ============ Fee Management ============
    
    function _calculateFee(uint256 amount, uint256 volume24h) internal view returns (uint256) {
        uint256 baseFee = poolConfig.fee;
        
        if (!poolConfig.dynamicFees) {
            return amount.mul(baseFee).div(PRECISION);
        }
        
        // Dynamic fee based on volume
        uint256 volumeTier = volume24h.div(100000 * 10**6); // Per 100k USDC
        uint256 feeReduction = volumeTier > 10 ? 10 : volumeTier;
        
        uint256 adjustedFee = baseFee > feeReduction ? baseFee.sub(feeReduction) : 1;
        
        return amount.mul(adjustedFee).div(PRECISION);
    }
    
    function _distributeFees(uint256 feeAmount) internal {
        if (feeAmount == 0) return;
        
        uint256 treasuryShare = feeAmount.mul(2000).div(PRECISION); // 20%
        uint256 lpShare = feeAmount.sub(treasuryShare);
        
        // Transfer treasury share
        if (treasuryShare > 0) {
            liquidityToken.safeTransfer(treasury, treasuryShare);
        }
        
        // Distribute LP share proportionally
        if (lpShare > 0 && totalSupply() > 0) {
            // Add to pool for LPs
            totalLiquidity = totalLiquidity.add(lpShare);
        }
        
        emit FeesDistributed(feeAmount, lpShare, treasuryShare);
    }
    
    // ============ Utility Functions ============
    
    function _exp(uint256 x) internal pure returns (uint256) {
        // Simplified exponential approximation
        // e^x ≈ 1 + x + x^2/2 + x^3/6 for small x
        if (x == 0) return PRECISION;
        
        uint256 x2 = x.mul(x).div(PRECISION);
        uint256 x3 = x2.mul(x).div(PRECISION);
        
        return PRECISION.add(x).add(x2.div(2)).add(x3.div(6));
    }
    
    // ============ Admin Functions ============
    
    function updatePoolConfig(PoolConfig calldata _config) 
        external 
        onlyRole(DEFAULT_ADMIN_ROLE) 
    {
        poolConfig = _config;
    }
    
    function setAMMType(bytes32 marketId, AMMType ammType) 
        external 
        onlyRole(OPERATOR_ROLE) 
    {
        marketLiquidity[marketId].ammType = ammType;
    }
    
    function setBettingPlatform(address _platform) 
        external 
        onlyRole(DEFAULT_ADMIN_ROLE) 
    {
        bettingPlatform = _platform;
    }
    
    function emergencyWithdraw(address token, uint256 amount) 
        external 
        onlyRole(DEFAULT_ADMIN_ROLE) 
    {
        IERC20(token).safeTransfer(msg.sender, amount);
    }
    
    // ============ View Functions ============
    
    function getMarketLiquidity(bytes32 marketId) 
        external 
        view 
        returns (MarketLiquidity memory) 
    {
        return marketLiquidity[marketId];
    }
    
    function getLPPosition(address provider) 
        external 
        view 
        returns (LPPosition memory) 
    {
        return lpPositions[provider];
    }
    
    function getPoolStats() 
        external 
        view 
        returns (uint256, uint256, uint256) 
    {
        return (totalLiquidity, totalVolume, totalFees);
    }
    
    function getMarketPrice(bytes32 marketId, uint256 outcome) 
        external 
        view 
        returns (uint256) 
    {
        return getPrice(marketId, true, outcome, 0);
    }
}