# Repo Walkthrough (End-to-End)

This repo contains **multiple overlapping implementations** of a “betting / prediction markets” platform:

- A **static, mock-only UI demo** under `experiments/static-root-demo/` (`server.js` → `platform_ui.html` + `platform_main.js`).
- A larger **full-stack prototype** under `betting_platform/` (Next.js UI + Rust/Axum API + Solana programs + Polygon contracts + keepers + heavy test artifacts).
- A separate set of smaller Solana programs in the root Rust workspace (`programs/*`) that look like **modular experiments** (verse classification, correlation engine, leverage safety, state compression).

Because the repo includes **~290k files** (dominantly `node_modules/`, build outputs, generated artifacts, test ledgers/snapshots, logs, and binaries), it is not realistic to claim “I read and understood every line of every third‑party/generated file”. What *is* realistic and verifiable is:

- I enumerated the entire tree (`find . -type f | wc -l` → **290,552 files**).
- I identified all **first‑party entrypoints** and traced the main runtime flows.
- I cataloged which directories are **first‑party vs generated/vendor/artifacts**, and highlighted the biggest wiring gaps / stubs.

---

## 1) Top-Level Inventory (repo root)

### Static demo (experiments)
- `experiments/static-root-demo/server.js` (Node http server, port **8080**) serves the static UI demo.
- `experiments/static-root-demo/platform_ui.html`, `platform_main.js`, `platform_styles.css`, `market_data.js`, plus many `*.html` “phase/demo” pages.
- `experiments/static-root-demo/solana_integration.js`, `verse_system.js`, `trading_interface.js`, `quantum_mode.js`: UI/logic scripts used by the static demo pages.

### Root Rust workspace (independent of `betting_platform/`)
- `Cargo.toml` workspace members:
  - `programs/verse-classification/`
  - `programs/correlation-engine/`
  - `programs/leverage-safety/`
  - `programs/state-compression/`

These are separate Solana programs with **placeholder program IDs** (e.g., `111111...`, `222222...`) and some TODOs (example: `programs/correlation-engine/src/state/correlation_matrix.rs` has a `// TODO: Calculate standard deviation`).

### Root “extra” programs (not in workspace)
- `programs/betting_platform/`, `programs/betting_platform_native/`, `programs/phase10_betting/`: additional Solana programs and/or build artifacts. They are not part of the root workspace build by default.

### Mobile
- `experiments/mobile/mobile/`: React Native app skeleton; contains imports to files that aren’t present (does not look buildable as-is).
- `experiments/mobile/mobile-app/`: small component library (cards/gestures/curve editor/theme/types).

### Docs / artifacts
- Many `PHASE_*`, `PART7_*`, and “implementation report” markdown files. Several contradict the code reality (e.g., claim “no mocks/TODOs” while the code contains TODOs and explicit `NotImplemented` branches).
- `target/`, `.next/`, various compiled binaries at root (artifacts).

---

## 2) The “Full Stack” Prototype (`betting_platform/`)

`betting_platform/` is the closest thing to a “complete product” in this repo, but it’s still a prototype with significant incomplete wiring.

### 2.1 UI: `betting_platform/app/` (Next.js)
Key facts:
- Next.js 14 app (`betting_platform/app/package.json`).
- Pages that talk directly to the backend at **`http://localhost:8081`** (examples: `src/pages/markets.tsx`, `markets-quantum.tsx`, `demo.tsx`).
- “Platform” page redirects to the static demo html in public: `src/pages/platform.tsx` → `/platform_ui.html`.
- Next API routes:
  - Catch-all proxy: `src/pages/api/[...path].ts` forwards `/api/*` to the Rust backend (configurable via `API_PROXY_TARGET` / `API_BASE_URL` / `NEXT_PUBLIC_API_URL`).

Wallet integration (MetaMask):
- Listener/reconnect handling is fixed in `src/lib/metamask.ts` (provider init on reconnect; correct handler removal).

Static demo assets inside Next:
- `betting_platform/app/public/platform_ui.html` + `platform_main.js` **do call** `http://localhost:8081/api` (unlike `experiments/static-root-demo/platform_main.js`, which is mock-only).

### 2.2 Backend: `betting_platform/api_runner/` (Rust + Axum)
Key entrypoint:
- `betting_platform/api_runner/src/main.rs` binds using `SERVER_HOST` / `SERVER_PORT` (default `127.0.0.1:8081`).

Route surface:
- `main.rs` defines a very large route set under `/api/*` (markets, verses, demo wallet creation, trading, quantum endpoints, websocket endpoints, cache/queue/db endpoints, etc.).

Polymarket order submission/status/cancel is implemented:
- `/api/orders/*` routes call `betting_platform/api_runner/src/handlers/polymarket_orders.rs`, which verifies EIP-712 signatures and then submits via the authenticated CLOB client (`integration::polymarket_clob`).
- Requires `POLYMARKET_API_KEY`, `POLYMARKET_API_SECRET` (base64), `POLYMARKET_API_PASSPHRASE` (optionally `POLYMARKET_CLOB_BASE_URL` for demo mocks).

