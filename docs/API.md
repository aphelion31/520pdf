# API Webhook

## Scalar (login wajib)

Buka `/docs`, login melalui dialog HTTP Basic browser. Konfigurasi `DOCS_USER`
dan `DOCS_PASSWORD` di `.env` (bukan `WEBHOOK_SECRET`). Tanpa konfigurasi,
akses docs ditolak. `/docs/openapi.json` memakai login yang sama dan `no-store`.
Nginx membatasi akses docs 5 request/menit dengan burst 10 per alamat proxy.
Scalar 1.82.1 dimuat dari jsDelivr, telemetry dan penyimpanan token dinonaktifkan.
Untuk mencoba `/compress`, masukkan Bearer `WEBHOOK_SECRET` di Scalar.

## Endpoint

| Method | Path | Auth | Hasil |
| --- | --- | --- | --- |
| `GET` | `/` | - | `200` teks cara pakai |
| `GET` | `/health` | - | `200 ok` |
| `POST` | `/compress` | `Authorization: Bearer <WEBHOOK_SECRET>` | `200` body `application/pdf` |

### `POST /compress` (multipart/form-data)

| Field | Wajib | Isi |
| --- | --- | --- |
| `file` | ya | File PDF |
| `target_kb` | tidak | Target ukuran KB, default `TARGET_KB` (250) |

Header respons:

| Header | Isi |
| --- | --- |
| `Content-Disposition` | `attachment; filename="compressed_<nama>.pdf"` |
| `X-Compress-Status` | `OK` (di bawah target) atau `OVER_TARGET` (sudah sekecil mungkin, tetap dikirim) |
| `X-Compress-Dpi` | DPI gambar yang dipakai |
| `X-Compress-Quality` | Kualitas JPEG yang dipakai |
| `X-Compress-Size` | Ukuran hasil (byte) |

Error (body teks):

| Status | Sebab |
| --- | --- |
| `401` | Token salah / tidak ada |
| `413` | Upload melebihi `MAX_UPLOAD_MB` |
| `415` | File bukan PDF |
| `422` | Field `file` tidak ada, `target_kb` tidak valid, atau PDF rusak |
| `504` | Melebihi `JOB_TIMEOUT_SECS` |

## Contoh

```bash
curl -fsS -H "Authorization: Bearer $WEBHOOK_SECRET" \
  -F file=@dokumen.pdf -F target_kb=250 \
  -o compressed_dokumen.pdf https://pdf.multimedika.id/compress
```

Laravel:

```php
$res = Http::withToken(config('services.compresspdf.secret'))
    ->timeout(150)
    ->attach('file', file_get_contents($path), 'dokumen.pdf')
    ->post(config('services.compresspdf.url').'/compress', ['target_kb' => 250])
    ->throw();

Storage::put('compressed_dokumen.pdf', $res->body());
$status = $res->header('X-Compress-Status'); // OK | OVER_TARGET
```

## Env

| Env | Default | Isi |
| --- | --- | --- |
| `WEBHOOK_SECRET` | - | Wajib, minimal 16 karakter (`openssl rand -hex 32`) |
| `PORT` | `8080` | Port HTTP |
| `TARGET_KB` | `250` | Target default |
| `MAX_UPLOAD_MB` | `50` | Batas upload |
| `MAX_JOBS` | `2` | Kompresi paralel (CPU berat, sesuaikan jumlah core) |
| `JOB_TIMEOUT_SECS` | `120` | Batas waktu per file |

## CLI (tanpa server)

Binary yang sama bisa dipakai lokal (butuh Ghostscript: `brew install ghostscript`):

```bash
cargo install --path .
compress-pdf dokumen.pdf               # hasil: compressed_dokumen.pdf (<= 250 KB)
compress-pdf dokumen.pdf -t 200 -o hasil.pdf
```
