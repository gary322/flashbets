// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";

/**
 * @title MockAavePool
 * @notice Mock Aave V3 Pool for testing
 */
contract MockAavePool {
    using SafeERC20 for IERC20;
    
    mapping(address => mapping(address => uint256)) public userBorrows;
    mapping(address => mapping(address => uint256)) public userDeposits;
    
    event Borrow(
        address indexed asset,
        address indexed user,
        uint256 amount,
        uint256 interestRateMode,
        uint16 referralCode
    );
    
    event Repay(
        address indexed asset,
        address indexed user,
        uint256 amount,
        uint256 interestRateMode
    );
    
    event Supply(
        address indexed asset,
        address indexed user,
        uint256 amount,
        uint16 referralCode
    );
    
    event Withdraw(
        address indexed asset,
        address indexed user,
        uint256 amount
    );
    
    /**
     * @notice Mock borrow function
     */
    function borrow(
        address asset,
        uint256 amount,
        uint256 interestRateMode,
        uint16 referralCode,
        address onBehalfOf
    ) external {
        // Simply transfer tokens from this contract
        // In production, this would handle complex lending logic
        IERC20(asset).safeTransfer(msg.sender, amount);
        userBorrows[onBehalfOf][asset] += amount;
        
        emit Borrow(asset, onBehalfOf, amount, interestRateMode, referralCode);
    }
    
    /**
     * @notice Mock repay function
     */
    function repay(
        address asset,
        uint256 amount,
        uint256 interestRateMode,
        address onBehalfOf
    ) external returns (uint256) {
        IERC20(asset).safeTransferFrom(msg.sender, address(this), amount);
        
        uint256 currentBorrow = userBorrows[onBehalfOf][asset];
        uint256 repayAmount = amount > currentBorrow ? currentBorrow : amount;
        userBorrows[onBehalfOf][asset] -= repayAmount;
        
        emit Repay(asset, onBehalfOf, repayAmount, interestRateMode);
        
        return repayAmount;
    }
    
    /**
     * @notice Mock supply function
     */
    function supply(
        address asset,
        uint256 amount,
        address onBehalfOf,
        uint16 referralCode
    ) external {
        IERC20(asset).safeTransferFrom(msg.sender, address(this), amount);
        userDeposits[onBehalfOf][asset] += amount;
        
        emit Supply(asset, onBehalfOf, amount, referralCode);
    }
    
    /**
     * @notice Mock withdraw function
     */
    function withdraw(
        address asset,
        uint256 amount,
        address to
    ) external returns (uint256) {
        uint256 userBalance = userDeposits[msg.sender][asset];
        uint256 withdrawAmount = amount > userBalance ? userBalance : amount;
        
        userDeposits[msg.sender][asset] -= withdrawAmount;
        IERC20(asset).safeTransfer(to, withdrawAmount);
        
        emit Withdraw(asset, msg.sender, withdrawAmount);
        
        return withdrawAmount;
    }
    
    /**
     * @notice Get user account data
     */
    function getUserAccountData(address user) 
        external 
        view 
        returns (
            uint256 totalCollateralBase,
            uint256 totalDebtBase,
            uint256 availableBorrowsBase,
            uint256 currentLiquidationThreshold,
            uint256 ltv,
            uint256 healthFactor
        ) 
    {
        // Simplified mock implementation
        return (
            1000000 * 10**6,  // 1M USDC collateral
            0,                 // No debt
            500000 * 10**6,   // 500k available to borrow
            8000,             // 80% liquidation threshold
            7500,             // 75% LTV
            type(uint256).max // Perfect health
        );
    }
}

/**
 * @title MockAaveAddressesProvider
 * @notice Mock Aave V3 AddressesProvider for testing
 */
contract MockAaveAddressesProvider {
    address public pool;
    
    constructor(address _pool) {
        pool = _pool;
    }
    
    function getPool() external view returns (address) {
        return pool;
    }
    
    function getPriceOracle() external pure returns (address) {
        return address(0); // Not needed for mock
    }
    
    function getACLManager() external pure returns (address) {
        return address(0); // Not needed for mock
    }
}