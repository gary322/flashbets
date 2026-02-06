# Tasks: Make flashbets demo fully working end-to-end, with green CI and deployable on GitHub.

## Overview

This spec is primarily a **stabilization + CI green** effort.

## Phase 1: Make demo work end-to-end (POC)

- [x] 1.1 Fix API duration config parsing
  - **Do**: switch `Duration` fields to `humantime-serde` and update TOML defaults to strings
  - **Files**: `betting_platform/api_runner/src/environment_config.rs`, `betting_platform/api_runner/config/config.default.toml`
  - **Done when**: API config loads without parsing errors
  - **Verify**: `cargo test --locked --manifest-path betting_platform/api_runner/Cargo.toml`
  - _Reqs: FR-2_

- [x] 1.2 Make request middleware tolerant of missing `ConnectInfo`
  - **Do**: accept `Option<ConnectInfo<SocketAddr>>` and use safe fallbacks
  - **Files**: `betting_platform/api_runner/src/*middleware*.rs`, `betting_platform/api_runner/src/security/*.rs`
  - **Done when**: `/health` doesn’t 500 in tests and E2E
  - **Verify**: `cargo test --locked --manifest-path betting_platform/api_runner/Cargo.toml`
  - _Reqs: FR-2, AC-2.2_

- [x] 1.3 Gate live-only integration tests
  - **Do**: only run “hits localhost/live server” tests when env var is set
  - **Files**: `betting_platform/api_runner/tests/integration_tests.rs`
  - **Done when**: CI doesn’t depend on local services
  - **Verify**: `cargo test --locked --manifest-path betting_platform/api_runner/Cargo.toml`
  - _Reqs: NFR-1_

- [x] 1.4 Fix Next.js Solana wallet SSR issues
  - **Do**: move wallet providers into a client-only component loaded via `next/dynamic({ ssr:false })`
  - **Files**: `betting_platform/app/src/pages/_app.tsx`, `betting_platform/app/src/components/SolanaWalletProviders.tsx`
  - **Done when**: `next build` succeeds without SSR/provider errors
  - **Verify**: `npm run build` (in a clean install)
  - _Reqs: AC-1.1_

- [x] 1.5 Implement flash-bets zk helpers + stabilize program tests
  - **Do**: implement Groth16 proof helpers using `ark-groth16`; align demo math + tests
  - **Files**: `betting_platform/flash_bets/program/src/zk/*`, `betting_platform/flash_bets/program/src/utils/*`
  - **Done when**: program compiles and tests pass
  - **Verify**: `cargo test --locked` (in `betting_platform/flash_bets/program`)
  - _Reqs: FR-4_

## Phase 2: Quality gates

- [x] 2.1 Rust test matrix
  - **Verify**:
    - `cargo test --locked` (repo root)
    - `cargo test --locked --manifest-path betting_platform/api_runner/Cargo.toml`
    - `cargo test --locked` (in `betting_platform/flash_bets/program`)

- [x] 2.2 UI quality gates (clean checkout)
  - **Verify** (in `betting_platform/app`):
    - `npm ci`
    - `npm run type-check`
    - `npm run lint`
    - `npm run build`
  - **Done when**: all commands succeed

- [x] 2.3 E2E smoke (clean checkout)
  - **Verify**: run mock server + API + UI and execute Playwright smoke spec
  - **Done when**: `polymarket-trade-smoke.spec.ts` passes
  - _Reqs: AC-2.1_

## Phase 3: Release

- [x] 3.1 Repo hygiene + docs
  - **Do**: remove junk/untracked artifacts; ensure no secrets are committed
  - **Verify**: `git status` is clean except intended changes

- [x] 3.2 Push to GitHub repo
  - **Do**: set `origin` to `git@github.com:gary322/flashbets.git`, commit, push
  - **Done when**: GitHub Actions runs on push

- [x] 3.3 Confirm CI green
  - **Do**: watch workflow run and fix any failures
