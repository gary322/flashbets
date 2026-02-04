const { ethers } = require('ethers');
const backend = require('./backend_integration');

async function grantRoles() {
  console.log('\n🔐 GRANTING REQUIRED ROLES FOR FLASH BETTING');
  console.log('=' .repeat(50));
  
  const { provider, signer } = backend.initPolygonProvider();
  const signerAddress = await signer.getAddress();
  
  console.log(`\n📍 Granting roles to: ${signerAddress}`);
  
  try {
    // 1. Grant KEEPER_ROLE for FlashBetting
    console.log('\n1️⃣ Granting KEEPER_ROLE for FlashBetting...');
    const flashBetting = backend.getPolygonContract('FlashBetting', signer);
    
    // Get KEEPER_ROLE hash
    const KEEPER_ROLE = await flashBetting.KEEPER_ROLE();
    console.log(`   KEEPER_ROLE hash: ${KEEPER_ROLE}`);
    
    // Check if already has role
    const hasKeeperRole = await flashBetting.hasRole(KEEPER_ROLE, signerAddress);
    
    if (!hasKeeperRole) {
      // Grant the role
      const tx1 = await flashBetting.grantRole(KEEPER_ROLE, signerAddress);
      await tx1.wait();
      console.log(`   ✅ KEEPER_ROLE granted to ${signerAddress}`);
    } else {
      console.log(`   ✅ Already has KEEPER_ROLE`);
    }
    
    // 2. Grant RESOLVER_ROLE for FlashBetting
    console.log('\n2️⃣ Granting RESOLVER_ROLE for FlashBetting...');
    const RESOLVER_ROLE = await flashBetting.RESOLVER_ROLE();
    console.log(`   RESOLVER_ROLE hash: ${RESOLVER_ROLE}`);
    
    const hasResolverRole = await flashBetting.hasRole(RESOLVER_ROLE, signerAddress);
    
    if (!hasResolverRole) {
      const tx2 = await flashBetting.grantRole(RESOLVER_ROLE, signerAddress);
      await tx2.wait();
      console.log(`   ✅ RESOLVER_ROLE granted to ${signerAddress}`);
    } else {
      console.log(`   ✅ Already has RESOLVER_ROLE`);
    }
    
    // 3. Grant KEEPER_ROLE for BettingPlatform (for market resolution)
    console.log('\n3️⃣ Granting KEEPER_ROLE for BettingPlatform...');
    const bettingPlatform = backend.getPolygonContract('BettingPlatform', signer);
    
    const BP_KEEPER_ROLE = await bettingPlatform.KEEPER_ROLE();
    const hasBPKeeperRole = await bettingPlatform.hasRole(BP_KEEPER_ROLE, signerAddress);
    
    if (!hasBPKeeperRole) {
      const tx3 = await bettingPlatform.grantRole(BP_KEEPER_ROLE, signerAddress);
      await tx3.wait();
      console.log(`   ✅ KEEPER_ROLE granted for BettingPlatform`);
    } else {
      console.log(`   ✅ Already has KEEPER_ROLE for BettingPlatform`);
    }
    
    // 4. Grant MARKET_CREATOR_ROLE for MarketFactory
    console.log('\n4️⃣ Granting MARKET_CREATOR_ROLE for MarketFactory...');
    const marketFactory = backend.getPolygonContract('MarketFactory', signer);
    
    const MF_CREATOR_ROLE = await marketFactory.MARKET_CREATOR_ROLE();
    const hasMFCreatorRole = await marketFactory.hasRole(MF_CREATOR_ROLE, signerAddress);
    
    if (!hasMFCreatorRole) {
      const tx4 = await marketFactory.grantRole(MF_CREATOR_ROLE, signerAddress);
      await tx4.wait();
      console.log(`   ✅ MARKET_CREATOR_ROLE granted for MarketFactory`);
    } else {
      console.log(`   ✅ Already has MARKET_CREATOR_ROLE for MarketFactory`);
    }
    
    // 5. Mint some USDC for testing
    console.log('\n5️⃣ Minting USDC for testing...');
    const usdc = new ethers.Contract(
      backend.addresses.polygon.USDC,
      [
        'function mint(address,uint256) returns (bool)',
        'function balanceOf(address) view returns (uint256)'
      ],
      signer
    );
    
    const currentBalance = await usdc.balanceOf(signerAddress);
    const requiredBalance = ethers.utils.parseUnits('100000', 6); // 100k USDC
    
    if (currentBalance.lt(requiredBalance)) {
      const mintAmount = requiredBalance.sub(currentBalance);
      const mintTx = await usdc.mint(signerAddress, mintAmount);
      await mintTx.wait();
      console.log(`   ✅ Minted ${ethers.utils.formatUnits(mintAmount, 6)} USDC`);
    } else {
      console.log(`   ✅ Already has ${ethers.utils.formatUnits(currentBalance, 6)} USDC`);
    }
    
    // Verify all roles
    console.log('\n' + '=' .repeat(50));
    console.log('✅ ROLE VERIFICATION');
    console.log('=' .repeat(50));
    
    const finalChecks = {
      'FlashBetting KEEPER': await flashBetting.hasRole(KEEPER_ROLE, signerAddress),
      'FlashBetting RESOLVER': await flashBetting.hasRole(RESOLVER_ROLE, signerAddress),
      'BettingPlatform KEEPER': await bettingPlatform.hasRole(BP_KEEPER_ROLE, signerAddress),
      'MarketFactory CREATOR': await marketFactory.hasRole(MF_CREATOR_ROLE, signerAddress),
      'USDC Balance': ethers.utils.formatUnits(await usdc.balanceOf(signerAddress), 6) + ' USDC'
    };
    
    for (const [check, result] of Object.entries(finalChecks)) {
      const icon = (result === true || result.includes('USDC')) ? '✅' : '❌';
      console.log(`${icon} ${check}: ${result}`);
    }
    
    console.log('\n🎉 All roles granted successfully!');
    console.log('You can now create flash markets and manage positions.');
    
  } catch (error) {
    console.error('\n❌ Error granting roles:', error.message);
    throw error;
  }
}

// Run the script
grantRoles()
  .then(() => {
    console.log('\n✅ Role setup complete');
    process.exit(0);
  })
  .catch(error => {
    console.error('Fatal error:', error);
    process.exit(1);
  });