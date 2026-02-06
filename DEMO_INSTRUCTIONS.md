# Quantum Betting Platform - Demo Instructions

## Recommended Demo Stack (Full-Stack)

- UI (Next.js): **http://localhost:3000**
- API (Rust/Axum): **http://localhost:8081**

## Quick Start Guide

### 1. Start the Polymarket mock (recommended for fully local demo)
This enables `/api/orders/*` to work end-to-end without hitting the real Polymarket service.

```bash
node betting_platform/mock/polymarket_mock_server.js
```

### 2. Start the API (configured for the mock)
```bash
cd betting_platform/api_runner
POLYMARKET_ENABLED=true \
POLYMARKET_CLOB_BASE_URL=http://127.0.0.1:8084 \
POLYMARKET_GAMMA_BASE_URL=http://127.0.0.1:8084 \
POLYMARKET_API_KEY=demo-key \
POLYMARKET_API_SECRET=ZHVtbXktc2VjcmV0 \
POLYMARKET_API_PASSPHRASE=demo-pass \
CACHE_ENABLED=false \
QUEUE_ENABLED=false \
cargo run --release
```

### 3. Start the UI (demo wallet mode)
Demo wallet mode avoids requiring a browser MetaMask extension.

```bash
cd betting_platform/app
npm ci
NEXT_PUBLIC_DEMO_WALLET_ENABLED=true \
API_PROXY_TARGET=http://127.0.0.1:8081 \
npm run dev -- -p 3000
```

### 4. (Optional) Run the end-to-end Playwright test
In another terminal (after the mock + API + UI are running):

```bash
cd betting_platform/tests/playwright
npm ci
npx playwright install chromium
npx playwright test -c playwright.next.config.ts
```

## Alternate Demo (Static Root UI)

The repo also contains a static UI demo under `experiments/static-root-demo/`:
```bash
node experiments/static-root-demo/server.js
```
This demo is mock-heavy and not the recommended “real app” stack.

## Suggested Demo Flows

- Browse markets and verses in the Next UI.
- Connect a Solana wallet (Phantom, etc.) for Solana-oriented UI flows.
- Connect MetaMask to sign a Polymarket EIP-712 order (the UI posts to `/api/orders/submit`).
- If you configured a mock Polymarket CLOB (`POLYMARKET_CLOB_BASE_URL`), submit/cancel/status flows should work end-to-end without hitting the real Polymarket service.

## Notes

- This repo is **demo-grade**; do not use real keys/funds.
- Prefer `betting_platform/` for end-to-end validation; root demos are primarily UI experiments.
