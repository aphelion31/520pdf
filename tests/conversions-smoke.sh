#!/bin/sh
# Run inside the Ubuntu runtime with the HTTP service already listening.
# Usage: sh tests/conversions-smoke.sh [http://127.0.0.1:8080]
set -eu
base=${1:-http://127.0.0.1:8080}
work=$(mktemp -d "${TMPDIR:-/tmp}/520pdf-smoke.XXXXXX")
trap 'rm -rf "$work"' EXIT HUP INT TERM
for bin in curl python3 gs qpdf pdftotext pdftoppm pdfinfo img2pdf identify libreoffice; do
    command -v "$bin" >/dev/null || { echo "Missing runtime tool: $bin" >&2; exit 1; }
done
curl --fail --silent --show-error "$base/health" >/dev/null
cat > "$work/first.ps" <<'PS'
%!PS-Adobe-3.0
<< /PageSize [612 792] >> setpagedevice
/Helvetica findfont 24 scalefont setfont
72 650 moveto (SAFE SECRET ALPHA) show
showpage
PS
cat > "$work/second.ps" <<'PS'
%!PS-Adobe-3.0
<< /PageSize [612 792] >> setpagedevice
/Helvetica findfont 24 scalefont setfont
72 650 moveto (SAFE SECRET BETA) show
showpage
PS
gs -q -dBATCH -dNOPAUSE -sDEVICE=pdfwrite -sOutputFile="$work/first.pdf" "$work/first.ps"
gs -q -dBATCH -dNOPAUSE -sDEVICE=pdfwrite -sOutputFile="$work/second.pdf" "$work/second.ps"
cat > "$work/input.html" <<'HTML'
<html><head><style>@import url('http://127.0.0.1:8080/health'); .hidden { display: none } .break { break-before: page }</style></head><body><p>SAFE HTML VISIBLE</p><p class="hidden">HIDDEN CSS</p><img src="file:///etc/passwd"><img src="http://169.254.169.254/latest/meta-data/"><iframe src="http://127.0.0.1:8080/health">HIDDEN FRAME</iframe><script>HIDDEN SCRIPT</script><p class="break">SECOND PAGE</p></body></html>
HTML
python3 - "$work" <<'PY'
import sys, zipfile
from pathlib import Path
p = Path(sys.argv[1])
with zipfile.ZipFile(p/'office.docx', 'w') as z:
    z.writestr('[Content_Types].xml', '<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>')
    z.writestr('_rels/.rels', '<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>')
    z.writestr('word/document.xml', '<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>SAFE OFFICE DOCUMENT</w:t></w:r></w:p></w:body></w:document>')
# Damage only the xref pointer. Page content remains recoverable.
data = (p/'first.pdf').read_bytes()
start = data.rfind(b'startxref')
assert start >= 0
(p/'damaged.pdf').write_bytes(data[:start] + b'startxref\n1\n%%EOF\n')
PY
post() {
    tool=$1
    output=$2
    shift 2
    code=$(curl --silent --show-error --max-time 240 -o "$output" -w '%{http_code}' "$@" "$base/api/tools/$tool")
    if [ "$code" != 200 ]; then
        echo "FAIL $tool HTTP $code" >&2
        cat "$output" >&2
        exit 1
    fi
    test -s "$output"
    echo "PASS $tool"
}
check_pdf() { qpdf --check "$1" >/dev/null; }
post office-to-pdf "$work/office.pdf" -F "files=@$work/office.docx"
check_pdf "$work/office.pdf"
pdftotext "$work/office.pdf" - | grep -q 'SAFE OFFICE DOCUMENT'
post html-to-pdf "$work/html.pdf" -F "files=@$work/input.html"
check_pdf "$work/html.pdf"
pdftotext "$work/html.pdf" "$work/html.txt"
grep -q 'SAFE HTML VISIBLE' "$work/html.txt"
if grep -Eq 'HIDDEN|root:|169\.254|127\.0\.0\.1' "$work/html.txt"; then echo 'HTML resource/script leak' >&2; exit 1; fi
pdfinfo "$work/html.pdf" | grep -Eq '^Pages:[[:space:]]+2$'
post compare "$work/compare.txt" -F "files=@$work/first.pdf" -F "files=@$work/second.pdf"
grep -q ALPHA "$work/compare.txt"
grep -q BETA "$work/compare.txt"
grep -q 'Visual comparison' "$work/compare.txt"
grep -Eq 'Page 1: [1-9][0-9]* of [0-9]+ pixels differ' "$work/compare.txt"
post pdf-to-markdown "$work/result.md" -F "files=@$work/first.pdf"
grep -q 'SAFE SECRET ALPHA' "$work/result.md"
post protect "$work/protected.pdf" -F "files=@$work/first.pdf" -F 'password=smoke-secret' -F 'owner_password=smoke-owner'
qpdf --password=smoke-secret --check "$work/protected.pdf" >/dev/null
if qpdf --password=wrong --check "$work/protected.pdf" >/dev/null 2>&1; then echo 'Encryption accepted wrong password' >&2; exit 1; fi
post unlock "$work/unlocked.pdf" -F "files=@$work/protected.pdf" -F 'password=smoke-secret'
check_pdf "$work/unlocked.pdf"
pdftotext "$work/unlocked.pdf" - | grep -q 'SAFE SECRET ALPHA'
post pdfa "$work/archive.pdf" -F "files=@$work/first.pdf"
check_pdf "$work/archive.pdf"
pdfinfo -meta "$work/archive.pdf" | grep -q 'pdfaid:part'
pdftoppm -r 180 -png -singlefile "$work/first.pdf" "$work/scan"
img2pdf "$work/scan.png" --output "$work/scan.pdf"
post scan-to-pdf "$work/ocr.pdf" -F "files=@$work/scan.pdf" -F 'language=eng' -F 'deskew=true'
check_pdf "$work/ocr.pdf"
pdftotext "$work/ocr.pdf" - | grep -q 'SECRET'
post repair "$work/repaired.pdf" -F "files=@$work/damaged.pdf"
check_pdf "$work/repaired.pdf"
pdftotext "$work/repaired.pdf" - | grep -q 'SAFE SECRET ALPHA'
post redact "$work/redacted.pdf" -F "files=@$work/first.pdf" -F 'rectangles=[{"page":1,"x":0,"y":0,"width":612,"height":792}]'
check_pdf "$work/redacted.pdf"
pdftotext "$work/redacted.pdf" "$work/redacted.txt"
if grep -q '[[:alnum:]]' "$work/redacted.txt"; then echo 'Redacted text survived' >&2; exit 1; fi
pdftoppm -r 30 -png -singlefile "$work/redacted.pdf" "$work/redacted-check"
mean=$(identify -format '%[fx:mean]' "$work/redacted-check.png")
python3 - "$mean" <<'PY'
import sys
assert float(sys.argv[1]) < 0.001, 'Fully redacted page contains non-black pixels'
PY
if grep -a -q 'SECRET' "$work/redacted.pdf"; then echo 'Redacted source bytes survived' >&2; exit 1; fi
# Explicitly exercise rejection only. No paid/provider AI request is permitted.
for tool in summarize translate; do
    code=$(curl --silent --show-error --max-time 30 -o "$work/ai-error.json" -w '%{http_code}' -F "files=@$work/first.pdf" -F 'consent=false' "$base/api/tools/$tool")
    test "$code" = 422
    grep -q 'consent' "$work/ai-error.json"
    echo "PASS $tool consent rejection (no provider request)"
done
echo 'PASS all conversion smoke checks'
