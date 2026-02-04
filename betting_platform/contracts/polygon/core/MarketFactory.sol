// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

import "@openzeppelin/contracts/access/AccessControl.sol";
import "@openzeppelin/contracts/proxy/Clones.sol";
import "@openzeppelin/contracts/utils/Counters.sol";

/**
 * @title MarketFactory
 * @notice Factory contract for creating and managing betting markets
 * @dev Uses minimal proxy pattern for gas-efficient market deployment
 */
contract MarketFactory is AccessControl {
    using Counters for Counters.Counter;
    
    // Roles
    bytes32 public constant MARKET_CREATOR_ROLE = keccak256("MARKET_CREATOR_ROLE");
    bytes32 public constant ORACLE_ROLE = keccak256("ORACLE_ROLE");
    
    // Market types
    enum MarketType {
        BINARY,         // Yes/No
        CATEGORICAL,    // Multiple outcomes
        SCALAR,         // Range-based
        FLASH,          // 5-60 second markets
        PERPETUAL       // No expiry
    }
    
    // Market status
    enum MarketStatus {
        PENDING,
        ACTIVE,
        SUSPENDED,
        RESOLVED,
        CANCELLED
    }
    
    // Market template
    struct MarketTemplate {
        address implementation;
        MarketType marketType;
        uint256 minDuration;
        uint256 maxDuration;
        uint256 minStake;
        uint256 maxStake;
        uint256 creationFee;
        bool isActive;
    }
    
    // Market metadata
    struct Market {
        bytes32 id;
        address marketAddress;
        MarketType marketType;
        string title;
        string description;
        string category;
        address creator;
        address oracle;
        uint256 createdAt;
        uint256 expiryTime;
        MarketStatus status;
        bytes32 conditionId;    // For Polymarket integration
        string[] outcomes;
        uint256 totalVolume;
        uint256 resolutionTime;
        uint256[] finalPrices;
    }
    
    // State variables
    Counters.Counter private marketIdCounter;
    mapping(bytes32 => Market) public markets;
    mapping(MarketType => MarketTemplate) public templates;
    mapping(address => bytes32[]) public creatorMarkets;
    mapping(string => bytes32[]) public categoryMarkets;
    mapping(address => bool) public verifiedOracles;
    
    bytes32[] public allMarkets;
    address public bettingPlatform;
    address public flashBetting;
    address public treasury;
    
    uint256 public totalMarketsCreated;
    uint256 public totalVolumeAllMarkets;
    
    // Events
    event MarketCreated(
        bytes32 indexed marketId,
        address indexed creator,
        MarketType marketType,
        string title,
        uint256 expiryTime
    );
    
    event MarketResolved(
        bytes32 indexed marketId,
        uint256[] finalPrices,
        uint256 resolutionTime
    );
    
    event MarketSuspended(bytes32 indexed marketId, string reason);
    event MarketReactivated(bytes32 indexed marketId);
    event MarketCancelled(bytes32 indexed marketId, string reason);
    event TemplateUpdated(MarketType indexed marketType, address implementation);
    event OracleVerified(address indexed oracle, bool verified);
    
    constructor(address _treasury) {
        _setupRole(DEFAULT_ADMIN_ROLE, msg.sender);
        _setupRole(MARKET_CREATOR_ROLE, msg.sender);
        treasury = _treasury;
    }
    
    // ============ Market Creation ============
    
    /**
     * @notice Creates a new binary market
     * @param title Market title
     * @param description Detailed description
     * @param category Market category
     * @param expiryTime Expiry timestamp
     * @param oracle Oracle address for resolution
     */
    function createBinaryMarket(
        string calldata title,
        string calldata description,
        string calldata category,
        uint256 expiryTime,
        address oracle
    ) external onlyRole(MARKET_CREATOR_ROLE) returns (bytes32) {
        require(templates[MarketType.BINARY].isActive, "Template not active");
        require(expiryTime > block.timestamp, "Invalid expiry");
        require(verifiedOracles[oracle], "Unverified oracle");
        
        MarketTemplate memory template = templates[MarketType.BINARY];
        uint256 duration = expiryTime - block.timestamp;
        require(duration >= template.minDuration && duration <= template.maxDuration, "Invalid duration");
        
        // Generate market ID
        bytes32 marketId = _generateMarketId(title, msg.sender);
        
        // Deploy market using minimal proxy
        address marketAddress = Clones.clone(template.implementation);
        
        // Initialize market
        _initializeBinaryMarket(marketAddress, marketId, title, expiryTime, oracle);
        
        // Create market record
        string[] memory outcomes = new string[](2);
        outcomes[0] = "Yes";
        outcomes[1] = "No";
        
        markets[marketId] = Market({
            id: marketId,
            marketAddress: marketAddress,
            marketType: MarketType.BINARY,
            title: title,
            description: description,
            category: category,
            creator: msg.sender,
            oracle: oracle,
            createdAt: block.timestamp,
            expiryTime: expiryTime,
            status: MarketStatus.ACTIVE,
            conditionId: bytes32(0),
            outcomes: outcomes,
            totalVolume: 0,
            resolutionTime: 0,
            finalPrices: new uint256[](0)
        });
        
        // Update mappings
        allMarkets.push(marketId);
        creatorMarkets[msg.sender].push(marketId);
        categoryMarkets[category].push(marketId);
        totalMarketsCreated++;
        
        emit MarketCreated(marketId, msg.sender, MarketType.BINARY, title, expiryTime);
        
        return marketId;
    }
    
    /**
     * @notice Creates a categorical market with multiple outcomes
     * @param title Market title
     * @param description Detailed description
     * @param category Market category
     * @param outcomes Array of possible outcomes
     * @param expiryTime Expiry timestamp
     * @param oracle Oracle address
     */
    function createCategoricalMarket(
        string calldata title,
        string calldata description,
        string calldata category,
        string[] calldata outcomes,
        uint256 expiryTime,
        address oracle
    ) external onlyRole(MARKET_CREATOR_ROLE) returns (bytes32) {
        require(templates[MarketType.CATEGORICAL].isActive, "Template not active");
        require(outcomes.length >= 2 && outcomes.length <= 10, "Invalid outcomes count");
        require(expiryTime > block.timestamp, "Invalid expiry");
        require(verifiedOracles[oracle], "Unverified oracle");
        
        bytes32 marketId = _generateMarketId(title, msg.sender);
        address marketAddress = Clones.clone(templates[MarketType.CATEGORICAL].implementation);
        
        _initializeCategoricalMarket(marketAddress, marketId, title, outcomes, expiryTime, oracle);
        
        markets[marketId] = Market({
            id: marketId,
            marketAddress: marketAddress,
            marketType: MarketType.CATEGORICAL,
            title: title,
            description: description,
            category: category,
            creator: msg.sender,
            oracle: oracle,
            createdAt: block.timestamp,
            expiryTime: expiryTime,
            status: MarketStatus.ACTIVE,
            conditionId: bytes32(0),
            outcomes: outcomes,
            totalVolume: 0,
            resolutionTime: 0,
            finalPrices: new uint256[](0)
        });
        
        allMarkets.push(marketId);
        creatorMarkets[msg.sender].push(marketId);
        categoryMarkets[category].push(marketId);
        totalMarketsCreated++;
        
        emit MarketCreated(marketId, msg.sender, MarketType.CATEGORICAL, title, expiryTime);
        
        return marketId;
    }
    
    /**
     * @notice Creates a flash market (5-60 seconds)
     * @param title Market title
     * @param duration Duration in seconds
     * @param sport Sport type for tau calculation
     */
    function createFlashMarket(
        string calldata title,
        uint256 duration,
        string calldata sport
    ) external returns (bytes32) {
        require(flashBetting != address(0), "Flash betting not set");
        require(duration >= 5 && duration <= 60, "Invalid flash duration");
        
        // Delegate to FlashBetting contract
        bytes32 parentVerseId = _generateMarketId(title, msg.sender);
        
        (bool success, bytes memory data) = flashBetting.call(
            abi.encodeWithSignature(
                "createFlashMarket(string,uint256,bytes32,string)",
                title,
                duration,
                parentVerseId,
                sport
            )
        );
        require(success, "Flash market creation failed");
        
        bytes32 marketId = abi.decode(data, (bytes32));
        
        // Track in factory
        string[] memory outcomes = new string[](2);
        outcomes[0] = "Yes";
        outcomes[1] = "No";
        
        markets[marketId] = Market({
            id: marketId,
            marketAddress: flashBetting,
            marketType: MarketType.FLASH,
            title: title,
            description: string(abi.encodePacked("Flash: ", title)),
            category: "flash",
            creator: msg.sender,
            oracle: flashBetting,
            createdAt: block.timestamp,
            expiryTime: block.timestamp + duration,
            status: MarketStatus.ACTIVE,
            conditionId: bytes32(0),
            outcomes: outcomes,
            totalVolume: 0,
            resolutionTime: 0,
            finalPrices: new uint256[](0)
        });
        
        allMarkets.push(marketId);
        creatorMarkets[msg.sender].push(marketId);
        categoryMarkets["flash"].push(marketId);
        totalMarketsCreated++;
        
        emit MarketCreated(marketId, msg.sender, MarketType.FLASH, title, block.timestamp + duration);
        
        return marketId;
    }
    
    // ============ Market Resolution ============
    
    /**
     * @notice Resolves a market with final prices
     * @param marketId Market identifier
     * @param finalPrices Final prices for each outcome
     */
    function resolveMarket(
        bytes32 marketId,
        uint256[] calldata finalPrices
    ) external onlyRole(ORACLE_ROLE) {
        Market storage market = markets[marketId];
        require(market.status == MarketStatus.ACTIVE, "Market not active");
        require(block.timestamp >= market.expiryTime, "Market not expired");
        require(finalPrices.length == market.outcomes.length, "Invalid prices length");
        
        // Validate prices sum to 1 (10000 basis points)
        uint256 sum = 0;
        for (uint i = 0; i < finalPrices.length; i++) {
            sum += finalPrices[i];
        }
        require(sum == 10000, "Prices must sum to 100%");
        
        // Update market
        market.status = MarketStatus.RESOLVED;
        market.finalPrices = finalPrices;
        market.resolutionTime = block.timestamp;
        
        // Call market contract to settle
        (bool success,) = market.marketAddress.call(
            abi.encodeWithSignature("settle(uint256[])", finalPrices)
        );
        require(success, "Settlement failed");
        
        emit MarketResolved(marketId, finalPrices, block.timestamp);
    }
    
    // ============ Market Management ============
    
    /**
     * @notice Suspends a market
     * @param marketId Market identifier
     * @param reason Suspension reason
     */
    function suspendMarket(bytes32 marketId, string calldata reason) 
        external 
        onlyRole(DEFAULT_ADMIN_ROLE) 
    {
        Market storage market = markets[marketId];
        require(market.status == MarketStatus.ACTIVE, "Market not active");
        
        market.status = MarketStatus.SUSPENDED;
        
        // Pause trading on market contract
        (bool success,) = market.marketAddress.call(
            abi.encodeWithSignature("pause()")
        );
        require(success, "Pause failed");
        
        emit MarketSuspended(marketId, reason);
    }
    
    /**
     * @notice Reactivates a suspended market
     * @param marketId Market identifier
     */
    function reactivateMarket(bytes32 marketId) 
        external 
        onlyRole(DEFAULT_ADMIN_ROLE) 
    {
        Market storage market = markets[marketId];
        require(market.status == MarketStatus.SUSPENDED, "Market not suspended");
        require(block.timestamp < market.expiryTime, "Market expired");
        
        market.status = MarketStatus.ACTIVE;
        
        // Unpause trading
        (bool success,) = market.marketAddress.call(
            abi.encodeWithSignature("unpause()")
        );
        require(success, "Unpause failed");
        
        emit MarketReactivated(marketId);
    }
    
    // ============ Template Management ============
    
    function setMarketTemplate(
        MarketType marketType,
        address implementation,
        uint256 minDuration,
        uint256 maxDuration,
        uint256 minStake,
        uint256 maxStake,
        uint256 creationFee
    ) external onlyRole(DEFAULT_ADMIN_ROLE) {
        templates[marketType] = MarketTemplate({
            implementation: implementation,
            marketType: marketType,
            minDuration: minDuration,
            maxDuration: maxDuration,
            minStake: minStake,
            maxStake: maxStake,
            creationFee: creationFee,
            isActive: true
        });
        
        emit TemplateUpdated(marketType, implementation);
    }
    
    // ============ Oracle Management ============
    
    function verifyOracle(address oracle, bool verified) 
        external 
        onlyRole(DEFAULT_ADMIN_ROLE) 
    {
        verifiedOracles[oracle] = verified;
        emit OracleVerified(oracle, verified);
    }
    
    // ============ Internal Functions ============
    
    function _generateMarketId(string memory title, address creator) 
        internal 
        returns (bytes32) 
    {
        marketIdCounter.increment();
        return keccak256(abi.encodePacked(
            title,
            creator,
            marketIdCounter.current(),
            block.timestamp
        ));
    }
    
    function _initializeBinaryMarket(
        address marketAddress,
        bytes32 marketId,
        string memory title,
        uint256 expiryTime,
        address oracle
    ) internal {
        (bool success,) = marketAddress.call(
            abi.encodeWithSignature(
                "initialize(bytes32,string,uint256,address)",
                marketId,
                title,
                expiryTime,
                oracle
            )
        );
        require(success, "Market initialization failed");
    }
    
    function _initializeCategoricalMarket(
        address marketAddress,
        bytes32 marketId,
        string memory title,
        string[] memory outcomes,
        uint256 expiryTime,
        address oracle
    ) internal {
        (bool success,) = marketAddress.call(
            abi.encodeWithSignature(
                "initialize(bytes32,string,string[],uint256,address)",
                marketId,
                title,
                outcomes,
                expiryTime,
                oracle
            )
        );
        require(success, "Market initialization failed");
    }
    
    // ============ View Functions ============
    
    function getMarket(bytes32 marketId) external view returns (Market memory) {
        return markets[marketId];
    }
    
    function getMarketsByCreator(address creator) external view returns (bytes32[] memory) {
        return creatorMarkets[creator];
    }
    
    function getMarketsByCategory(string calldata category) external view returns (bytes32[] memory) {
        return categoryMarkets[category];
    }
    
    function getAllMarkets() external view returns (bytes32[] memory) {
        return allMarkets;
    }
    
    function getActiveMarkets() external view returns (bytes32[] memory) {
        uint256 count = 0;
        for (uint i = 0; i < allMarkets.length; i++) {
            if (markets[allMarkets[i]].status == MarketStatus.ACTIVE) {
                count++;
            }
        }
        
        bytes32[] memory activeMarkets = new bytes32[](count);
        uint256 index = 0;
        for (uint i = 0; i < allMarkets.length; i++) {
            if (markets[allMarkets[i]].status == MarketStatus.ACTIVE) {
                activeMarkets[index++] = allMarkets[i];
            }
        }
        
        return activeMarkets;
    }
    
    function getMarketTemplate(MarketType marketType) external view returns (MarketTemplate memory) {
        return templates[marketType];
    }
    
    // ============ Admin Functions ============
    
    function setBettingPlatform(address _platform) external onlyRole(DEFAULT_ADMIN_ROLE) {
        bettingPlatform = _platform;
    }
    
    function setFlashBetting(address _flash) external onlyRole(DEFAULT_ADMIN_ROLE) {
        flashBetting = _flash;
    }
    
    function setTreasury(address _treasury) external onlyRole(DEFAULT_ADMIN_ROLE) {
        treasury = _treasury;
    }
}