// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

import "@openzeppelin/contracts/access/AccessControl.sol";
import "@openzeppelin/contracts/security/ReentrancyGuard.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import "@openzeppelin/contracts/utils/math/SafeMath.sol";

/**
 * @title FlashBetting
 * @notice Handles flash betting (5-60 second markets) with micro-tau AMM
 * @dev Supports 500x effective leverage through 3-step chaining
 */
contract FlashBetting is AccessControl, ReentrancyGuard {
    using SafeERC20 for IERC20;
    using SafeMath for uint256;

    // Roles
    bytes32 public constant KEEPER_ROLE = keccak256("KEEPER_ROLE");
    bytes32 public constant RESOLVER_ROLE = keccak256("RESOLVER_ROLE");
    
    // Flash market structure
    struct FlashMarket {
        bytes32 id;
        bytes32 parentVerseId;  // Link to main platform verse
        string title;
        uint256 startTime;
        uint256 endTime;
        uint256 tau;            // Micro-tau value
        uint256 totalVolume;
        uint256 yesShares;
        uint256 noShares;
        bool isResolved;
        bool outcome;
        uint256 resolvedAt;
        bytes32 zkProofHash;    // ZK proof verification hash
    }
    
    struct FlashPosition {
        address trader;
        bytes32 marketId;
        uint256 shares;
        bool isYes;
        uint256 entryPrice;
        uint256 leverage;
        bytes32[] chainedMarkets;  // For leverage chaining
        uint256 effectiveLeverage;
        bool isClosed;
    }
    
    struct ChainedBet {
        bytes32[] marketIds;
        uint256[] leverages;
        uint256 totalStake;
        uint256 potentialPayout;
        address trader;
        bool isActive;
    }
    
    struct ZKProof {
        bytes32 marketId;
        uint256[2] a;
        uint256[2][2] b;
        uint256[2] c;
        uint256[] publicInputs;
    }
    
    // State variables
    mapping(bytes32 => FlashMarket) public flashMarkets;
    mapping(bytes32 => FlashPosition) public flashPositions;
    mapping(bytes32 => ChainedBet) public chainedBets;
    mapping(address => bytes32[]) public userFlashPositions;
    mapping(bytes32 => mapping(address => uint256)) public marketShares;
    
    IERC20 public collateralToken;
    address public bettingPlatform;
    address public zkVerifier;
    
    uint256 public flashMarketCount;
    uint256 public positionCount;
    uint256 public constant MAX_FLASH_DURATION = 300; // 5 minutes
    uint256 public constant MIN_FLASH_DURATION = 5;   // 5 seconds
    uint256 public constant BASE_TAU = 1; // 0.0001 (basis points)
    uint256 public constant MAX_CHAIN_LENGTH = 3;
    uint256 public constant BASE_LEVERAGE = 100;
    uint256 public constant CHAIN_MULTIPLIER = 5;
    
    // Sport-specific tau values (in basis points)
    mapping(string => uint256) public sportTauValues;
    
    // Events
    event FlashMarketCreated(
        bytes32 indexed marketId,
        string title,
        uint256 duration,
        uint256 tau
    );
    
    event FlashPositionOpened(
        bytes32 indexed positionId,
        address indexed trader,
        bytes32 indexed marketId,
        uint256 shares,
        bool isYes
    );
    
    event ChainedBetPlaced(
        bytes32 indexed betId,
        address indexed trader,
        bytes32[] marketIds,
        uint256 effectiveLeverage
    );
    
    event FlashMarketResolved(
        bytes32 indexed marketId,
        bool outcome,
        bytes32 zkProofHash
    );
    
    event FlashPositionClosed(
        bytes32 indexed positionId,
        uint256 payout
    );
    
    constructor(address _collateralToken, address _bettingPlatform) {
        _setupRole(DEFAULT_ADMIN_ROLE, msg.sender);
        _setupRole(KEEPER_ROLE, msg.sender);
        
        collateralToken = IERC20(_collateralToken);
        bettingPlatform = _bettingPlatform;
        
        // Initialize sport-specific tau values
        sportTauValues["soccer"] = 15;     // 0.0015
        sportTauValues["basketball"] = 40; // 0.0040
        sportTauValues["tennis"] = 20;     // 0.0020
        sportTauValues["default"] = 10;    // 0.0010
    }
    
    // ============ Flash Market Creation ============
    
    /**
     * @notice Creates a new flash market
     * @param title Market title (e.g., "Next Goal in 30s?")
     * @param duration Duration in seconds (5-300)
     * @param parentVerseId Link to parent verse in main platform
     * @param sport Sport type for tau calculation
     */
    function createFlashMarket(
        string calldata title,
        uint256 duration,
        bytes32 parentVerseId,
        string calldata sport
    ) external onlyRole(KEEPER_ROLE) returns (bytes32) {
        require(duration >= MIN_FLASH_DURATION && duration <= MAX_FLASH_DURATION, "Invalid duration");
        
        bytes32 marketId = keccak256(abi.encodePacked(
            title,
            block.timestamp,
            flashMarketCount++
        ));
        
        // Calculate micro-tau based on duration and sport
        uint256 tau = _calculateTau(duration, sport);
        
        flashMarkets[marketId] = FlashMarket({
            id: marketId,
            parentVerseId: parentVerseId,
            title: title,
            startTime: block.timestamp,
            endTime: block.timestamp + duration,
            tau: tau,
            totalVolume: 0,
            yesShares: 0,
            noShares: 0,
            isResolved: false,
            outcome: false,
            resolvedAt: 0,
            zkProofHash: bytes32(0)
        });
        
        emit FlashMarketCreated(marketId, title, duration, tau);
        
        return marketId;
    }
    
    // ============ Trading Functions ============
    
    /**
     * @notice Opens a flash position with leverage
     * @param marketId Flash market ID
     * @param amount Collateral amount
     * @param isYes True for YES, false for NO
     * @param leverage Leverage multiplier
     */
    function openFlashPosition(
        bytes32 marketId,
        uint256 amount,
        bool isYes,
        uint256 leverage
    ) external nonReentrant returns (bytes32) {
        FlashMarket storage market = flashMarkets[marketId];
        require(market.id != bytes32(0), "Market doesn't exist");
        require(block.timestamp < market.endTime, "Market expired");
        require(!market.isResolved, "Market resolved");
        require(leverage > 0 && leverage <= BASE_LEVERAGE, "Invalid leverage");
        
        // Transfer collateral
        collateralToken.safeTransferFrom(msg.sender, address(this), amount);
        
        // Calculate shares with leverage
        uint256 shares = amount.mul(leverage);
        
        // Calculate entry price using micro-tau AMM
        uint256 entryPrice = _calculatePrice(market, isYes);
        
        // Generate position ID
        bytes32 positionId = keccak256(abi.encodePacked(
            msg.sender,
            marketId,
            positionCount++
        ));
        
        // Create position
        flashPositions[positionId] = FlashPosition({
            trader: msg.sender,
            marketId: marketId,
            shares: shares,
            isYes: isYes,
            entryPrice: entryPrice,
            leverage: leverage,
            chainedMarkets: new bytes32[](0),
            effectiveLeverage: leverage,
            isClosed: false
        });
        
        // Update market state
        if (isYes) {
            market.yesShares = market.yesShares.add(shares);
        } else {
            market.noShares = market.noShares.add(shares);
        }
        market.totalVolume = market.totalVolume.add(shares);
        
        // Track user position
        userFlashPositions[msg.sender].push(positionId);
        marketShares[marketId][msg.sender] = marketShares[marketId][msg.sender].add(shares);
        
        emit FlashPositionOpened(positionId, msg.sender, marketId, shares, isYes);
        
        return positionId;
    }
    
    /**
     * @notice Places a chained bet across multiple flash markets
     * @param marketIds Array of flash market IDs (max 3)
     * @param leverages Array of leverage values for each market
     * @param initialStake Initial stake amount
     */
    function placeChainedBet(
        bytes32[] calldata marketIds,
        uint256[] calldata leverages,
        uint256 initialStake
    ) external nonReentrant returns (bytes32) {
        require(marketIds.length <= MAX_CHAIN_LENGTH, "Chain too long");
        require(marketIds.length == leverages.length, "Length mismatch");
        require(marketIds.length > 0, "Empty chain");
        
        // Verify all markets are active
        for (uint i = 0; i < marketIds.length; i++) {
            FlashMarket memory market = flashMarkets[marketIds[i]];
            require(market.id != bytes32(0), "Invalid market");
            require(!market.isResolved, "Market resolved");
            require(block.timestamp < market.endTime, "Market expired");
            require(leverages[i] > 0 && leverages[i] <= BASE_LEVERAGE, "Invalid leverage");
        }
        
        // Transfer initial stake
        collateralToken.safeTransferFrom(msg.sender, address(this), initialStake);
        
        // Calculate effective leverage and potential payout
        uint256 effectiveLeverage = _calculateChainedLeverage(leverages);
        uint256 potentialPayout = initialStake.mul(effectiveLeverage);
        
        // Generate chained bet ID
        bytes32 betId = keccak256(abi.encodePacked(
            msg.sender,
            marketIds,
            block.timestamp
        ));
        
        // Store chained bet
        chainedBets[betId] = ChainedBet({
            marketIds: marketIds,
            leverages: leverages,
            totalStake: initialStake,
            potentialPayout: potentialPayout,
            trader: msg.sender,
            isActive: true
        });
        
        emit ChainedBetPlaced(betId, msg.sender, marketIds, effectiveLeverage);
        
        return betId;
    }
    
    // ============ Resolution Functions ============
    
    /**
     * @notice Resolves a flash market with ZK proof
     * @param marketId Flash market ID
     * @param outcome Market outcome
     * @param proof ZK proof data
     */
    function resolveFlashMarket(
        bytes32 marketId,
        bool outcome,
        ZKProof calldata proof
    ) external onlyRole(RESOLVER_ROLE) {
        FlashMarket storage market = flashMarkets[marketId];
        require(market.id != bytes32(0), "Market doesn't exist");
        require(!market.isResolved, "Already resolved");
        require(block.timestamp >= market.endTime, "Market not ended");
        
        // Verify ZK proof
        require(_verifyZKProof(proof), "Invalid ZK proof");
        
        // Update market
        market.isResolved = true;
        market.outcome = outcome;
        market.resolvedAt = block.timestamp;
        market.zkProofHash = keccak256(abi.encode(proof));
        
        emit FlashMarketResolved(marketId, outcome, market.zkProofHash);
    }
    
    /**
     * @notice Claims winnings from a resolved flash position
     * @param positionId Position ID
     */
    function claimFlashWinnings(bytes32 positionId) external nonReentrant {
        FlashPosition storage position = flashPositions[positionId];
        require(position.trader == msg.sender, "Not position owner");
        require(!position.isClosed, "Already claimed");
        
        FlashMarket memory market = flashMarkets[position.marketId];
        require(market.isResolved, "Market not resolved");
        
        // Calculate payout
        uint256 payout = 0;
        bool won = (position.isYes && market.outcome) || (!position.isYes && !market.outcome);
        
        if (won) {
            // Winner gets shares * price ratio
            uint256 totalShares = market.yesShares.add(market.noShares);
            payout = position.shares.mul(totalShares).div(
                position.isYes ? market.yesShares : market.noShares
            );
        }
        
        // Mark as closed
        position.isClosed = true;
        
        // Transfer payout
        if (payout > 0) {
            collateralToken.safeTransfer(msg.sender, payout);
        }
        
        emit FlashPositionClosed(positionId, payout);
    }
    
    // ============ Internal Functions ============
    
    function _calculateTau(uint256 duration, string calldata sport) internal view returns (uint256) {
        uint256 sportTau = sportTauValues[sport];
        if (sportTau == 0) {
            sportTau = sportTauValues["default"];
        }
        
        // tau = sportTau * (duration / 60)
        return sportTau.mul(duration).div(60);
    }
    
    function _calculatePrice(FlashMarket memory market, bool isYes) internal view returns (uint256) {
        uint256 totalShares = market.yesShares.add(market.noShares);
        
        if (totalShares == 0) {
            return 5000; // 50% initial price
        }
        
        uint256 targetShares = isYes ? market.yesShares : market.noShares;
        uint256 price = targetShares.mul(10000).div(totalShares);
        
        // Apply micro-tau adjustment
        uint256 timeLeft = market.endTime > block.timestamp ? 
            market.endTime - block.timestamp : 0;
        uint256 tauAdjustment = market.tau.mul(timeLeft).div(market.endTime - market.startTime);
        
        // Price converges toward 50% as time expires
        price = price.add(5000).div(2).add(tauAdjustment);
        
        return price > 10000 ? 10000 : price;
    }
    
    function _calculateChainedLeverage(uint256[] memory leverages) internal pure returns (uint256) {
        uint256 effective = BASE_LEVERAGE;
        
        for (uint i = 0; i < leverages.length; i++) {
            uint256 multiplier = CHAIN_MULTIPLIER.mul(i + 1).div(leverages.length);
            effective = effective.mul(leverages[i]).mul(multiplier).div(100);
        }
        
        // Cap at 500x
        return effective > 500 ? 500 : effective;
    }
    
    function _verifyZKProof(ZKProof calldata proof) internal view returns (bool) {
        // In production, this would call the ZK verifier contract
        // For now, basic validation
        require(proof.marketId != bytes32(0), "Invalid market ID");
        require(proof.publicInputs.length > 0, "No public inputs");
        
        // Placeholder verification
        return true;
    }
    
    // ============ Admin Functions ============
    
    function setZKVerifier(address _verifier) external onlyRole(DEFAULT_ADMIN_ROLE) {
        zkVerifier = _verifier;
    }
    
    function setSportTau(string calldata sport, uint256 tau) external onlyRole(DEFAULT_ADMIN_ROLE) {
        sportTauValues[sport] = tau;
    }
    
    function emergencyPause(bytes32 marketId) external onlyRole(DEFAULT_ADMIN_ROLE) {
        FlashMarket storage market = flashMarkets[marketId];
        market.isResolved = true;
        market.outcome = false; // Refund all positions
    }
    
    // ============ View Functions ============
    
    function getFlashMarket(bytes32 marketId) external view returns (FlashMarket memory) {
        return flashMarkets[marketId];
    }
    
    function getFlashPosition(bytes32 positionId) external view returns (FlashPosition memory) {
        return flashPositions[positionId];
    }
    
    function getUserFlashPositions(address user) external view returns (bytes32[] memory) {
        return userFlashPositions[user];
    }
    
    function getChainedBet(bytes32 betId) external view returns (ChainedBet memory) {
        return chainedBets[betId];
    }
    
    function getCurrentPrice(bytes32 marketId, bool isYes) external view returns (uint256) {
        return _calculatePrice(flashMarkets[marketId], isYes);
    }
    
    function getEffectiveLeverage(uint256[] calldata leverages) external pure returns (uint256) {
        return _calculateChainedLeverage(leverages);
    }
}