---
spec: flashbets-demo-prod
phase: research
created: 2026-02-05T15:11:32+00:00
---

# Research: flashbets-demo-prod

## Goal

Make flashbets demo fully working end-to-end, with green CI and deployable on GitHub.

## Executive summary

- Feasibility: **High** — the repo already contains UI + API + mock integrations + tests; main work is stabilizing config/runtime and CI.
- Key constraints:
  - **iCloud Drive workspace** (`/Users/nish/Library/Mobile Documents/...`) can cause flaky file reads (`ETIMEDOUT`, truncated reads) and slow scans.
  - **Local sandboxing** can block localhost binds; some tests/E2E need running “outside sandbox” for validation.
  - Multi-language stack (Next.js + Rust + Solana-style programs) → need consistent CI gates and smoke E2E.
- Risks:
  - E2E can be flaky locally due to iCloud filesystem; CI on GitHub should be the source of truth.
  - Config parsing mismatches (durations) can cause the API to fail at startup → breaks E2E.
  - Middleware requiring `ConnectInfo` can return 500 in in-memory router tests if connect info isn’t provided.

## Codebase scan

### Relevant existing components

- `betting_platform/app` — Next.js UI for markets/trading flow.
- `betting_platform/api_runner` — Rust/Axum API providing `/health`, trading endpoints, Polymarket integrations.
- `betting_platform/mock/polymarket_mock_server.js` — demo/mock server backing E2E.
- `betting_platform/tests/playwright/next-tests/polymarket-trade-smoke.spec.ts` — end-to-end smoke test.
- `betting_platform/flash_bets/program` — flash-bets “program” crate (demo zk/Groth16 helpers + tests).
- `programs/*` — additional Solana-style program crates; included in root workspace + unit tests.
- `.github/workflows/ci.yml` — CI gates for Rust + UI + E2E.

### Patterns to follow

- Axum middleware patterns in `betting_platform/api_runner/src/tracing_middleware.rs` / `rate_limit.rs`.
- Next.js app client-only providers pattern via `next/dynamic({ ssr: false })`.

### Gaps / missing pieces

- Duration config values were numeric while structs used `Duration` → API config parsing failures at startup.
- Some integration tests assumed a live server on `localhost` → flaky/incorrect for CI.
- Local E2E reliability issues when running from iCloud path.

## External research (optional)

- N/A (kept demo scope local/off-chain; no external API contracts required for CI).

## Open questions

- What is “production” for this repo? **Decision:** demo-only (mock Polymarket; no mainnet/on-chain deploy).
- Should CI be the official end-to-end verifier given local iCloud constraints? **Decision:** yes.

## Sources

- `.github/workflows/ci.yml`
- `betting_platform/api_runner/config/config.default.toml`
- `betting_platform/tests/playwright/next-tests/polymarket-trade-smoke.spec.ts`
