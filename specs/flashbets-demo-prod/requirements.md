# Requirements: Make flashbets demo fully working end-to-end, with green CI and deployable on GitHub.

## Goal

Ship a demo-ready FlashBets stack that runs end-to-end (UI → API → mock external services) with a green GitHub Actions pipeline and reproducible local instructions.

## Users / personas

- **Demo user / stakeholder**: wants to click through markets and place a “trade” without setup pain.
- **Developer / maintainer**: wants deterministic CI gates and a quick local workflow.

## User stories

### US-1: Browse markets

**As a** demo user  
**I want** to view a list of markets in the UI  
**So that** I can pick an outcome to trade.

**Acceptance criteria**
- AC-1.1: `betting_platform/app` builds (`npm run build`) and renders the markets view without SSR errors.
- AC-1.2: Markets load in demo mode backed by mock services (no real API keys required).

### US-2: Place a demo trade end-to-end

**As a** demo user  
**I want** to place a trade on an outcome  
**So that** the UI shows a successful submission and the API logs a coherent request/response.

**Acceptance criteria**
- AC-2.1: Playwright smoke test passes: `polymarket-trade-smoke.spec.ts`.
- AC-2.2: API `/health` returns HTTP 200 during E2E runs.

## Functional requirements (FR)

| ID | Requirement | Priority | Verification |
|----|-------------|----------|--------------|
| FR-1 | Green CI: Rust tests, UI lint/typecheck/build, and E2E smoke | High | GitHub Actions workflow is green |
| FR-2 | API config loads from TOML defaults without parsing errors | High | `cargo test --locked` in `api_runner` |
| FR-3 | Demo external integration via mock Polymarket server | High | E2E + API integration unit tests |
| FR-4 | Flash-bets program compiles and zk helpers are covered by tests | Medium | `cargo test --locked` in `flash_bets/program` |

## Non-functional requirements (NFR)

| ID | Category | Target | Notes |
|----|----------|--------|-------|
| NFR-1 | Reliability | Deterministic CI | No tests should depend on live localhost services |
| NFR-2 | Security | No secrets committed | Defaults use placeholders; production uses env vars |
| NFR-3 | Developer UX | Clear demo steps | Documented “run demo” steps and expected ports |

## Out of scope / non-goals

- Real mainnet/on-chain deployments.
- Real Polymarket API keys / live trading.
- Hardening for adversarial production traffic (demo-only).

## Assumptions

- Demo environment uses mock external services and in-memory defaults; Postgres/Redis are not required to run the smoke demo.

## Dependencies

- Node.js + npm (CI uses Node LTS).
- Rust stable toolchain.
- Playwright (Chromium) for E2E.

## Success metrics

- CI green on the `flashbets` GitHub repo.
- A fresh clone can run the demo and the E2E smoke test with the documented commands.
