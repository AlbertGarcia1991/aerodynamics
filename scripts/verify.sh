#!/usr/bin/env bash
# Full verification: everything CI would run, in dependency order.
#   scripts/verify.sh            # all stages
#   SKIP_E2E=1 scripts/verify.sh # skip the browser suites
set -euo pipefail
cd "$(dirname "$0")/.."
step() { printf '\n\033[1;34m▶ %s\033[0m\n' "$*"; }

step "Rust: formatting";            cargo fmt --all -- --check
step "Rust: clippy (deny warnings)"; cargo clippy --workspace --all-targets --quiet -- -D warnings
step "Rust: tests (numerical, contract, unit)"; cargo test --workspace --quiet
step "WASM: build";                 wasm-pack build crates/wasm-api --target web --release \
                                      --out-dir ../../apps/web/src/wasm/pkg --out-name aeroflow --quiet
cd apps/web
[ -d node_modules ] || { step "npm install"; npm ci --no-audit --no-fund; }
step "TypeScript: typecheck";       npx tsc -b --noEmit
step "Vitest: unit, contract, boundaries"; npx vitest run
if [ "${SKIP_E2E:-0}" != "1" ]; then
  step "Playwright: functional, visual, accessibility, performance (dev server)"; npx playwright test
  step "Playwright: production-build smoke";  npx playwright test -c playwright.preview.config.ts
fi
printf '\n\033[1;32m✔ all checks passed\033[0m\n'
