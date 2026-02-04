const hre = require("hardhat");
const fs = require("fs");
const path = require("path");

async function main() {
  console.log("🚀 Starting deployment of Betting Platform contracts...");
  
  const [deployer] = await ethers.getSigners();
  console.log("Deploying contracts with account:", deployer.address);
  console.log("Account balance:", (await deployer.getBalance()).toString());
  
  const deploymentInfo = {
    network: hre.network.name,
    deployer: deployer.address,
    timestamp: new Date().toISOString(),
    contracts: {}
  };
  
  // Deploy Mock USDC for testing
  console.log("\n📦 Deploying Mock USDC...");
  const MockUSDC = await ethers.getContractFactory("MockUSDC");
  const usdc = await MockUSDC.deploy();
  await usdc.deployed();
  console.log("✅ Mock USDC deployed to:", usdc.address);
  deploymentInfo.contracts.USDC = usdc.address;
  
  // Deploy Core Contracts
  console.log("\n📦 Deploying Core Contracts...");
  
  // 1. Deploy BettingPlatform
  console.log("Deploying BettingPlatform...");
  const BettingPlatform = await ethers.getContractFactory("BettingPlatform");
  const bettingPlatform = await BettingPlatform.deploy(
    usdc.address,
    deployer.address, // treasury
    deployer.address  // insurance fund
  );
  await bettingPlatform.deployed();
  console.log("✅ BettingPlatform deployed to:", bettingPlatform.address);
  deploymentInfo.contracts.BettingPlatform = bettingPlatform.address;
  
  // 2. Deploy PolymarketIntegration
  console.log("Deploying PolymarketIntegration...");
  const PolymarketIntegration = await ethers.getContractFactory("PolymarketIntegration");
  const polymarketIntegration = await PolymarketIntegration.deploy(bettingPlatform.address);
  await polymarketIntegration.deployed();
  console.log("✅ PolymarketIntegration deployed to:", polymarketIntegration.address);
  deploymentInfo.contracts.PolymarketIntegration = polymarketIntegration.address;
  
  // 3. Deploy MarketFactory
  console.log("Deploying MarketFactory...");
  const MarketFactory = await ethers.getContractFactory("MarketFactory");
  const marketFactory = await MarketFactory.deploy(deployer.address);
  await marketFactory.deployed();
  console.log("✅ MarketFactory deployed to:", marketFactory.address);
  deploymentInfo.contracts.MarketFactory = marketFactory.address;
  
  // Deploy Flash Betting Contracts
  console.log("\n📦 Deploying Flash Betting Contracts...");
  
  // 4. Deploy FlashBetting
  console.log("Deploying FlashBetting...");
  const FlashBetting = await ethers.getContractFactory("FlashBetting");
  const flashBetting = await FlashBetting.deploy(usdc.address, bettingPlatform.address);
  await flashBetting.deployed();
  console.log("✅ FlashBetting deployed to:", flashBetting.address);
  deploymentInfo.contracts.FlashBetting = flashBetting.address;
  
  // Deploy DeFi Contracts
  console.log("\n📦 Deploying DeFi Contracts...");
  
  // 5. Deploy Mock Aave for local testing
  console.log("Deploying Mock Aave...");
  const MockAave = await ethers.getContractFactory("MockAavePool");
  const mockAave = await MockAave.deploy();
  await mockAave.deployed();
  console.log("✅ Mock Aave deployed to:", mockAave.address);
  
  const MockAaveProvider = await ethers.getContractFactory("MockAaveAddressesProvider");
  const mockAaveProvider = await MockAaveProvider.deploy(mockAave.address);
  await mockAaveProvider.deployed();
  console.log("✅ Mock Aave Provider deployed to:", mockAaveProvider.address);
  
  // 6. Deploy LeverageVault
  console.log("Deploying LeverageVault...");
  const LeverageVault = await ethers.getContractFactory("LeverageVault");
  const leverageVault = await LeverageVault.deploy(
    usdc.address,
    mockAaveProvider.address,
    deployer.address, // treasury
    deployer.address  // insurance fund
  );
  await leverageVault.deployed();
  console.log("✅ LeverageVault deployed to:", leverageVault.address);
  deploymentInfo.contracts.LeverageVault = leverageVault.address;
  
  // 7. Deploy LiquidityPool
  console.log("Deploying LiquidityPool...");
  const LiquidityPool = await ethers.getContractFactory("LiquidityPool");
  const liquidityPool = await LiquidityPool.deploy(
    usdc.address,
    deployer.address, // treasury
    "BettingLP",
    "BLP"
  );
  await liquidityPool.deployed();
  console.log("✅ LiquidityPool deployed to:", liquidityPool.address);
  deploymentInfo.contracts.LiquidityPool = liquidityPool.address;
  
  // Configure Contract Relationships
  console.log("\n⚙️ Configuring contract relationships...");
  
  // Set Polymarket integration in BettingPlatform
  await bettingPlatform.setPolymarketIntegration(polymarketIntegration.address);
  console.log("✅ Set Polymarket integration in BettingPlatform");
  
  // Set LeverageVault in BettingPlatform
  await bettingPlatform.setLeverageVault(leverageVault.address);
  console.log("✅ Set LeverageVault in BettingPlatform");
  
  // Set BettingPlatform in MarketFactory
  await marketFactory.setBettingPlatform(bettingPlatform.address);
  console.log("✅ Set BettingPlatform in MarketFactory");
  
  // Set FlashBetting in MarketFactory
  await marketFactory.setFlashBetting(flashBetting.address);
  console.log("✅ Set FlashBetting in MarketFactory");
  
  // Set BettingPlatform in LeverageVault
  await leverageVault.setBettingPlatform(bettingPlatform.address);
  console.log("✅ Set BettingPlatform in LeverageVault");
  
  // Set FlashBetting in LeverageVault
  await leverageVault.setFlashBetting(flashBetting.address);
  console.log("✅ Set FlashBetting in LeverageVault");
  
  // Set BettingPlatform in LiquidityPool
  await liquidityPool.setBettingPlatform(bettingPlatform.address);
  console.log("✅ Set BettingPlatform in LiquidityPool");
  
  // Grant roles
  console.log("\n👤 Granting roles...");
  const OPERATOR_ROLE = await bettingPlatform.OPERATOR_ROLE();
  const KEEPER_ROLE = await bettingPlatform.KEEPER_ROLE();
  
  await bettingPlatform.grantRole(OPERATOR_ROLE, deployer.address);
  await bettingPlatform.grantRole(KEEPER_ROLE, deployer.address);
  console.log("✅ Granted OPERATOR and KEEPER roles to deployer");
  
  // Save deployment info
  const deploymentPath = path.join(__dirname, "../deployments");
  if (!fs.existsSync(deploymentPath)) {
    fs.mkdirSync(deploymentPath, { recursive: true });
  }
  
  const deploymentFile = path.join(deploymentPath, `${hre.network.name}-deployment.json`);
  fs.writeFileSync(deploymentFile, JSON.stringify(deploymentInfo, null, 2));
  console.log(`\n💾 Deployment info saved to: ${deploymentFile}`);
  
  // Generate ABI files
  console.log("\n📄 Generating ABI files...");
  const abiPath = path.join(__dirname, "../abi");
  if (!fs.existsSync(abiPath)) {
    fs.mkdirSync(abiPath, { recursive: true });
  }
  
  const contracts = {
    BettingPlatform: bettingPlatform,
    PolymarketIntegration: polymarketIntegration,
    MarketFactory: marketFactory,
    FlashBetting: flashBetting,
    LeverageVault: leverageVault,
    LiquidityPool: liquidityPool
  };
  
  for (const [name, contract] of Object.entries(contracts)) {
    const artifact = await hre.artifacts.readArtifact(name);
    const abiFile = path.join(abiPath, `${name}.json`);
    fs.writeFileSync(abiFile, JSON.stringify(artifact.abi, null, 2));
    console.log(`✅ ABI for ${name} saved to ${abiFile}`);
  }
  
  console.log("\n🎉 Deployment complete!");
  console.log("\n📊 Contract Addresses:");
  console.log("=======================");
  for (const [name, address] of Object.entries(deploymentInfo.contracts)) {
    console.log(`${name}: ${address}`);
  }
}

main()
  .then(() => process.exit(0))
  .catch((error) => {
    console.error(error);
    process.exit(1);
  });