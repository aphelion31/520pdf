#!/bin/sh
set -eu
w=$(mktemp -d)
trap 'rm -rf "$w"' EXIT
post() {
 tool=$1; out=$2; shift 2
 code=$(curl -sS --max-time 180 -o "$out" -w '%{http_code}' "$@" "http://127.0.0.1:8080/api/tools/$tool")
 if [ "$code" != 200 ]; then cat "$out"; exit 1; fi
 test -s "$out"
 echo "PASS $tool"
}
printf '%s' '{"name":"TEST RESUME","summary":"Escaped <script>not executable</script>","experience":"Engineer","skills":"Rust"}' > "$w/resume.json"
post resume-builder "$w/resume.pdf" -F "files=@$w/resume.json"
qpdf --check "$w/resume.pdf"
pdftotext "$w/resume.pdf" - | grep -q 'TEST RESUME'
printf '%s' '\frac{a}{b} = x^2' > "$w/math.txt"
post equation-to-image "$w/math.png" -F "files=@$w/math.txt" -F font_size=32
identify "$w/math.png"
post pdf-to-image "$w/pages.zip" -F "files=@$w/resume.pdf" -F dpi=72
python3 - "$w" <<'PY'
import sys,zipfile
from pathlib import Path
p=Path(sys.argv[1])
with zipfile.ZipFile(p/'pages.zip') as z:
 assert len(z.namelist())==1
 (p/'page.png').write_bytes(z.read(z.namelist()[0]))
PY
identify "$w/page.png"
post compress-image "$w/compressed" -F "files=@$w/page.png" -F target_kb=25
identify "$w/compressed"
python3 - "$w" <<'PY'
import sys
from pathlib import Path
p=Path(sys.argv[1]); assert (p/'compressed').stat().st_size <= (p/'page.png').stat().st_size
PY
curl -fsS http://127.0.0.1:8080/api/tools | python3 -c 'import sys,json; t=json.load(sys.stdin)["tools"]; assert len(t)==36; print("PASS 36-tool catalog")'
echo 'PASS new reference tools runtime'
