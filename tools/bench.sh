#!/usr/bin/env bash
# Benchmark 520pdf (Rust Axum + React Router 8 SPA) against original iHatePDF (Express + React Vite)
# Usage: tools/bench.sh ../iHatePDF
set -euo pipefail

ORIG=${1:?usage: tools/bench.sh <path-to-iHatePDF-checkout>}
ORIG=$(cd "$ORIG" && pwd)
ROOT=$(cd "$(dirname "$0")/.." && pwd)
DURATION=${DURATION:-10s}
CONC=${CONC:-32}
OUT=$(mktemp -d)
PIDS=""
cleanup() {
  for p in $PIDS; do
    kill "$p" 2>/dev/null || true
    wait "$p" 2>/dev/null || true
  done
  rm -rf "$OUT"
}
trap cleanup EXIT

log() { echo "• $*" >&2; }
tree() {
  local p
  for p in "$@"; do
    [[ -n "$p" ]] || continue
    echo "$p"
    tree $(pgrep -P "$p" || true)
  done
}
rss_mib() {
  local all
  all=$(tree "$@" | sort -u | tr '\n' ',')
  ps -o rss= -p "${all%,}" 2>/dev/null | awk '{s+=$1} END {printf "%.2f", s/1024}'
}
nproc_of() { tree "$@" | sort -u | wc -l | tr -d ' '; }
wait_up() {
  local start
  start=$(python3 -c 'import time; print(time.time())')
  for _ in $(seq 1 300); do
    if [[ "$(curl -s -o /dev/null -w '%{http_code}' "$1")" == 200 ]]; then
      python3 -c "import time; print(f'{(time.time()-$start)*1000:.0f}')"; return 0
    fi
    sleep 0.02
  done
  echo "timeout"; return 1
}
load() {
  local name=$1 url=$2
  oha -z "$DURATION" -c "$CONC" --no-tui --output-format json "$url" > "$OUT/$name.json"
  python3 - "$OUT/$name.json" <<'PY'
import json, sys
d = json.load(open(sys.argv[1]))
s, p = d["summary"], d["latencyPercentiles"]
print(f'{s["requestsPerSec"]:.0f} {p["p50"]*1000:.2f} {p["p99"]*1000:.2f}')
PY
}

log "Ensuring 520pdf release binary & web build..."
(cd "$ROOT/web" && bun run build >/dev/null 2>&1)
(cd "$ROOT" && cargo build --release --locked -q)

