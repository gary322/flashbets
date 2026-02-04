// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

import "@openzeppelin/contracts/access/AccessControl.sol";
import "@openzeppelin/contracts/security/ReentrancyGuard.sol";
import "@openzeppelin/contracts/security/Pausable.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import "@openzeppelin/contracts/utils/math/SafeMath.sol";

/**
 * @title BettingPlatform
 * @notice Main betting platform contract for Polygon deployment
 * @dev Integrates with Polymarket CTF Exchange for market data and settlement
 */
contract BettingPlatform is AccessControl, ReentrancyGuard, Pausable {
    using SafeERC20 for IERC20;
    using SafeMath for uint256;

    // Roles
    bytes32 public constant ADMIN_ROLE = keccak256("ADMIN_ROLE");
    bytes32 public constant OPERATOR_ROLE = keccak256("OPERATOR_ROLE");
    bytes32 public constant KEEPER_ROLE = keccak256("KEEPER_ROLE");

    // State structures
    struct Position {
        address trader;
        bytes32 marketId;
        uint256 size;
        uint256 collateral;
        uint256 leverage;
        bool isLong;
        uint256 entryPrice;
        uint256 timestamp;
        bool isOpen;
        uint256 realizedPnL;
    }

    struct Market {
        bytes32 id;
        string description;
        address oracle;
        uint256 strikePrice;
        uint256 expiryTime;
        bool isResolved;
        uint256 settlementPrice;
        uint256 totalLongInterest;
        uint256 totalShortInterest;
        uint256 maxLeverage;
        bool isActive;
    }

    struct FeeStructure {
        uint256 makerFee;    // Basis points (0.01%)
        uint256 takerFee;    // Basis points
        uint256 liquidationFee;
        uint256 treasuryShare;
        uint256 insuranceShare;
        uint256 stakersShare;
    }

    // State variables
    mapping(bytes32 => Position) public positions;
    mapping(bytes32 => Market) public markets;
    mapping(address => bytes32[]) public userPositions;
    mapping(address => uint256) public userBalances;
    
    FeeStructure public fees;
    address public treasury;
    address public insuranceFund;
    address public polymarketIntegration;
    address public leverageVault;
    
    uint256 public totalVolume;
    uint256 public totalFees;
    uint256 public positionCounter;
    uint256 public constant MAX_LEVERAGE = 500;
    uint256 public constant MIN_COLLATERAL = 10 * 10**6; // 10 USDC
    uint256 public constant LIQUIDATION_THRESHOLD = 8000; // 80%
    
    IERC20 public collateralToken; // USDC

    // Events
    event PositionOpened(
        bytes32 indexed positionId,
        address indexed trader,
        bytes32 indexed marketId,
        uint256 size,
        uint256 collateral,
        uint256 leverage,
        bool isLong
    );
    
    event PositionClosed(
        bytes32 indexed positionId,
        address indexed trader,
        uint256 realizedPnL,
        uint256 fees
    );
    
    event MarketCreated(
        bytes32 indexed marketId,
        string description,
        uint256 expiryTime,
        uint256 maxLeverage
    );
    
    event Liquidation(
        bytes32 indexed positionId,
        address indexed liquidator,
        uint256 reward
    );
    
    event FeesDistributed(
        uint256 treasuryAmount,
        uint256 insuranceAmount,
        uint256 stakersAmount
    );

    constructor(
        address _collateralToken,
        address _treasury,
        address _insuranceFund
    ) {
        _setupRole(DEFAULT_ADMIN_ROLE, msg.sender);
        _setupRole(ADMIN_ROLE, msg.sender);
        
        collateralToken = IERC20(_collateralToken);
        treasury = _treasury;
        insuranceFund = _insuranceFund;
        
        // Initialize fee structure (basis points)
        fees = FeeStructure({
            makerFee: 10,      // 0.1%
            takerFee: 30,      // 0.3%
            liquidationFee: 100, // 1%
            treasuryShare: 3000, // 30%
            insuranceShare: 2000, // 20%
            stakersShare: 5000   // 50%
        });
    }

    // ============ Position Management ============

    /**
     * @notice Opens a new trading position
     * @param marketId The market identifier
     * @param collateral Amount of collateral to deposit
     * @param leverage Leverage multiplier (1-500)
     * @param isLong True for long, false for short
     */
    function openPosition(
        bytes32 marketId,
        uint256 collateral,
        uint256 leverage,
        bool isLong
    ) external nonReentrant whenNotPaused returns (bytes32) {
        require(markets[marketId].isActive, "Market not active");
        require(collateral >= MIN_COLLATERAL, "Insufficient collateral");
        require(leverage > 0 && leverage <= MAX_LEVERAGE, "Invalid leverage");
        require(leverage <= markets[marketId].maxLeverage, "Exceeds market max leverage");
        
        // Transfer collateral from user
        collateralToken.safeTransferFrom(msg.sender, address(this), collateral);
        
        // Calculate position size
        uint256 size = collateral.mul(leverage);
        
        // Generate position ID
        bytes32 positionId = keccak256(abi.encodePacked(msg.sender, marketId, positionCounter++));
        
        // Get current price from oracle or Polymarket
        uint256 entryPrice = _getCurrentPrice(marketId);
        
        // Create position
        positions[positionId] = Position({
            trader: msg.sender,
            marketId: marketId,
            size: size,
            collateral: collateral,
            leverage: leverage,
            isLong: isLong,
            entryPrice: entryPrice,
            timestamp: block.timestamp,
            isOpen: true,
            realizedPnL: 0
        });
        
        // Update market open interest
        if (isLong) {
            markets[marketId].totalLongInterest = markets[marketId].totalLongInterest.add(size);
        } else {
            markets[marketId].totalShortInterest = markets[marketId].totalShortInterest.add(size);
        }
        
        // Add to user's positions
        userPositions[msg.sender].push(positionId);
        
        // Update total volume
        totalVolume = totalVolume.add(size);
        
        emit PositionOpened(positionId, msg.sender, marketId, size, collateral, leverage, isLong);
        
        return positionId;
    }

    /**
     * @notice Closes an existing position
     * @param positionId The position identifier
     */
    function closePosition(bytes32 positionId) external nonReentrant {
        Position storage position = positions[positionId];
        require(position.trader == msg.sender, "Not position owner");
        require(position.isOpen, "Position already closed");
        
        // Get current price
        uint256 currentPrice = _getCurrentPrice(position.marketId);
        
        // Calculate PnL
        int256 pnl = _calculatePnL(position, currentPrice);
        
        // Calculate fees
        uint256 closingFee = position.size.mul(fees.takerFee).div(10000);
        
        // Update position
        position.isOpen = false;
        position.realizedPnL = pnl > 0 ? uint256(pnl) : 0;
        
        // Update market open interest
        if (position.isLong) {
            markets[position.marketId].totalLongInterest = 
                markets[position.marketId].totalLongInterest.sub(position.size);
        } else {
            markets[position.marketId].totalShortInterest = 
                markets[position.marketId].totalShortInterest.sub(position.size);
        }
        
        // Calculate final payout
        uint256 payout = position.collateral;
        if (pnl > 0) {
            payout = payout.add(uint256(pnl));
        } else if (pnl < 0) {
            uint256 loss = uint256(-pnl);
            if (loss >= position.collateral) {
                payout = 0; // Total loss
            } else {
                payout = position.collateral.sub(loss);
            }
        }
        
        // Deduct fees
        if (payout > closingFee) {
            payout = payout.sub(closingFee);
            _distributeFees(closingFee);
        } else {
            _distributeFees(payout);
            payout = 0;
        }
        
        // Transfer payout to trader
        if (payout > 0) {
            collateralToken.safeTransfer(msg.sender, payout);
        }
        
        emit PositionClosed(positionId, msg.sender, position.realizedPnL, closingFee);
    }

    /**
     * @notice Liquidates an undercollateralized position
     * @param positionId The position to liquidate
     */
    function liquidatePosition(bytes32 positionId) external nonReentrant {
        Position storage position = positions[positionId];
        require(position.isOpen, "Position not open");
        
        uint256 currentPrice = _getCurrentPrice(position.marketId);
        int256 pnl = _calculatePnL(position, currentPrice);
        
        // Check if position is liquidatable
        uint256 maintenanceMargin = position.collateral.mul(LIQUIDATION_THRESHOLD).div(10000);
        bool isLiquidatable = false;
        
        if (pnl < 0) {
            uint256 loss = uint256(-pnl);
            if (loss >= maintenanceMargin) {
                isLiquidatable = true;
            }
        }
        
        require(isLiquidatable, "Position not liquidatable");
        
        // Calculate liquidation reward
        uint256 liquidationReward = position.collateral.mul(fees.liquidationFee).div(10000);
        
        // Close position
        position.isOpen = false;
        
        // Update market open interest
        if (position.isLong) {
            markets[position.marketId].totalLongInterest = 
                markets[position.marketId].totalLongInterest.sub(position.size);
        } else {
            markets[position.marketId].totalShortInterest = 
                markets[position.marketId].totalShortInterest.sub(position.size);
        }
        
        // Transfer reward to liquidator
        if (liquidationReward > 0) {
            collateralToken.safeTransfer(msg.sender, liquidationReward);
        }
        
        // Remaining collateral goes to insurance fund
        uint256 remainingCollateral = position.collateral > liquidationReward ? 
            position.collateral.sub(liquidationReward) : 0;
        if (remainingCollateral > 0) {
            collateralToken.safeTransfer(insuranceFund, remainingCollateral);
        }
        
        emit Liquidation(positionId, msg.sender, liquidationReward);
    }

    // ============ Market Management ============

    /**
     * @notice Creates a new market
     * @param marketId Unique market identifier
     * @param description Market description
     * @param oracle Oracle address for price feeds
     * @param expiryTime Market expiry timestamp
     * @param maxLeverage Maximum allowed leverage
     */
    function createMarket(
        bytes32 marketId,
        string calldata description,
        address oracle,
        uint256 expiryTime,
        uint256 maxLeverage
    ) external onlyRole(OPERATOR_ROLE) {
        require(markets[marketId].id == bytes32(0), "Market already exists");
        require(expiryTime > block.timestamp, "Invalid expiry");
        require(maxLeverage > 0 && maxLeverage <= MAX_LEVERAGE, "Invalid max leverage");
        
        markets[marketId] = Market({
            id: marketId,
            description: description,
            oracle: oracle,
            strikePrice: 0,
            expiryTime: expiryTime,
            isResolved: false,
            settlementPrice: 0,
            totalLongInterest: 0,
            totalShortInterest: 0,
            maxLeverage: maxLeverage,
            isActive: true
        });
        
        emit MarketCreated(marketId, description, expiryTime, maxLeverage);
    }

    /**
     * @notice Resolves a market with final price
     * @param marketId The market to resolve
     * @param settlementPrice The final settlement price
     */
    function resolveMarket(bytes32 marketId, uint256 settlementPrice) 
        external 
        onlyRole(KEEPER_ROLE) 
    {
        Market storage market = markets[marketId];
        require(market.isActive, "Market not active");
        require(!market.isResolved, "Market already resolved");
        require(block.timestamp >= market.expiryTime, "Market not expired");
        
        market.isResolved = true;
        market.settlementPrice = settlementPrice;
        market.isActive = false;
    }

    // ============ Internal Functions ============

    function _getCurrentPrice(bytes32 marketId) internal view returns (uint256) {
        Market memory market = markets[marketId];
        if (market.isResolved) {
            return market.settlementPrice;
        }
        
        // Get price from oracle or Polymarket integration
        if (polymarketIntegration != address(0)) {
            // Call Polymarket integration for price
            return IPolymarketIntegration(polymarketIntegration).getMarketPrice(marketId);
        } else if (market.oracle != address(0)) {
            // Get from oracle
            return IOracle(market.oracle).getPrice(marketId);
        }
        
        revert("No price source available");
    }

    function _calculatePnL(Position memory position, uint256 currentPrice) 
        internal 
        pure 
        returns (int256) 
    {
        int256 priceDiff = int256(currentPrice) - int256(position.entryPrice);
        int256 pnl;
        
        if (position.isLong) {
            pnl = (priceDiff * int256(position.size)) / int256(position.entryPrice);
        } else {
            pnl = (-priceDiff * int256(position.size)) / int256(position.entryPrice);
        }
        
        return pnl;
    }

    function _distributeFees(uint256 amount) internal {
        uint256 treasuryAmount = amount.mul(fees.treasuryShare).div(10000);
        uint256 insuranceAmount = amount.mul(fees.insuranceShare).div(10000);
        uint256 stakersAmount = amount.sub(treasuryAmount).sub(insuranceAmount);
        
        if (treasuryAmount > 0) {
            collateralToken.safeTransfer(treasury, treasuryAmount);
        }
        if (insuranceAmount > 0) {
            collateralToken.safeTransfer(insuranceFund, insuranceAmount);
        }
        // Stakers amount handled by staking contract
        
        totalFees = totalFees.add(amount);
        
        emit FeesDistributed(treasuryAmount, insuranceAmount, stakersAmount);
    }

    // ============ Admin Functions ============

    function setPolymarketIntegration(address _integration) external onlyRole(ADMIN_ROLE) {
        polymarketIntegration = _integration;
    }

    function setLeverageVault(address _vault) external onlyRole(ADMIN_ROLE) {
        leverageVault = _vault;
    }

    function updateFees(FeeStructure calldata _fees) external onlyRole(ADMIN_ROLE) {
        fees = _fees;
    }

    function pause() external onlyRole(ADMIN_ROLE) {
        _pause();
    }

    function unpause() external onlyRole(ADMIN_ROLE) {
        _unpause();
    }

    function emergencyWithdraw(address token, uint256 amount) 
        external 
        onlyRole(DEFAULT_ADMIN_ROLE) 
    {
        IERC20(token).safeTransfer(msg.sender, amount);
    }

    // ============ View Functions ============

    function getUserPositions(address user) external view returns (bytes32[] memory) {
        return userPositions[user];
    }

    function getMarketInfo(bytes32 marketId) external view returns (Market memory) {
        return markets[marketId];
    }

    function getPositionInfo(bytes32 positionId) external view returns (Position memory) {
        return positions[positionId];
    }

    function getHealthFactor(bytes32 positionId) external view returns (uint256) {
        Position memory position = positions[positionId];
        if (!position.isOpen) return 0;
        
        uint256 currentPrice = _getCurrentPrice(position.marketId);
        int256 pnl = _calculatePnL(position, currentPrice);
        
        if (pnl >= 0) {
            return 10000; // 100% healthy
        }
        
        uint256 loss = uint256(-pnl);
        if (loss >= position.collateral) {
            return 0; // Liquidatable
        }
        
        return (position.collateral.sub(loss)).mul(10000).div(position.collateral);
    }
}

// Interface for Polymarket Integration
interface IPolymarketIntegration {
    function getMarketPrice(bytes32 marketId) external view returns (uint256);
}

// Interface for Oracle
interface IOracle {
    function getPrice(bytes32 marketId) external view returns (uint256);
}