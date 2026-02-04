# Production Readiness Assessment

This codebase is **not production-ready** today. It contains multiple overlapping stacks, significant stubs/placeholders in critical paths, inconsistent configuration/ports across components, and large volumes of generated/vendor/test artifact content that must not be shipped as part of a production repo.

This document answers:
1) What “end-to-end” really is in this repo
2) What is missing for production readiness
3) What we need to decide before publishing/deploying

---

## 1) What “the product” is (today)

There are effectively **three stacks**:

1) **Root static demo (mock-first)**  
   - Entry: `server.js` (serves `platform_ui.html` + `platform_main.js` at port 8080)
   - Primarily mock/demo behavior (does not consistently hit the real backend)

2) **Full-stack prototype under `betting_platform/` (closest to real app)**  
   - UI: `betting_platform/app` (Next.js 14)
   - Backend: `betting_platform/api_runner` (Rust/Axum)
   - On-chain: `betting_platform/programs/*` and `betting_platform/flash_bets/*`
   - EVM contracts: `betting_platform/contracts/*`
   - Keepers/off-chain tooling: `betting_platform/src/*`

3) **Root Rust workspace `programs/*` (modular experiments)**  
   - `programs/verse-classification`, `programs/correlation-engine`, `programs/leverage-safety`, `programs/state-compression`

**Production requires choosing one source-of-truth stack**. The only realistic candidate is (2) `betting_platform/`.

---

## 2) Hard blockers to production

### A) Polymarket order pipeline must be configured
The backend order endpoints now submit/status/cancel via an authenticated Polymarket CLOB client:
- Endpoints: `/api/orders/submit`, `/api/orders/:order_id/status`, `/api/orders/:order_id/cancel`, `/api/orders`
- Client: `betting_platform/api_runner/src/integration/polymarket_clob.rs`

**Demo impact:** order endpoints are disabled unless you configure:
- `POLYMARKET_API_KEY`, `POLYMARKET_API_SECRET` (base64), `POLYMARKET_API_PASSPHRASE`
- Optional: `POLYMARKET_CLOB_BASE_URL` to point at a local mock server

### B) Flash-bets now uses Groth16 verification (demo-grade circuit)
The flash-bets program now verifies Groth16 proofs for:
- Flash resolution: `betting_platform/flash_bets/program/src/zk/verifier.rs`
- Quantum collapse: `betting_platform/flash_bets/program/src/zk/groth16_verifier.rs` + `process_collapse_quantum`

**Demo impact:** proofs/keys are generated deterministically in-process for test/demo; this is *not* suitable as-is for real-money/on-chain deployment without a proper key-management + oracle design.

### C) Solana program has explicit `NotImplemented` branches
The native Solana program includes explicit `NotImplemented` returns for some instruction paths and advanced order features.

**Production impact:** API/UI must not expose these features as usable, or they must be implemented fully.

### D) Database schema is not clearly unified

There are multiple competing schema sources:
- SQL migrations: `betting_platform/migrations/001_initial_schema*.sql`, `betting_platform/migrations/002_polymarket_integration.sql`
- Rust DB modules under `betting_platform/api_runner/src/db/*` that include both “old” and “production” schema variants.
- API-runner embedded migrations now also include `polymarket_orders` for tracking: `betting_platform/api_runner/src/db/migrations_production.rs`

**Production impact:** any persistence-backed feature (orders, positions, history, analytics, auth) is at high risk of being inconsistent or silently broken.

### E) Configuration is inconsistent across components
Examples:
- Backend binds to `127.0.0.1:8081` by default; scripts/docker historically assumed different ports.
- Multiple config systems exist (`api_runner/src/config.rs` vs `api_runner/src/environment_config.rs`).

**Production impact:** deployments become fragile; “works on my machine” behavior is likely.

### F) Repo contains sensitive / non-shippable artifacts
The repo includes items that must not be committed or used in production:
- Keypairs: `betting_platform/keypairs/deployer.json` (must be ignored/removed from Git)
- `.env` files (environment-specific config)
- Large test ledgers, snapshots, rocksdb state, logs, embedded node_modules in test folders

**Production impact:** accidental key exposure and an unmanageable repository size.

### G) Frontend/back-end contract is not stable
Even where endpoints exist, there are places where naming/shape mismatches occur (e.g. snake_case vs camelCase). Any production path needs a versioned contract (OpenAPI/JSON schema) and test coverage.

### H) Build & CI are not currently reproducible here
In this environment:
- Rust builds/tests fail because crates can’t be fetched (no network to `crates.io`).
- Next lint fails under Node 24; Next 14 expects a supported Node LTS (typically 18/20/22).

**Demo impact:** GitHub Actions CI is set up to validate builds/tests with pinned Node (20) and Rust toolchain jobs.

---

## 3) Minimum “production-ready” checklist (pragmatic)

### Repo hygiene
- One git repo (no nested `.git/` directories)
- Strong `.gitignore` for artifacts, ledgers, logs, `node_modules`, `target`, `artifacts`, `typechain`, `.env`, keypairs
- Replace real keys with examples (`.env.example` etc.)

### Reproducible builds
- Pin Node version for `betting_platform/app` (e.g. `.nvmrc` or `engines`)
- Pin Rust toolchain (e.g. `rust-toolchain.toml`) for `api_runner` and programs
- Document local run steps (dev vs prod)

### CI (GitHub Actions)
- UI: `npm ci`, `type-check`, `lint`, `build`
- Backend: `cargo test` / `cargo check` + fmt/clippy
- (Optional) contracts: `hardhat test` and solana programs build checks

### Runtime hardening
- Remove/feature-gate all stubs in production paths (Polymarket order submission/status, ZK verification, NotImplemented instructions)
- Consistent config system + environment-variable contract
- DB migrations as the single source of truth + validated schema
- Observability: structured logs, metrics, alertable health checks
- Security: auth/rbac verified, rate limiting verified, secrets stored in a secret manager

---

## 4) Decisions you must make before “production”

1) **Scope:** is “production” a demo environment, or real-money / real trading integrations?
2) **Deployment target:** where should UI + backend run (and what domain layout)?
3) **Chain scope:** do we ship Solana + Polygon + flash-bets + keepers now, or phase them?
4) **Integrations:** should Polymarket/Kalshi be enabled at launch, or behind feature flags?
