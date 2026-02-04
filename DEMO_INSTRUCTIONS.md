# Quantum Betting Platform - Demo Instructions

## Recommended Demo Stack (Full-Stack)

- UI (Next.js): **http://localhost:3000**
- API (Rust/Axum): **http://localhost:8081**

## Quick Start Guide

### 1. Start the API
```bash
cd betting_platform/api_runner
cargo run --release
```

### 2. Start the UI
```bash
cd betting_platform/app
npm ci
npm run dev -- -p 3000
```

### 3. (Optional) Enable Polymarket order endpoints for demo
The `/api/orders/*` endpoints require Polymarket CLOB credentials. For a pure demo, you can use dummy credentials and point at a mock server:
- Set `POLYMARKET_API_KEY`, `POLYMARKET_API_SECRET` (base64), `POLYMARKET_API_PASSPHRASE`
- Optionally set `POLYMARKET_CLOB_BASE_URL` to a local mock

## Alternate Demo (Static Root UI)

The repo root also contains a static UI demo served by `server.js`:
```bash
node server.js
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