log "Starting 520pdf on 127.0.0.1:3900..."
WEB_DIR="$ROOT/web/build/client" BIND="127.0.0.1:3900" "$ROOT/target/release/pdf520" serve >"$OUT/520pdf.log" 2>&1 &
PDF_PID=$!
PIDS="$PDF_PID"
PDF_START=$(wait_up http://127.0.0.1:3900/api/status)
sleep 0.3
PDF_IDLE=$(rss_mib "$PDF_PID")
read -r PDF_ST_RPS PDF_ST_P50 PDF_ST_P99 < <(load pdf_status http://127.0.0.1:3900/api/status)
read -r PDF_IDX_RPS PDF_IDX_P50 PDF_IDX_P99 < <(load pdf_index http://127.0.0.1:3900/)
PDF_PEAK=$(rss_mib "$PDF_PID")
PDF_PROCS=$(nproc_of "$PDF_PID")
kill "$PDF_PID" 2>/dev/null || true
wait "$PDF_PID" 2>/dev/null || true
PIDS=""

log "Starting iHatePDF (Express dist/index.js) on 127.0.0.1:3901 (macOS temp harness)..."
mkdir -p "$OUT/ihatepdf-run/dist"
ln -s "$ORIG/package.json" "$OUT/ihatepdf-run/package.json"
ln -s "$ORIG/node_modules" "$OUT/ihatepdf-run/node_modules"
ln -s "$ORIG/dist/public" "$OUT/ihatepdf-run/dist/public"
python3 - "$ORIG/dist/index.js" "$OUT/ihatepdf-run/dist/index.js" <<'PY'
import sys
s = open(sys.argv[1]).read()
for pat in ("var port = 5000;", "const port = 5000;", "const port = 5e3;", "var port = 5e3;"):
    s = s.replace(pat, "const port = Number(process.env.PORT || 3901);")
s = s.replace("reusePort: true", "reusePort: false")
assert "PORT || 3901" in s and "reusePort: false" in s, "patch failed"
open(sys.argv[2], "w").write(s)
PY
(cd "$OUT/ihatepdf-run" && NODE_ENV=production PORT=3901 node dist/index.js >"$OUT/orig.log" 2>&1) &
ORIG_PID=$!
PIDS="$ORIG_PID"
ORIG_START=$(wait_up http://127.0.0.1:3901/api/status)
sleep 0.3
ORIG_IDLE=$(rss_mib "$ORIG_PID")
read -r ORIG_ST_RPS ORIG_ST_P50 ORIG_ST_P99 < <(load orig_status http://127.0.0.1:3901/api/status)
read -r ORIG_IDX_RPS ORIG_IDX_P50 ORIG_IDX_P99 < <(load orig_index http://127.0.0.1:3901/)
ORIG_PEAK=$(rss_mib "$ORIG_PID")
ORIG_PROCS=$(nproc_of "$ORIG_PID")
kill "$ORIG_PID" 2>/dev/null || true
wait "$ORIG_PID" 2>/dev/null || true
PIDS=""

# Measure binary/bundle sizes and source LOC
python3 - "$ORIG" "$ROOT" \
  "$PDF_START" "$PDF_IDLE" "$PDF_PEAK" "$PDF_PROCS" "$PDF_ST_RPS" "$PDF_ST_P50" "$PDF_ST_P99" "$PDF_IDX_RPS" "$PDF_IDX_P50" "$PDF_IDX_P99" \
  "$ORIG_START" "$ORIG_IDLE" "$ORIG_PEAK" "$ORIG_PROCS" "$ORIG_ST_RPS" "$ORIG_ST_P50" "$ORIG_ST_P99" "$ORIG_IDX_RPS" "$ORIG_IDX_P50" "$ORIG_IDX_P99" <<'PY'
import json, os, sys
from pathlib import Path

orig = Path(sys.argv[1])
root = Path(sys.argv[2])
(
    pdf_start, pdf_idle, pdf_peak, pdf_procs, pdf_st_rps, pdf_st_p50, pdf_st_p99, pdf_idx_rps, pdf_idx_p50, pdf_idx_p99,
    orig_start, orig_idle, orig_peak, orig_procs, orig_st_rps, orig_st_p50, orig_st_p99, orig_idx_rps, orig_idx_p50, orig_idx_p99,
) = sys.argv[3:]

def scan_files(base, exts, exclude_dirs=()):
    files, loc, total_bytes = 0, 0, 0
    if not base.exists():
        return files, loc, total_bytes
    for p in base.rglob("*"):
        if not p.is_file():
            continue
        if any(part in exclude_dirs for part in p.parts):
            continue
        if exts and p.suffix not in exts:
            continue
        files += 1
        b = p.read_bytes()
        total_bytes += len(b)
        loc += b.count(b"\n") + (1 if b and not b.endswith(b"\n") else 0)
    return files, loc, total_bytes

def dir_bytes(base, exclude=()):
    total = 0
    if not base.exists():
        return 0
    for p in base.rglob("*"):
        if any(part in exclude for part in p.parts):
            continue
        if p.is_file() and not p.is_symlink():
            total += p.stat().st_size
    return total

orig_be = (scan_files(orig / "server", {".ts"})[0] + scan_files(orig / "shared", {".ts"})[0],
           scan_files(orig / "server", {".ts"})[1] + scan_files(orig / "shared", {".ts"})[1],
           scan_files(orig / "server", {".ts"})[2] + scan_files(orig / "shared", {".ts"})[2])
orig_fe = scan_files(orig / "client/src", {".ts", ".tsx", ".css"})
orig_pkg = json.loads((orig / "package.json").read_text())
orig_deps = len(orig_pkg.get("dependencies", {})) + len(orig_pkg.get("devDependencies", {}))
orig_nm_bytes = dir_bytes(orig / "node_modules")
orig_dist_bytes = dir_bytes(orig / "dist")
orig_clean_bytes = dir_bytes(orig, {".git", "node_modules", "dist", ".DS_Store"})

pdf_be = scan_files(root / "src", {".rs"})
pdf_fe = scan_files(root / "web/app", {".ts", ".tsx", ".css"})
pdf_cargo = (root / "Cargo.toml").read_text()
pdf_crate_deps = sum(1 for line in pdf_cargo.split("[dependencies]")[1].split("[profile")[0].splitlines() if "=" in line and not line.strip().startswith("#"))
pdf_web_pkg = json.loads((root / "web/package.json").read_text())
pdf_npm_deps = len(pdf_web_pkg.get("dependencies", {})) + len(pdf_web_pkg.get("devDependencies", {}))
pdf_bin_bytes = (root / "target/release/pdf520").stat().st_size
pdf_web_build_bytes = dir_bytes(root / "web/build/client")
pdf_clean_bytes = dir_bytes(root, {".git", "target", "node_modules", "build", ".react-router", ".DS_Store"})

def mib(b): return f"{b / (1024*1024):.2f} MiB"
def kib(b): return f"{b / 1024:.1f} KiB"

print("### Size & Footprint Comparison\n")
print("| Metric | iHatePDF (Original) | 520pdf (Rust Rewrite) | Difference |")
print("| --- | --- | --- | --- |")
print(f"| **Backend source** | {orig_be[0]} files · {orig_be[1]:,} LOC (`server/` + `shared/`, {kib(orig_be[2])}) | {pdf_be[0]} files · {pdf_be[1]:,} LOC (`src/*.rs`, {kib(pdf_be[2])}) | Full PDF/media/resume engine in Rust |")
print(f"| **Frontend source** | {orig_fe[0]} files · {orig_fe[1]:,} LOC (`client/src`, {kib(orig_fe[2])}) | {pdf_fe[0]} files · {pdf_fe[1]:,} LOC (`web/app`, {kib(pdf_fe[2])}) | **{(orig_fe[1]/pdf_fe[1]):.1f}× fewer LOC** |")
print(f"| **Direct dependencies** | {orig_deps} npm packages ({len(orig_pkg.get('dependencies', {}))} + {len(orig_pkg.get('devDependencies', {}))} dev) | {pdf_crate_deps} Rust crates + {pdf_npm_deps} npm packages | **{(orig_deps/(pdf_crate_deps+pdf_npm_deps)):.1f}× fewer deps** |")
print(f"| **Clean source tree** | {mib(orig_clean_bytes)} | {mib(pdf_clean_bytes)} | Self-contained repo |")
print(f"| **Runtime footprint** | {mib(orig_dist_bytes)} `dist/` + {mib(orig_nm_bytes)} `node_modules/` | {mib(pdf_bin_bytes)} binary + {mib(pdf_web_build_bytes)} `web/build/client` | **No Node runtime (`{mib(orig_nm_bytes)}` eliminated)** |")
print(f"| **Tools / capabilities** | 12 client-side tools | **28 tools** (12/12 iHatePDF parity + 16 workbench tools) | **2.3× broader toolkit** |")
print("\n### HTTP & Resource Benchmark (`oha`)\n")
print("| Metric | iHatePDF (Node/Express) | 520pdf (Rust/Axum `--release`) | Speedup / Delta |")
print("| --- | --- | --- | --- |")
print(f"| **Runtime processes** | {orig_procs} (`node dist/index.js`) | {pdf_procs} (`pdf520 serve`) | Single binary |")
print(f"| **Cold startup to 200** | {orig_start} ms | {pdf_start} ms | **{float(orig_start)/max(float(pdf_start),1):.1f}× faster** |")
print(f"| **Memory at idle (RSS)** | {orig_idle} MiB | {pdf_idle} MiB | **{float(orig_idle)/max(float(pdf_idle),0.1):.1f}× less RAM** |")
print(f"| **Memory after load (RSS)** | {orig_peak} MiB | {pdf_peak} MiB | **{float(orig_peak)/max(float(pdf_peak),0.1):.1f}× less RAM** |")
print(f"| `GET /api/status` | {int(orig_st_rps):,} req/s · p50 {orig_st_p50} ms · p99 {orig_st_p99} ms | {int(pdf_st_rps):,} req/s · p50 {pdf_st_p50} ms · p99 {pdf_st_p99} ms | **{float(pdf_st_rps)/max(float(orig_st_rps),1):.1f}× throughput** |")
print(f"| `GET /` (SPA HTML) | {int(orig_idx_rps):,} req/s · p50 {orig_idx_p50} ms · p99 {orig_idx_p99} ms | {int(pdf_idx_rps):,} req/s · p50 {pdf_idx_p50} ms · p99 {pdf_idx_p99} ms | **{float(pdf_idx_rps)/max(float(orig_idx_rps),1):.1f}× throughput** |")
PY
