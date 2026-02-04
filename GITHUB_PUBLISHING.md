# GitHub Publishing / “Deploy to GitHub” Guide

You asked to “deploy to production on GitHub”. There are two distinct meanings:

1) **Publish the repository to GitHub** (clean history, CI, docs, reproducible builds).  
2) **Deploy a running service** using GitHub Actions (to a cloud provider or to on-chain mainnet).

This guide focuses on (1) and lays out what’s needed for (2).

---

## A) Decide what to publish (required)

This workspace currently has:
- (Historically) a nested git repo inside `betting_platform/`
- A single top-level repo is recommended for “publish everything”

You must choose one:

### Option 1 (recommended): publish **one** repo at the top-level
- Pros: “entire codebase” is in one GitHub repo
- Cons: you must remove/resolve the nested `betting_platform/.git/` first (otherwise it becomes a submodule-like nested repo)

### Option 2: publish only `betting_platform/`
- Pros: simplest if you only want the app subtree
- Cons: does not publish the root demos/docs/programs unless you move/copy them

---

## B) Publishing steps (Option 1: single top-level repo)

1) Remove the nested git directory:
   - delete `betting_platform/.git/` (you approved this to avoid nested-repo/submodule behavior)

2) Initialize git at the top-level:
   - `git init`
   - add a root `.gitignore` (must ignore `node_modules/`, `target/`, artifacts, ledgers, logs, `.env`, keypairs, etc.)

3) Make a first commit:
   - `git add -A`
   - `git commit -m "Initial import"`

4) Create the GitHub repo (web UI) and push:
   - `git remote add origin <your-repo-url>`
   - `git push -u origin main`

---

## C) Publishing steps (Option 2: publish only `betting_platform/`)

1) `cd betting_platform`
2) Ensure `.gitignore` excludes secrets + artifacts (`.env`, `keypairs/`, ledgers/snapshots, node_modules, etc.)
3) `git add -A && git commit -m "Initial import"`
4) Create GitHub repo and push:
   - `git remote add origin <your-repo-url>`
   - `git push -u origin main`

---

## D) “Production deploy” (running service) via GitHub

To deploy an actual running service you need:
- A target platform (Vercel/Render/Fly.io/AWS/GCP/Kubernetes, etc.)
- A deployment method (Docker image publish + deploy, or platform-native builds)
- GitHub Secrets for:
  - DB URL, JWT secret, Polymarket credentials, wallet/key management, etc.
- CI checks that **pass** reliably (build, test, lint)

For this repo, treat “production” as **demo**: use feature flags/mocks by default, and only enable real external integrations when you’ve validated auth, risk controls, and monitoring.
