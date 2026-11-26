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
assert len(tools) == 36, f"expected 36 tools, got {len(tools)}"
PY
echo "PASS /api/tools (36 tools)"

for route in / /drop /login /merge-pdfs /split-pdf /compress-pdf /compress-image /pdf-to-image /image-to-pdf /watermark-pdf /lock-pdf /sign-pdf /edit-pdf /equation-to-image /latex-resume /resume-builder; do
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

# 3. Merge, Split, Compress, Watermark, Visual Protect, Sign + filename & 250 KB floor check
cp "$TMP/resume.pdf" "$TMP/NPWP.pdf"
post_tool merge "$TMP/merged.pdf" -F "files=@$TMP/NPWP.pdf" -F "files=@$TMP/latex.pdf"
grep -qi 'filename="NPWP_520pdf_merge.pdf"' "$TMP/headers"
post_tool split "$TMP/split.pdf" -F "files=@$TMP/merged.pdf" -F "pages=1" -F "mode=extract"
post_tool split "$TMP/split.zip" -F "files=@$TMP/merged.pdf" -F "mode=all"
post_tool compress "$TMP/compressed.pdf" -F "files=@$TMP/NPWP.pdf" -F "quality_preset=high" -F "remove_metadata=true"
grep -qi '^x-compress-status:' "$TMP/headers"
grep -qi 'filename="NPWP_520pdf_compress.pdf"' "$TMP/headers"
TOO_LOW_CODE=$(curl -sS -o /dev/null -w '%{http_code}' -F "files=@$TMP/NPWP.pdf" -F "target_kb=100" "$BASE/api/tools/compress")
test "$TOO_LOW_CODE" = "422"
post_tool watermark "$TMP/watermark.pdf" -F "files=@$TMP/NPWP.pdf" -F "text=CONFIDENTIAL" -F "color=#E63228" -F "cols=3" -F "rows=3" -F "rotation=35" -F "opacity=0.3"
grep -qi 'filename="NPWP_520pdf_watermark.pdf"' "$TMP/headers"
head -c 5 "$TMP/watermark.pdf" | grep -q '%PDF-'
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
post_tool pdf-to-image "$TMP/extracted.zip" -F "files=@$TMP/from-img.pdf" -F "mode=extract" -F "format=png"
python3 - "$TMP/extracted.zip" <<'PY'
import sys, zipfile
with zipfile.ZipFile(sys.argv[1]) as z:
    names = z.namelist()
    assert len(names) >= 1 and names[0].endswith(".png"), names
PY

post_tool rotate "$TMP/rotated-batch.zip" -F "files=@$TMP/NPWP.pdf" -F "files=@$TMP/latex.pdf" -F "angle=90"
python3 - "$TMP/rotated-batch.zip" <<'PY'
import sys, zipfile
with zipfile.ZipFile(sys.argv[1]) as z:
    assert len(z.namelist()) == 2, z.namelist()
PY

post_tool edit-pdf "$TMP/edited.pdf" -F "files=@$TMP/NPWP.pdf" -F 'annotations=[{"type":"text","page":1,"x":60,"y":720,"text":"Reviewed","font_size":18,"color":"#E63228"},{"type":"highlight","page":1,"x":60,"y":680,"width":160,"height":20},{"type":"rect","page":1,"x":60,"y":600,"width":120,"height":50,"color":"#2563EB"},{"type":"path","page":1,"points":[[60,550],[120,570],[180,545]]}]'
grep -qi 'filename="NPWP_520pdf_edit-pdf.pdf"' "$TMP/headers"
head -c 5 "$TMP/edited.pdf" | grep -q '%PDF-'

post_tool pdf-to-word "$TMP/doc.docx" -F "files=@$TMP/NPWP.pdf"
grep -qi 'filename="NPWP_520pdf_pdf-to-word.docx"' "$TMP/headers"
python3 - "$TMP/doc.docx" <<'PY'
import sys, zipfile
with zipfile.ZipFile(sys.argv[1]) as z:
    assert "word/document.xml" in z.namelist()
PY
post_tool word-to-pdf "$TMP/from-docx.pdf" -F "files=@$TMP/doc.docx"
head -c 5 "$TMP/from-docx.pdf" | grep -q '%PDF-'

post_tool pdf-to-powerpoint "$TMP/deck-vis.pptx" -F "files=@$TMP/NPWP.pdf" -F "mode=visual"
post_tool pdf-to-powerpoint "$TMP/deck-edit.pptx" -F "files=@$TMP/NPWP.pdf" -F "mode=editable"
python3 - "$TMP/deck-vis.pptx" "$TMP/deck-edit.pptx" <<'PY'
import sys, zipfile
for p in sys.argv[1:]:
    with zipfile.ZipFile(p) as z:
        assert "ppt/slides/slide1.xml" in z.namelist(), p
PY
post_tool powerpoint-to-pdf "$TMP/from-pptx.pdf" -F "files=@$TMP/deck-edit.pptx"
head -c 5 "$TMP/from-pptx.pdf" | grep -q '%PDF-'

post_tool pdf-to-excel "$TMP/sheet.xlsx" -F "files=@$TMP/NPWP.pdf"
python3 - "$TMP/sheet.xlsx" <<'PY'
import sys, zipfile
with zipfile.ZipFile(sys.argv[1]) as z:
    assert "xl/worksheets/sheet1.xml" in z.namelist()
PY
post_tool excel-to-pdf "$TMP/from-xlsx.pdf" -F "files=@$TMP/sheet.xlsx"
head -c 5 "$TMP/from-xlsx.pdf" | grep -q '%PDF-'

post_tool pdf-to-markdown "$TMP/out.md" -F "files=@$TMP/NPWP.pdf"
test -s "$TMP/out.md"

post_tool html-to-pdf "$TMP/from-html.pdf" --form-string "html=<h1>520pdf</h1><script>evil()</script><p>Offline HTML test.</p>"
head -c 5 "$TMP/from-html.pdf" | grep -q '%PDF-'

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
