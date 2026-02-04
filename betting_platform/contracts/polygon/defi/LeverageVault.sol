// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

import "@openzeppelin/contracts/access/AccessControl.sol";
import "@openzeppelin/contracts/security/ReentrancyGuard.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import "@openzeppelin/contracts/utils/math/SafeMath.sol";
import "../interfaces/IPool.sol";

/**
 * @title LeverageVault
 * @notice Manages leveraged positions and collateral for 500x effective leverage
 * @dev Integrates with Aave V3 for capital efficiency
 */
contract LeverageVault is AccessControl, ReentrancyGuard {
    using SafeERC20 for IERC20;
    using SafeMath for uint256;
    
    // Roles
    bytes32 public constant MANAGER_ROLE = keccak256("MANAGER_ROLE");
    bytes32 public constant LIQUIDATOR_ROLE = keccak256("LIQUIDATOR_ROLE");
    
    // Leverage tiers
    struct LeverageTier {
        uint256 minCollateral;
        uint256 maxLeverage;
        uint256 maintenanceMargin;  // Basis points
        uint256 liquidationPenalty; // Basis points
        uint256 interestRate;       // Annual rate in basis points
    }
    
    // User position
    struct LeveragePosition {
        address user;
        uint256 collateral;
        uint256 debt;
        uint256 leverage;
        uint256 entryPrice;
        uint256 lastInterestUpdate;
        bool isLong;
        bytes32 marketId;
    }
    
    // Vault statistics
    struct VaultStats {
        uint256 totalCollateral;
        uint256 totalDebt;
        uint256 totalLiquidations;
        uint256 totalInterestEarned;
        uint256 utilizationRate;
    }
    
    // Constants
    uint256 public constant MAX_LEVERAGE = 500;
    uint256 public constant PRECISION = 10000;
    uint256 public constant SECONDS_PER_YEAR = 31536000;
    uint256 public constant LIQUIDATION_DISCOUNT = 500; // 5%
    
    // State variables
    mapping(bytes32 => LeveragePosition) public positions;
    mapping(address => bytes32[]) public userPositions;
    mapping(uint256 => LeverageTier) public leverageTiers;
    
    VaultStats public vaultStats;
    
    IERC20 public collateralToken;
    IPoolAddressesProvider public aaveAddressProvider;
    IPool public aavePool;
    
    address public bettingPlatform;
    address public flashBetting;
    address public treasury;
    address public insuranceFund;
    
    uint256 public baseInterestRate = 500; // 5% annual
    uint256 public interestMultiplier = 20; // 0.2% per utilization point
    uint256 public optimalUtilization = 8000; // 80%
    uint256 public excessUtilizationRate = 100; // 1% extra above optimal
    
    bool public emergencyMode = false;
    
    // Events
    event PositionOpened(
        bytes32 indexed positionId,
        address indexed user,
        uint256 collateral,
        uint256 leverage,
        uint256 debt
    );
    
    event PositionClosed(
        bytes32 indexed positionId,
        uint256 profit,
        uint256 loss
    );
    
    event Liquidation(
        bytes32 indexed positionId,
        address indexed liquidator,
        uint256 collateralSeized,
        uint256 debtRepaid
    );
    
    event InterestAccrued(
        bytes32 indexed positionId,
        uint256 interestAmount
    );
    
    event CollateralDeposited(
        address indexed user,
        uint256 amount
    );
    
    event CollateralWithdrawn(
        address indexed user,
        uint256 amount
    );
    
    event EmergencyModeActivated(string reason);
    
    constructor(
        address _collateralToken,
        address _aaveAddressProvider,
        address _treasury,
        address _insuranceFund
    ) {
        _setupRole(DEFAULT_ADMIN_ROLE, msg.sender);
        _setupRole(MANAGER_ROLE, msg.sender);
        
        collateralToken = IERC20(_collateralToken);
        aaveAddressProvider = IPoolAddressesProvider(_aaveAddressProvider);
        aavePool = IPool(aaveAddressProvider.getPool());
        treasury = _treasury;
        insuranceFund = _insuranceFund;
        
        _initializeLeverageTiers();
    }
    
    // ============ Position Management ============
    
    /**
     * @notice Opens a leveraged position
     * @param collateralAmount Amount of collateral to deposit
     * @param leverage Desired leverage (1-500)
     * @param isLong Direction of the position
     * @param marketId Associated market ID
     */
    function openPosition(
        uint256 collateralAmount,
        uint256 leverage,
        bool isLong,
        bytes32 marketId
    ) external nonReentrant returns (bytes32) {
        require(!emergencyMode, "Emergency mode active");
        require(leverage > 0 && leverage <= MAX_LEVERAGE, "Invalid leverage");
        require(collateralAmount > 0, "Zero collateral");
        
        // Check leverage tier
        LeverageTier memory tier = _getLeverageTier(collateralAmount, leverage);
        require(leverage <= tier.maxLeverage, "Exceeds max leverage for tier");
        
        // Transfer collateral
        collateralToken.safeTransferFrom(msg.sender, address(this), collateralAmount);
        
        // Calculate position size and debt
        uint256 positionSize = collateralAmount.mul(leverage);
        uint256 debt = positionSize.sub(collateralAmount);
        
        // Borrow from Aave if needed
        if (debt > 0) {
            _borrowFromAave(debt);
        }
        
        // Generate position ID
        bytes32 positionId = keccak256(abi.encodePacked(
            msg.sender,
            marketId,
            block.timestamp,
            collateralAmount
        ));
        
        // Create position
        positions[positionId] = LeveragePosition({
            user: msg.sender,
            collateral: collateralAmount,
            debt: debt,
            leverage: leverage,
            entryPrice: _getCurrentPrice(marketId),
            lastInterestUpdate: block.timestamp,
            isLong: isLong,
            marketId: marketId
        });
        
        // Update vault stats
        vaultStats.totalCollateral = vaultStats.totalCollateral.add(collateralAmount);
        vaultStats.totalDebt = vaultStats.totalDebt.add(debt);
        _updateUtilizationRate();
        
        // Track user position
        userPositions[msg.sender].push(positionId);
        
        emit PositionOpened(positionId, msg.sender, collateralAmount, leverage, debt);
        
        return positionId;
    }
    
    /**
     * @notice Closes a leveraged position
     * @param positionId Position identifier
     */
    function closePosition(bytes32 positionId) external nonReentrant {
        LeveragePosition storage position = positions[positionId];
        require(position.user == msg.sender || hasRole(MANAGER_ROLE, msg.sender), "Unauthorized");
        require(position.collateral > 0, "Position doesn't exist");
        
        // Calculate accrued interest
        uint256 interest = _calculateInterest(position);
        position.debt = position.debt.add(interest);
        
        // Get current price and calculate PnL
        uint256 currentPrice = _getCurrentPrice(position.marketId);
        (uint256 profit, uint256 loss) = _calculatePnL(position, currentPrice);
        
        // Repay debt to Aave
        if (position.debt > 0) {
            _repayToAave(position.debt);
        }
        
        // Calculate final payout
        uint256 payout = position.collateral;
        if (profit > 0) {
            payout = payout.add(profit);
        } else if (loss > 0) {
            if (loss >= position.collateral) {
                payout = 0; // Total loss
            } else {
                payout = position.collateral.sub(loss);
            }
        }
        
        // Update vault stats
        vaultStats.totalCollateral = vaultStats.totalCollateral.sub(position.collateral);
        vaultStats.totalDebt = vaultStats.totalDebt.sub(position.debt);
        vaultStats.totalInterestEarned = vaultStats.totalInterestEarned.add(interest);
        _updateUtilizationRate();
        
        // Clear position
        delete positions[positionId];
        
        // Transfer payout
        if (payout > 0) {
            collateralToken.safeTransfer(msg.sender, payout);
        }
        
        emit PositionClosed(positionId, profit, loss);
    }
    
    /**
     * @notice Adds collateral to an existing position
     * @param positionId Position identifier
     * @param amount Amount to add
     */
    function addCollateral(bytes32 positionId, uint256 amount) external nonReentrant {
        LeveragePosition storage position = positions[positionId];
        require(position.user == msg.sender, "Not position owner");
        require(amount > 0, "Zero amount");
        
        // Transfer collateral
        collateralToken.safeTransferFrom(msg.sender, address(this), amount);
        
        // Update position
        position.collateral = position.collateral.add(amount);
        
        // Update vault stats
        vaultStats.totalCollateral = vaultStats.totalCollateral.add(amount);
        
        emit CollateralDeposited(msg.sender, amount);
    }
    
    /**
     * @notice Removes excess collateral from a position
     * @param positionId Position identifier
     * @param amount Amount to remove
     */
    function removeCollateral(bytes32 positionId, uint256 amount) external nonReentrant {
        LeveragePosition storage position = positions[positionId];
        require(position.user == msg.sender, "Not position owner");
        require(amount > 0, "Zero amount");
        
        // Calculate minimum required collateral
        uint256 minCollateral = _calculateMinCollateral(position);
        require(position.collateral.sub(amount) >= minCollateral, "Insufficient collateral");
        
        // Update position
        position.collateral = position.collateral.sub(amount);
        
        // Update vault stats
        vaultStats.totalCollateral = vaultStats.totalCollateral.sub(amount);
        
        // Transfer collateral
        collateralToken.safeTransfer(msg.sender, amount);
        
        emit CollateralWithdrawn(msg.sender, amount);
    }
    
    // ============ Liquidation ============
    
    /**
     * @notice Liquidates an undercollateralized position
     * @param positionId Position to liquidate
     */
    function liquidate(bytes32 positionId) external nonReentrant {
        LeveragePosition storage position = positions[positionId];
        require(position.collateral > 0, "Position doesn't exist");
        
        // Check if liquidatable
        require(_isLiquidatable(position), "Position not liquidatable");
        
        // Calculate liquidation amounts
        uint256 debtToRepay = position.debt;
        uint256 collateralToSeize = position.collateral;
        uint256 liquidationReward = collateralToSeize.mul(LIQUIDATION_DISCOUNT).div(PRECISION);
        
        // Liquidator must repay the debt
        collateralToken.safeTransferFrom(msg.sender, address(this), debtToRepay);
        
        // Repay to Aave
        if (debtToRepay > 0) {
            _repayToAave(debtToRepay);
        }
        
        // Transfer rewards
        uint256 liquidatorReward = liquidationReward;
        uint256 insuranceAmount = collateralToSeize.sub(liquidationReward);
        
        if (liquidatorReward > 0) {
            collateralToken.safeTransfer(msg.sender, liquidatorReward);
        }
        
        if (insuranceAmount > 0) {
            collateralToken.safeTransfer(insuranceFund, insuranceAmount);
        }
        
        // Update vault stats
        vaultStats.totalCollateral = vaultStats.totalCollateral.sub(position.collateral);
        vaultStats.totalDebt = vaultStats.totalDebt.sub(position.debt);
        vaultStats.totalLiquidations = vaultStats.totalLiquidations.add(1);
        _updateUtilizationRate();
        
        // Clear position
        delete positions[positionId];
        
        emit Liquidation(positionId, msg.sender, collateralToSeize, debtToRepay);
    }
    
    // ============ Interest Management ============
    
    /**
     * @notice Accrues interest on a position
     * @param positionId Position identifier
     */
    function accrueInterest(bytes32 positionId) external nonReentrant {
        LeveragePosition storage position = positions[positionId];
        require(position.collateral > 0, "Position doesn't exist");
        
        uint256 interest = _calculateInterest(position);
        
        if (interest > 0) {
            position.debt = position.debt.add(interest);
            position.lastInterestUpdate = block.timestamp;
            
            vaultStats.totalDebt = vaultStats.totalDebt.add(interest);
            vaultStats.totalInterestEarned = vaultStats.totalInterestEarned.add(interest);
            
            emit InterestAccrued(positionId, interest);
        }
    }
    
    // ============ Internal Functions ============
    
    function _initializeLeverageTiers() internal {
        // Tier 1: Small positions
        leverageTiers[1] = LeverageTier({
            minCollateral: 10 * 10**6,    // 10 USDC
            maxLeverage: 50,
            maintenanceMargin: 1000,      // 10%
            liquidationPenalty: 500,       // 5%
            interestRate: 1000             // 10% annual
        });
        
        // Tier 2: Medium positions
        leverageTiers[2] = LeverageTier({
            minCollateral: 100 * 10**6,   // 100 USDC
            maxLeverage: 100,
            maintenanceMargin: 500,       // 5%
            liquidationPenalty: 300,       // 3%
            interestRate: 800              // 8% annual
        });
        
        // Tier 3: Large positions
        leverageTiers[3] = LeverageTier({
            minCollateral: 1000 * 10**6,  // 1000 USDC
            maxLeverage: 200,
            maintenanceMargin: 300,       // 3%
            liquidationPenalty: 200,       // 2%
            interestRate: 600              // 6% annual
        });
        
        // Tier 4: Whale positions
        leverageTiers[4] = LeverageTier({
            minCollateral: 10000 * 10**6, // 10000 USDC
            maxLeverage: 500,
            maintenanceMargin: 200,       // 2%
            liquidationPenalty: 100,       // 1%
            interestRate: 400              // 4% annual
        });
    }
    
    function _getLeverageTier(uint256 collateral, uint256 leverage) 
        internal 
        view 
        returns (LeverageTier memory) 
    {
        if (collateral >= leverageTiers[4].minCollateral) {
            return leverageTiers[4];
        } else if (collateral >= leverageTiers[3].minCollateral) {
            return leverageTiers[3];
        } else if (collateral >= leverageTiers[2].minCollateral) {
            return leverageTiers[2];
        } else {
            return leverageTiers[1];
        }
    }
    
    function _borrowFromAave(uint256 amount) internal {
        aavePool.borrow(
            address(collateralToken),
            amount,
            2, // Variable rate
            0,
            address(this)
        );
    }
    
    function _repayToAave(uint256 amount) internal {
        collateralToken.safeApprove(address(aavePool), amount);
        aavePool.repay(
            address(collateralToken),
            amount,
            2, // Variable rate
            address(this)
        );
    }
    
    function _getCurrentPrice(bytes32 marketId) internal view returns (uint256) {
        // Get price from betting platform
        (bool success, bytes memory data) = bettingPlatform.staticcall(
            abi.encodeWithSignature("getCurrentPrice(bytes32)", marketId)
        );
        
        if (success && data.length > 0) {
            return abi.decode(data, (uint256));
        }
        
        return 5000; // Default 50%
    }
    
    function _calculatePnL(LeveragePosition memory position, uint256 currentPrice) 
        internal 
        pure 
        returns (uint256 profit, uint256 loss) 
    {
        uint256 positionSize = position.collateral.mul(position.leverage);
        
        if (position.isLong) {
            if (currentPrice > position.entryPrice) {
                profit = positionSize.mul(currentPrice.sub(position.entryPrice)).div(position.entryPrice);
            } else {
                loss = positionSize.mul(position.entryPrice.sub(currentPrice)).div(position.entryPrice);
            }
        } else {
            if (currentPrice < position.entryPrice) {
                profit = positionSize.mul(position.entryPrice.sub(currentPrice)).div(position.entryPrice);
            } else {
                loss = positionSize.mul(currentPrice.sub(position.entryPrice)).div(position.entryPrice);
            }
        }
        
        return (profit, loss);
    }
    
    function _calculateInterest(LeveragePosition memory position) 
        internal 
        view 
        returns (uint256) 
    {
        if (position.debt == 0) return 0;
        
        uint256 timeElapsed = block.timestamp.sub(position.lastInterestUpdate);
        uint256 rate = _getInterestRate();
        
        return position.debt.mul(rate).mul(timeElapsed).div(PRECISION).div(SECONDS_PER_YEAR);
    }
    
    function _getInterestRate() internal view returns (uint256) {
        uint256 utilization = vaultStats.utilizationRate;
        
        if (utilization <= optimalUtilization) {
            return baseInterestRate.add(utilization.mul(interestMultiplier).div(PRECISION));
        } else {
            uint256 excessUtilization = utilization.sub(optimalUtilization);
            return baseInterestRate
                .add(optimalUtilization.mul(interestMultiplier).div(PRECISION))
                .add(excessUtilization.mul(excessUtilizationRate).div(PRECISION));
        }
    }
    
    function _calculateMinCollateral(LeveragePosition memory position) 
        internal 
        view 
        returns (uint256) 
    {
        LeverageTier memory tier = _getLeverageTier(position.collateral, position.leverage);
        uint256 positionSize = position.collateral.mul(position.leverage);
        
        return positionSize.mul(tier.maintenanceMargin).div(PRECISION);
    }
    
    function _isLiquidatable(LeveragePosition memory position) 
        internal 
        view 
        returns (bool) 
    {
        uint256 currentPrice = _getCurrentPrice(position.marketId);
        (, uint256 loss) = _calculatePnL(position, currentPrice);
        
        if (loss == 0) return false;
        
        uint256 minCollateral = _calculateMinCollateral(position);
        uint256 effectiveCollateral = position.collateral > loss ? 
            position.collateral.sub(loss) : 0;
            
        return effectiveCollateral < minCollateral;
    }
    
    function _updateUtilizationRate() internal {
        if (vaultStats.totalCollateral == 0) {
            vaultStats.utilizationRate = 0;
        } else {
            vaultStats.utilizationRate = vaultStats.totalDebt.mul(PRECISION).div(
                vaultStats.totalCollateral.add(vaultStats.totalDebt)
            );
        }
    }
    
    // ============ Emergency Functions ============
    
    function activateEmergencyMode(string calldata reason) 
        external 
        onlyRole(DEFAULT_ADMIN_ROLE) 
    {
        emergencyMode = true;
        emit EmergencyModeActivated(reason);
    }
    
    function deactivateEmergencyMode() 
        external 
        onlyRole(DEFAULT_ADMIN_ROLE) 
    {
        emergencyMode = false;
    }
    
    // ============ Admin Functions ============
    
    function setInterestParameters(
        uint256 _baseRate,
        uint256 _multiplier,
        uint256 _optimal,
        uint256 _excess
    ) external onlyRole(DEFAULT_ADMIN_ROLE) {
        baseInterestRate = _baseRate;
        interestMultiplier = _multiplier;
        optimalUtilization = _optimal;
        excessUtilizationRate = _excess;
    }
    
    function setBettingPlatform(address _platform) external onlyRole(DEFAULT_ADMIN_ROLE) {
        bettingPlatform = _platform;
    }
    
    function setFlashBetting(address _flash) external onlyRole(DEFAULT_ADMIN_ROLE) {
        flashBetting = _flash;
    }
    
    // ============ View Functions ============
    
    function getPosition(bytes32 positionId) external view returns (LeveragePosition memory) {
        return positions[positionId];
    }
    
    function getUserPositions(address user) external view returns (bytes32[] memory) {
        return userPositions[user];
    }
    
    function getVaultStats() external view returns (VaultStats memory) {
        return vaultStats;
    }
    
    function getHealthFactor(bytes32 positionId) external view returns (uint256) {
        LeveragePosition memory position = positions[positionId];
        if (position.collateral == 0) return 0;
        
        uint256 currentPrice = _getCurrentPrice(position.marketId);
        (, uint256 loss) = _calculatePnL(position, currentPrice);
        
        if (loss == 0) return PRECISION; // 100% healthy
        
        uint256 effectiveCollateral = position.collateral > loss ? 
            position.collateral.sub(loss) : 0;
        uint256 minCollateral = _calculateMinCollateral(position);
        
        if (minCollateral == 0) return PRECISION;
        
        return effectiveCollateral.mul(PRECISION).div(minCollateral);
    }
    
    function getCurrentInterestRate() external view returns (uint256) {
        return _getInterestRate();
    }
    
    function getLeverageTier(uint256 collateral, uint256 leverage) 
        external 
        view 
        returns (LeverageTier memory) 
    {
        return _getLeverageTier(collateral, leverage);
    }
}