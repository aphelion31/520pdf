#!/usr/bin/env bash
# Single-command build, verify, and cutover rehearsal for 520pdf.
# Usage: rust/cutover.sh [--e2e] [--bench]
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT"

WITH_E2E=0
WITH_BENCH=0
for arg in "$@"; do
  case "$arg" in
    --e2e) WITH_E2E=1 ;;
    --bench) WITH_BENCH=1 ;;
    *) echo "Unknown option: $arg" >&2; exit 2 ;;
  esac
done

echo "==> Checking Rust formatting, lints, and unit tests"
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --locked

echo "==> Typechecking and building React Router 8 SPA (web/)"
(cd web && bun run typecheck && bun run build)

echo "==> Building release binary"
cargo build --release --locked

echo "==> Running API, PDF, and Argon2id smoke suite on 127.0.0.1:8089"
WEB_DIR="$ROOT/web/build/client" BIND="127.0.0.1:8089" "$ROOT/target/release/pdf520" serve >/tmp/pdf520-cutover.log 2>&1 &
SERVER_PID=$!
cleanup() {
  kill "$SERVER_PID" 2>/dev/null || true
  wait "$SERVER_PID" 2>/dev/null || true
}
trap cleanup EXIT

sleep 1
BASE="http://127.0.0.1:8089" "$ROOT/tools/smoke.sh"

if [[ "$WITH_E2E" -eq 1 ]]; then
  echo "==> Running headless Chromium E2E and screenshot verification"
  BASE="http://127.0.0.1:8089" node "$ROOT/tools/e2e.mjs"
fi

if [[ "$WITH_BENCH" -eq 1 && -d "$ROOT/../iHatePDF" ]]; then
  echo "==> Running oha benchmark against ../iHatePDF"
  cleanup
  "$ROOT/tools/bench.sh" "$ROOT/../iHatePDF"
fi

echo "==> PASS: 520pdf cutover verified (binary + SPA ready)"
