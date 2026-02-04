// Hardhat configuration for local testing
require("@nomiclabs/hardhat-ethers");

module.exports = {
  solidity: {
    version: "0.8.19",
    settings: {
      optimizer: {
        enabled: true,
        runs: 200
      }
    }
  },
  networks: {
    hardhat: {
      chainId: 31337,
      initialBaseFeePerGas: 0,
      gas: 30000000,
      blockGasLimit: 30000000,
      allowUnlimitedContractSize: true,
      accounts: {
        mnemonic: "test test test test test test test test test test test junk",
        count: 100,
        accountsBalance: "100000000000000000000000"
      }
    },
    localhost: {
      url: "http://127.0.0.1:8545",
      chainId: 31337,
      timeout: 60000
    }
  },
  paths: {
    sources: "./contracts/polygon",
    tests: "./test",
    cache: "./cache",
    artifacts: "./artifacts"
  }
};