Database schema mismatch risk:
- There are multiple DB schema sources: SQL migrations in `betting_platform/migrations/*.sql` and Rust migration/modules in `betting_platform/api_runner/src/db/*`.
- The SQL schema defines `markets(market_id UUID, question TEXT, ...)` and Polymarket tables like `polymarket_orders(order_id, condition_id, token_id, ...)`.
- The Rust DB layer appears to include both “old” and “production” schema variants (models/queries don’t consistently match the SQL).

Newer Polymarket stack exists but isn’t clearly wired:
- There are richer Polymarket-related modules (router/service/repository/integration) present under `api_runner/src/handlers`, `api_runner/src/services`, `api_runner/src/integration`, and `api_runner/src/db`, but the active HTTP route for `/api/orders/submit` currently targets the older stub handler (`handlers/polymarket_orders.rs`).

### 2.3 On-chain: Solana programs (`betting_platform/programs/` + `betting_platform/flash_bets/`)
Main Solana program(s):
- `betting_platform/programs/betting_platform_native/` (native Solana program, no Anchor) declares program ID `Hr6kfa5dvGU8sHQ9qNpFXkkJQmUSzjSZxdZ9BGRPPSa4`.
- `betting_platform/programs/betting_platform/` (Anchor wrapper) also declares the same ID.

Status:
- The codebase contains explicit `NotImplemented` returns for certain instructions and advanced order types:
  - Example: `betting_platform/programs/betting_platform_native/src/processor.rs` has a branch returning `BettingPlatformError::NotImplemented`.
  - Advanced order modules (TWAP/iceberg/etc.) also return `NotImplemented`.

Flash bets:
- `betting_platform/flash_bets/program/` is a Solana program implementing “flash verse / micro-tau / leverage chaining / quantum” concepts.
- The ZK verifier under `betting_platform/flash_bets/program/src/zk/` performs Groth16 proof verification (demo-grade circuits/keys; see `zk/groth16_verifier.rs`).

### 2.4 EVM/Polygon contracts: `betting_platform/contracts/`
Source Solidity is primarily under `betting_platform/contracts/polygon/**`:
- Core: `BettingPlatform.sol`, `MarketFactory.sol`, `PolymarketIntegration.sol`
- DeFi: `LeverageVault.sol`, `LiquidityPool.sol`
- Flash: `FlashBetting.sol`
- Mocks: `MockUSDC.sol`, `MockAavePool.sol`

This directory also contains heavy generated output (`artifacts/`, `typechain/`), which dominates file count/time for tooling.

### 2.5 Off-chain automation: `betting_platform/src/`
Notable modules:
- `polymarket_client.ts`: Polymarket REST + WS client with retry/rate limit.
- `mock_polymarket.ts`: mock API + websocket server for testing.
- `keeper.ts`, `keeper_coordinator.ts`, `failover_manager.ts`: keeper coordination and reliability scaffolding; includes TODO/unimplemented sections.
- `verification_framework.ts`: validation framework but currently uses mock data paths.

### 2.6 Tests / artifacts (very large)
Directories:
- `betting_platform/tests/` (~1GB): includes screenshots, load testing results, embedded `node_modules/`, and Solana test ledgers/snapshots.
- `betting_platform/test_e2e/`: bash scripts + results/logs + Redis dump.

Important caveat:
- Some scripts refer to features/configs that don’t exist (example: `test_e2e/start_api_test_mode.sh` runs `cargo ... --features "test-mode"`, but `api_runner/Cargo.toml` does not define a `test-mode` feature).
- Multiple scripts/docker configs assume the API is on **8080**, but the backend defaults to **8081** unless overridden via `SERVER_PORT`.

---

## 3) “Source of Truth” Reality Check (what’s actually wired today)

If you run only what is wired consistently in code:

1) Backend: `betting_platform/api_runner` → listens on `SERVER_HOST:SERVER_PORT` (default `127.0.0.1:8081`).
2) UI:
   - Next pages that call `http://localhost:8081/...` directly should work for read-only / demo flows.
   - `/api/*` calls go through a Next catch-all proxy route, so UI can use relative `/api/...` paths.
3) The `experiments/static-root-demo/server.js` demo UI is largely mock-only and does not fetch backend data.

---

## 4) Validation Attempts (in this restricted environment)

- `cargo test` at repo root failed because network access to `crates.io` is blocked (dependency fetch fails).
- `npm run lint` in `betting_platform/app` failed under Node **v24.5.0** with `ERR_INVALID_PACKAGE_CONFIG` inside Next’s compiled dependencies (suggest using a supported Node version, typically 18/20/22 for Next 14).
- `npm run type-check` in `betting_platform/app` did not finish within 10 minutes (likely due to FS size/latency or TS project issues).

---

## 5) Recommended Next Step (to make this repo coherent)

Pick exactly one “stack” as the source of truth:

- **Option A (recommended):** treat `betting_platform/` as the product, and delete/ignore the root static demo and root experimental programs, or move them to `experiments/`.
- **Option B:** treat the root as the product, and remove `betting_platform/` as a separate project.

Then:
- Fix API port/config consistency (8080 vs 8081).
- Unify DB schema (choose a single source of truth, update Rust models/queries accordingly).
- Decide whether flash-bets and Polymarket use real integrations or demo mocks by default.
- Make Node version explicit (e.g., `.nvmrc`) and make Next lint/typecheck reproducible.
