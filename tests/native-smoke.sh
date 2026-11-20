#!/bin/sh
# Run inside runtime container with service listening. No external provider calls.
set -eu
w=$(mktemp -d)
trap 'rm -rf "$w"' EXIT
base=${1:-http://127.0.0.1:8080}
cat > "$w/in.ps" <<'PS'
%!PS
/Helvetica findfont 20 scalefont setfont
72 650 moveto (PAGE ALPHA) show showpage
/Helvetica findfont 20 scalefont setfont
72 650 moveto (PAGE BETA) show showpage
PS
gs -q -dBATCH -dNOPAUSE -sDEVICE=pdfwrite -sOutputFile="$w/in.pdf" "$w/in.ps"
pdftoppm -r 40 -png -singlefile "$w/in.pdf" "$w/image"
python3 - "$w/form.pdf" <<'PY'
import sys
objects=[b'<< /Type /Catalog /Pages 2 0 R /AcroForm 5 0 R >>',b'<< /Type /Pages /Kids [3 0 R] /Count 1 >>',b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Annots [4 0 R] >>',b'<< /Type /Annot /Subtype /Widget /FT /Tx /T (name) /V (Old) /Rect [72 600 250 630] /P 3 0 R /DA (/Helv 12 Tf 0 g) >>',b'<< /Fields [4 0 R] /DR << /Font << /Helv 6 0 R >> >> /DA (/Helv 12 Tf 0 g) >>',b'<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>']
data=b'%PDF-1.4\n'; offsets=[0]
for i,obj in enumerate(objects,1):
 offsets.append(len(data)); data+=f'{i} 0 obj\n'.encode()+obj+b'\nendobj\n'
x=len(data); data+=f'xref\n0 {len(offsets)}\n0000000000 65535 f \n'.encode()
for off in offsets[1:]: data+=f'{off:010} 00000 n \n'.encode()
data+=f'trailer\n<< /Size {len(offsets)} /Root 1 0 R >>\nstartxref\n{x}\n%%EOF\n'.encode()
open(sys.argv[1],'wb').write(data)
PY
post() {
 tool=$1; out=$2; shift 2
 code=$(curl -sS --max-time 240 -D "$w/headers" -o "$out" -w '%{http_code}' "$@" "$base/api/tools/$tool")
 if [ "$code" != 200 ]; then cat "$out"; exit 1; fi
 test -s "$out"
 echo "PASS $tool HTTP"
}
pages() { test "$(qpdf --show-npages "$1")" = "$2"; qpdf --check "$1" >/dev/null; }
post merge "$w/merged.pdf" -F "files=@$w/in.pdf" -F "files=@$w/in.pdf"
pages "$w/merged.pdf" 4
post split "$w/split.pdf" -F "files=@$w/in.pdf" -F pages=2
pages "$w/split.pdf" 1
pdftotext "$w/split.pdf" - | grep -q BETA
post organize "$w/order.pdf" -F "files=@$w/in.pdf" -F pages=2,1,2
pages "$w/order.pdf" 3
pdftotext -f 1 -l 1 "$w/order.pdf" - | grep -q BETA
post rotate "$w/rotate.pdf" -F "files=@$w/in.pdf" -F pages=1 -F angle=90
pdfinfo -f 1 -l 1 "$w/rotate.pdf" | grep -Eq 'rot:[[:space:]]+90'
post crop "$w/crop.pdf" -F "files=@$w/in.pdf" -F margin_left=20 -F margin_right=20 -F margin_top=20 -F margin_bottom=20
pdfinfo -box "$w/crop.pdf" > "$w/boxes"
python3 - "$w/boxes" <<'PY'
import sys
s=open(sys.argv[1]).read().splitlines()
a=next(x for x in s if x.startswith('MediaBox:')).split()[1:]
b=next(x for x in s if x.startswith('CropBox:')).split()[1:]
assert a!=b
PY
post watermark "$w/watermark.pdf" -F "files=@$w/in.pdf" -F text=WATERMARK
pdftotext "$w/watermark.pdf" - | grep -q WATERMARK
post page-numbers "$w/number.pdf" -F "files=@$w/in.pdf" --form-string 'format=TESTNUMBER{n}'
pdftotext "$w/number.pdf" - | grep -q TESTNUMBER2
post sign "$w/sign.pdf" -F "files=@$w/in.pdf" -F text=SIGNATURE
pdftotext "$w/sign.pdf" - | grep -q SIGNATURE
post image-to-pdf "$w/image.pdf" -F "files=@$w/image.png" -F page_size=a4
pages "$w/image.pdf" 1
pdfimages -list "$w/image.pdf" | grep -q image
post forms "$w/discover.json" -F "files=@$w/form.pdf" -F action=discover
grep -q name "$w/discover.json"
post forms "$w/filled.pdf" -F "files=@$w/form.pdf" --form-string 'fields={"name":"NEWVALUE"}'
qpdf --qdf --object-streams=disable "$w/filled.pdf" "$w/filled.qdf"
grep -aq NEWVALUE "$w/filled.qdf"
post compress "$w/compress.pdf" -F "files=@$w/in.pdf" -F target_kb=250
cmp "$w/in.pdf" "$w/compress.pdf"
pages "$w/compress.pdf" 2
code=$(curl -sS -o "$w/reject" -w '%{http_code}' -F "files=@$w/in.pdf" -F pages=999 "$base/api/tools/split")
test "$code" = 422
echo 'PASS all native/compression runtime assertions'
