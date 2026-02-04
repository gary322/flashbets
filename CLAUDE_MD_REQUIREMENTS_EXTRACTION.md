# CLAUDE.MD Requirements Extraction and Gap Analysis

## Core Requirements from CLAUDE.MD

### 1. SPECIFICATION COMPLIANCE PROCESS
- **Requirement**: Extract EVERY requirement from specification documents
- **Process**: 
  1. Check existing code FIRST
  2. Verify correctness of existing implementation
  3. Implement ONLY missing/incorrect parts
  4. Test new/modified functionality end-to-end
  5. Ensure production-grade quality

### 2. IMPLEMENTATION RULES
- ✅ **Native Solana ONLY** - No Anchor (VERIFIED: No Anchor in Cargo.toml)
- **Production-ready** - No placeholders, mocks, or TODO comments
- **Maintain consistency** with existing code patterns
- **Full testing** of all changes

### 3. BUILD AND ERROR REQUIREMENTS
- **Build after every major edit/addition**
- **0 errors required** to proceed
- **Fix all errors** before moving to next phase
- **Type safety** across all components

### 4. TESTING REQUIREMENTS
- **Exhaustive user journey simulations**
- **Check comprehensive integration**
- **Fix all issues until 0 errors**
- **Verify each part before marking as done**

### 5. DOCUMENTATION REQUIREMENTS
- **Very extensive documentation** of what was done
- **How it was implemented** in detail
- **Mark critical elements**
- **Identify parallel tasks** for 5 agents

## Current State Analysis

### ✅ Confirmed Complete:
1. **92 Smart Contracts** - All documented in SMART_CONTRACTS_INVENTORY.md
2. **Native Solana** - No Anchor dependencies found
3. **Type-safe instruction enum** - BorshSerialize/Deserialize implemented
4. **Program structure** - Proper entrypoint and processor routing

### ❌ Issues Found:
1. **Build Errors** - lib.rs declares modules that don't exist
2. **Missing Module Files**:
   - global_config
   - fee_vault
   - admin
   - upgrade
   - crank
   - cpi_depth_tracker
   - liquidity
   - market_maker
   - spread
   - volume_tracker
   - And many more...

3. **Stack Size Issues** - SPL Token 2022 functions exceeding BPF limits

## Gap Analysis

### Critical Gaps to Address:

1. **Module Structure Mismatch**
   - lib.rs declares ~80+ modules
   - Only ~30-40 actual module files exist
   - Need to reconcile or create missing modules

2. **Build System**
   - Program won't compile due to missing modules
   - Stack size errors from SPL dependencies
   - Need to fix before any testing can occur

3. **Specification Documents**
   - No direct specification PDF found
   - Need to extract requirements from existing documentation
   - Part 7 implementation seems complete per PART7_FINAL_IMPLEMENTATION_SUMMARY.md

4. **Testing Infrastructure**
   - Test files exist but can't run without successful build
   - Need comprehensive user journey tests
   - Integration tests for all 92 contracts

## Action Items

### Phase 1: Fix Build Issues (CRITICAL)
1. Audit all module declarations in lib.rs
2. Either create missing modules or update lib.rs to match actual structure
3. Fix stack size issues (possibly by removing spl-token-2022 features)
4. Achieve successful cargo build-sbf

### Phase 2: Specification Verification
1. Map all 92 contracts to their implementations
2. Verify each contract follows Native Solana patterns
3. Check for any Anchor-style code that needs conversion
4. Document any missing functionality

### Phase 3: Testing Implementation
1. Create comprehensive test suite for all contracts
2. Implement user journey simulations
3. Test type safety across module boundaries
4. Verify 0 errors in all scenarios

### Phase 4: Documentation
1. Document all implementations extensively
2. Create API reference for all 92 contracts
3. Write integration guide
4. Produce final compliance matrix

## Parallel Task Opportunities

Based on the codebase structure, these tasks can be parallelized:

1. **Agent 1**: Fix AMM module implementations (LMSR, PM-AMM, L2-AMM)
2. **Agent 2**: Fix Trading module implementations (orders, liquidation, margin)
3. **Agent 3**: Fix DeFi module implementations (staking, yield, vault)
4. **Agent 4**: Fix Security module implementations (MEV protection, circuit breakers)
5. **Agent 5**: Fix Infrastructure modules (oracle, keeper network, resolution)

## Next Steps

1. **IMMEDIATE**: Fix lib.rs to match actual file structure
2. **URGENT**: Resolve build errors to enable testing
3. **HIGH**: Verify all 92 contracts are properly implemented
4. **MEDIUM**: Create comprehensive test suite
5. **FINAL**: Document everything extensively