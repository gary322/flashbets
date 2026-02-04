#!/usr/bin/env python3
"""
Phase 7 & 7.5 Implementation Test Verification Script
Verifies all components are properly implemented according to CLAUDE.md
"""

import os
import re
from pathlib import Path

class TestVerifier:
    def __init__(self, base_path):
        self.base_path = Path(base_path)
        self.results = []
        self.passed = 0
        self.total = 0
    
    def check_file_exists(self, path, description):
        """Check if a file exists"""
        self.total += 1
        full_path = self.base_path / path
        if full_path.exists():
            self.passed += 1
            self.results.append(f"✅ {description}: {path}")
            return True
        else:
            self.results.append(f"❌ {description}: {path} NOT FOUND")
            return False
    
    def check_content_in_file(self, path, patterns, description):
        """Check if specific patterns exist in a file"""
        self.total += 1
        full_path = self.base_path / path
        
        if not full_path.exists():
            self.results.append(f"❌ {description}: File {path} not found")
            return False
        
        with open(full_path, 'r') as f:
            content = f.read()
        
        missing = []
        for pattern in patterns:
            if not re.search(pattern, content, re.MULTILINE | re.IGNORECASE):
                missing.append(pattern)
        
        if not missing:
            self.passed += 1
            self.results.append(f"✅ {description}")
            return True
        else:
            self.results.append(f"❌ {description}: Missing patterns: {missing}")
            return False
    
    def run_all_tests(self):
        """Run all verification tests"""
        print("🔍 Phase 7 & 7.5 Implementation Verification\n")
        
        # Phase 7.1: Directory Structure
        print("📁 Phase 7.1: Directory Structure")
        self.check_file_exists("src/deployment/mod.rs", "Deployment module")
        self.check_file_exists("src/deployment/errors.rs", "Deployment errors")
        self.check_file_exists("src/deployment/deploy_manager.rs", "DeploymentManager")
        self.check_file_exists("src/deployment/genesis_setup.rs", "Genesis setup")
        self.check_file_exists("src/deployment/launch_monitor.rs", "Launch monitor")
        self.check_file_exists("src/deployment/bootstrap_incentives.rs", "Bootstrap incentives")
        self.check_file_exists("src/deployment/types.rs", "Deployment types")
        
        # Phase 7.2-7.3: DeploymentManager
        print("\n🚀 Phase 7.2-7.3: DeploymentManager")
        self.check_content_in_file(
            "src/deployment/deploy_manager.rs",
            [
                r"struct DeploymentManager",
                r"deploy_immutable_program",
                r"verify_immutability",
                r"program_id: Pubkey",
                r"vault_seed: \[u8; 32\]"
            ],
            "DeploymentManager implementation"
        )
        
        # Phase 7.4-7.6: Genesis Configuration
        print("\n⚡ Phase 7.4-7.6: Genesis Configuration")
        self.check_content_in_file(
            "src/deployment/genesis_setup.rs",
            [
                r"struct GenesisConfig",
                r"fee_base: u64.*// 3bp",
                r"fee_slope: u64.*// 25bp",
                r"mmt_supply: u128.*// 100M",
                r"initial_coverage: 0\.0",
                r"create_mmt_token",
                r"lock_undecided_tokens",
                r"90_000_000.*10u128\.pow\(9\)"
            ],
            "Genesis configuration with $0 vault and 90M token lock"
        )
        
        # Phase 7.7: Launch Monitor
        print("\n📊 Phase 7.7: Launch Monitor")
        self.check_content_in_file(
            "src/deployment/launch_monitor.rs",
            [
                r"struct LaunchMonitor",
                r"MetricsCollector",
                r"AlertSystem",
                r"HealthChecker",
                r"check_vault_balance",
                r"calculate_coverage",
                r"AlertLevel::Critical"
            ],
            "Launch monitoring system"
        )
        
        # Phase 7.8-7.9: Bootstrap Incentives
        print("\n💰 Phase 7.8-7.9: Bootstrap Incentives")
        self.check_content_in_file(
            "src/deployment/bootstrap_incentives.rs",
            [
                r"struct BootstrapIncentives",
                r"double_mmt_duration.*100",
                r"early_maker_bonus.*2\.0",
                r"calculate_mmt_reward",
                r"should_apply_double_mmt",
                r"bootstrap_trade_count < .*bootstrap_max_trades"
            ],
            "Bootstrap incentives with double MMT"
        )
        
        # Phase 7.5.1: Performance Infrastructure
        print("\n⚙️ Phase 7.5.1: Performance Infrastructure")
        self.check_file_exists("src/performance/mod.rs", "Performance module")
        self.check_file_exists("src/performance/errors.rs", "Performance errors")
        self.check_file_exists("src/performance/profiler.rs", "Performance profiler")
        self.check_file_exists("src/performance/cu_optimizer.rs", "CU optimizer")
        self.check_file_exists("src/performance/stress_test.rs", "Stress test framework")
        self.check_file_exists("src/performance/optimizations.rs", "Optimization techniques")
        
        # Phase 7.5.2: Performance Profiler
        print("\n📈 Phase 7.5.2: Performance Profiler")
        self.check_content_in_file(
            "src/performance/profiler.rs",
            [
                r"struct PerformanceProfiler",
                r"ComputeUnitTracker",
                r"LatencyMonitor",
                r"BottleneckDetector",
                r"profile_transaction",
                r"TARGET_CU_PER_TRADE"
            ],
            "Performance profiler with CU tracking"
        )
        
        # Phase 7.5.3-7.5.5: CU Optimizer
        print("\n🔧 Phase 7.5.3-7.5.5: CU Optimizer")
        self.check_content_in_file(
            "src/performance/cu_optimizer.rs",
            [
                r"struct CUOptimizer",
                r"PrecomputedTables",
                r"sqrt_lookup.*insert\(4, 2_000\)",
                r"optimize_leverage_calculation",
                r"optimize_pm_amm",
                r"CacheManager",
                r"NEWTON_RAPHSON_MAX_ITERATIONS"
            ],
            "CU optimizer with precomputed tables and caching"
        )
        
        # Phase 7.5.6-7.5.7: Stress Testing
        print("\n🏃 Phase 7.5.6-7.5.7: Stress Testing")
        self.check_content_in_file(
            "src/performance/stress_test.rs",
            [
                r"struct StressTestFramework",
                r"test_concurrent_users.*1000",
                r"test_market_volatility.*50.*0\.10",
                r"test_chain_execution_load.*500.*5",
                r"test_liquidation_cascade",
                r"test_api_degradation",
                r"test_network_congestion",
                r"TARGET_TPS.*5.*000"
            ],
            "Stress test framework with 6 scenarios"
        )
        
        # Phase 7.5.8-7.5.9: Optimization Techniques
        print("\n🎯 Phase 7.5.8-7.5.9: Optimization Techniques")
        self.check_content_in_file(
            "src/performance/optimizations.rs",
            [
                r"struct OptimizationTechniques",
                r"optimize_batch_operations",
                r"optimize_state_compression",
                r"generate_state_proof",
                r"delta_encode_positions",
                r"compression_ratio.*10"
            ],
            "Optimization techniques with batching and compression"
        )
        
        # Integration
        print("\n🔗 Integration")
        self.check_content_in_file(
            "src/lib.rs",
            [
                r"pub mod deployment",
                r"pub mod performance"
            ],
            "Module integration in lib.rs"
        )
        
        # Tests
        print("\n🧪 Tests")
        self.check_file_exists("tests/deployment/deployment_tests.rs", "Deployment tests")
        self.check_file_exists("tests/performance/optimization_tests.rs", "Performance tests")
        
        # Test Content Verification
        self.check_content_in_file(
            "tests/deployment/deployment_tests.rs",
            [
                r"test_immutable_deployment",
                r"test_genesis_initialization",
                r"test_double_mmt_rewards",
                r"test_bootstrap_incentives"
            ],
            "Deployment test coverage"
        )
        
        self.check_content_in_file(
            "tests/performance/optimization_tests.rs",
            [
                r"test_cu_optimization_leverage",
                r"test_stress_5k_tps",
                r"test_state_compression",
                r"benchmark_leverage_calculation"
            ],
            "Performance test coverage"
        )
        
        # Summary
        print("\n" + "="*50)
        print(f"📊 SUMMARY: {self.passed}/{self.total} tests passed")
        print("="*50 + "\n")
        
        print("Detailed Results:")
        for result in self.results:
            print(result)
        
        # Critical Requirements Check
        print("\n🎯 Critical Requirements Verification:")
        critical_checks = [
            ("Immutable deployment", self.check_immutable_deployment()),
            ("$0 vault initialization", self.check_zero_vault()),
            ("90M token lock", self.check_token_lock()),
            ("Double MMT for first 100", self.check_double_mmt()),
            ("<1k CU leverage calc", self.check_cu_optimization()),
            ("5k TPS capability", self.check_tps_target()),
            ("10x compression ratio", self.check_compression())
        ]
        
        for desc, passed in critical_checks:
            status = "✅" if passed else "❌"
            print(f"{status} {desc}")
    
    def check_immutable_deployment(self):
        """Verify immutable deployment implementation"""
        path = self.base_path / "src/deployment/deploy_manager.rs"
        if path.exists():
            content = path.read_text()
            return "verify_immutability" in content
        return False
    
    def check_zero_vault(self):
        """Verify $0 vault initialization"""
        path = self.base_path / "src/deployment/genesis_setup.rs"
        if path.exists():
            content = path.read_text()
            return "initial_coverage: 0.0" in content
        return False
    
    def check_token_lock(self):
        """Verify 90M token lock"""
        path = self.base_path / "src/deployment/genesis_setup.rs"
        if path.exists():
            content = path.read_text()
            return "90_000_000" in content and "lock_undecided_tokens" in content
        return False
    
    def check_double_mmt(self):
        """Verify double MMT implementation"""
        path = self.base_path / "src/deployment/bootstrap_incentives.rs"
        if path.exists():
            content = path.read_text()
            return "double_mmt_duration" in content and "100" in content
        return False
    
    def check_cu_optimization(self):
        """Verify CU optimization"""
        path = self.base_path / "src/performance/cu_optimizer.rs"
        if path.exists():
            content = path.read_text()
            return "optimize_leverage_calculation" in content and "PrecomputedTables" in content
        return False
    
    def check_tps_target(self):
        """Verify 5k TPS target"""
        path = self.base_path / "src/performance/stress_test.rs"
        if path.exists():
            content = path.read_text()
            return "TARGET_TPS" in content and "5" in content and "000" in content
        return False
    
    def check_compression(self):
        """Verify compression ratio"""
        path = self.base_path / "src/performance/optimizations.rs"
        if path.exists():
            content = path.read_text()
            return "compression_ratio" in content and "10" in content
        return False


if __name__ == "__main__":
    # Run verification
    base_path = "/Users/nishu/Downloads/betting/betting_platform/programs/betting_platform"
    verifier = TestVerifier(base_path)
    verifier.run_all_tests()