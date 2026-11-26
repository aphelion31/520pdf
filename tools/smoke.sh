#!/usr/bin/env bash
set -euo pipefail

BASE=${1:-${BASE:-http://127.0.0.1:8080}}
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

check_get() {
  local path=$1
  local code
  code=$(curl -sS -o "$TMP/body" -w '%{http_code}' "$BASE$path")
  if [ "$code" != "200" ]; then
    echo "FAIL GET $path -> $code"
    cat "$TMP/body"
    exit 1
  fi
}

post_tool() {
  local tool=$1
  local out=$2
  shift 2
  local code
  code=$(curl -sS --max-time 180 -D "$TMP/headers" -o "$out" -w '%{http_code}' "$@" "$BASE/api/tools/$tool")
  if [ "$code" != "200" ]; then
    echo "FAIL POST /api/tools/$tool -> $code"
    cat "$out"
    exit 1
  fi
  test -s "$out"
  echo "PASS POST /api/tools/$tool"
}

check_get /health
grep -q '^ok$' "$TMP/body"
check_get /ready
grep -q '^ok$' "$TMP/body"
check_get /api/status
python3 - "$TMP/body" <<'PY'
import json, sys
data = json.load(open(sys.argv[1]))
assert data["status"] == "ok", data
assert data["timestamp"].endswith("Z"), data
PY
echo "PASS /health, /ready, /api/status"

check_get /api/tools
python3 - "$TMP/body" <<'PY'
import json, sys
tools = json.load(open(sys.argv[1]))["tools"]
assert len(tools) == 28, f"expected 28 tools, got {len(tools)}"
PY
echo "PASS /api/tools (28 tools)"

for route in / /drop /login /merge-pdfs /split-pdf /compress-pdf /compress-image /pdf-to-image /image-to-pdf /watermark-pdf /lock-pdf /sign-pdf /equation-to-image /latex-resume /resume-builder; do
  check_get "$route"
  grep -qi '<!doctype html>' "$TMP/body"
done
echo "PASS SPA routes"

check_get /api/auth/me
python3 - "$TMP/body" <<'PY'
import json, sys
d = json.load(open(sys.argv[1]))
assert d["algorithm"] == "argon2id", d
assert d["auth_enabled"] is False, d
PY
echo "PASS /api/auth/me (Argon2id)"

# 1. Pure-Rust resume-builder
cat > "$TMP/resume.json" <<'JSON'
{"name":"Nadia Kusuma","email":"nadia@520pdf.dev","summary":"Systems engineer.","experience":"Staff Engineer","skills":"Rust, Axum, React Router"}
JSON
post_tool resume-builder "$TMP/resume.pdf" -F "files=@$TMP/resume.json"
head -c 5 "$TMP/resume.pdf" | grep -q '%PDF-'

# 2. Pure-Rust latex-resume
cat > "$TMP/resume.tex" <<'TEX'
\documentclass{article}
\begin{document}
\begin{center}
{\LARGE \textbf{Alex Rivera}}\\
alex@520pdf.dev $\cdot$ San Francisco
\end{center}
\hrule
\section*{Experience}
\textbf{Lead Engineer} \hfill 2022 -- Present\\
\begin{itemize}
  \item Built pure-Rust Axum PDF platform
\end{itemize}
\end{document}
TEX
post_tool latex-resume "$TMP/latex.pdf" -F "files=@$TMP/resume.tex"
head -c 5 "$TMP/latex.pdf" | grep -q '%PDF-'

# 3. Merge, Split, Compress, Watermark, Visual Protect, Sign
post_tool merge "$TMP/merged.pdf" -F "files=@$TMP/resume.pdf" -F "files=@$TMP/latex.pdf"
post_tool split "$TMP/split.pdf" -F "files=@$TMP/merged.pdf" -F "pages=1" -F "mode=extract"
post_tool split "$TMP/split.zip" -F "files=@$TMP/merged.pdf" -F "mode=all"
post_tool compress "$TMP/compressed.pdf" -F "files=@$TMP/merged.pdf" -F "quality_preset=medium" -F "remove_metadata=true"
grep -qi '^x-compress-status:' "$TMP/headers"
post_tool watermark "$TMP/watermark.pdf" -F "files=@$TMP/resume.pdf" -F "text=CONFIDENTIAL"
post_tool protect "$TMP/protected.pdf" -F "files=@$TMP/resume.pdf" -F "mode=visual" -F "watermark_text=PRIVATE" -F "remove_metadata=true"
post_tool sign "$TMP/signed.pdf" -F "files=@$TMP/resume.pdf" -F "text=Signed by Nadia"

# 4. PDF -> Image (PNG + WebP via Ghostscript fallback) -> Image -> PDF -> Compress Image
post_tool pdf-to-image "$TMP/pages.zip" -F "files=@$TMP/resume.pdf" -F "dpi=120" -F "format=png"
python3 - "$TMP/pages.zip" "$TMP/page1.png" <<'PY'
import sys, zipfile
with zipfile.ZipFile(sys.argv[1]) as z:
    names = z.namelist()
    assert len(names) == 1 and names[0].endswith(".png"), names
    data = z.read(names[0])
    assert data[:4] == b"\x89PNG"
    open(sys.argv[2], "wb").write(data)
PY
post_tool image-to-pdf "$TMP/from-img.pdf" -F "files=@$TMP/page1.png" -F "page_size=a4"
head -c 5 "$TMP/from-img.pdf" | grep -q '%PDF-'

post_tool compress-image "$TMP/img-out.jpg" -F "files=@$TMP/page1.png" -F "quality=75" -F "max_width=640" -F "output_format=jpeg"
python3 - "$TMP/img-out.jpg" <<'PY'
import sys
b = open(sys.argv[1], "rb").read(4)
assert b[:2] == b"\xff\xd8", f"not jpeg: {b!r}"
PY

# 5. Optional Argon2id authentication roundtrip test if binary path exists
BIN=${BIN:-./target/release/pdf520}
if [ -x "$BIN" ]; then
  HASH=$("$BIN" hash-password 'Secret-520!')
  echo "$HASH" | grep -q '^\$argon2id\$v=19\$'
  AUTH_PORT=$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1]); s.close()')
  AUTH_USER=admin AUTH_PASSWORD_HASH="$HASH" WEB_DIR="${WEB_DIR:-web/build/client}" BIND="127.0.0.1:$AUTH_PORT" "$BIN" serve >"$TMP/auth.log" 2>&1 &
  AUTH_PID=$!
  trap 'kill "$AUTH_PID" 2>/dev/null || true; rm -rf "$TMP"' EXIT
  for _ in $(seq 1 100); do
    if curl -sf "http://127.0.0.1:$AUTH_PORT/ready" >/dev/null; then break; fi
    sleep 0.05
  done
  UNAUTH_CODE=$(curl -sS -o /dev/null -w '%{http_code}' -F "resume_json={\"name\":\"Test\"}" "http://127.0.0.1:$AUTH_PORT/api/tools/resume-builder")
  test "$UNAUTH_CODE" = "401"
  BAD_CODE=$(curl -sS -o /dev/null -w '%{http_code}' -H "Content-Type: application/json" -d '{"username":"admin","password":"wrong"}' "http://127.0.0.1:$AUTH_PORT/api/auth/login")
  test "$BAD_CODE" = "401"
  OK_CODE=$(curl -sS -c "$TMP/cookies.txt" -o /dev/null -w '%{http_code}' -H "Content-Type: application/json" -d '{"username":"admin","password":"Secret-520!"}' "http://127.0.0.1:$AUTH_PORT/api/auth/login")
  test "$OK_CODE" = "200"
  AUTH_CODE=$(curl -sS -b "$TMP/cookies.txt" -o "$TMP/auth-resume.pdf" -w '%{http_code}' -F "resume_json={\"name\":\"Test\"}" "http://127.0.0.1:$AUTH_PORT/api/tools/resume-builder")
  test "$AUTH_CODE" = "200"
  head -c 5 "$TMP/auth-resume.pdf" | grep -q '%PDF-'
  kill "$AUTH_PID" 2>/dev/null || true
  wait "$AUTH_PID" 2>/dev/null || true
  echo "PASS Argon2id auth roundtrip (401 -> login -> 200)"
fi

echo "ALL SMOKE CHECKS PASSED"
