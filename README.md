# Betting Platform (Prototype)

This repository contains multiple overlapping implementations of a prediction/betting platform (Solana + optional Polygon/Polymarket integrations), plus extensive demos and test artifacts.

Start here:
- `REPO_WALKTHROUGH.md` – end-to-end map of what’s in the repo
- `PRODUCTION_READINESS.md` – what’s missing for production
- `GITHUB_PUBLISHING.md` – how to publish/deploy via GitHub

## Local development (recommended stack)

The most “real app” stack lives under `betting_platform/`:
- UI (Next.js): `betting_platform/app`
- Backend (Rust/Axum): `betting_platform/api_runner` (default `127.0.0.1:8081`)

### Backend
```bash
cd betting_platform/api_runner
cargo run --release
```

### UI
```bash
cd betting_platform/app
npm install
npm run dev
```

## Notes
- This repo is **not production-ready** yet (see `PRODUCTION_READINESS.md`).
- Do **not** commit real keys or `.env` files. Use `betting_platform/.env.example`.

