# Pull Request Flow

Lihat [02-GIT-BRANCH.md](02-GIT-BRANCH.md) untuk nomor branch dan [01-GIT-COMMIT.md](01-GIT-COMMIT.md) untuk pesan commit.
Agent tidak boleh commit, push, membuat PR, merge, atau **deploy** kecuali user memintanya secara eksplisit.

## Sinkron dengan staging (wajib)

```bash
# sebelum commit (di branch PDF-xxx/<type>-<nama>)
git pull origin staging --no-rebase

# setelah PR di-merge
git switch staging
git pull origin staging
```

## Pull request ke staging

1. Cek nomor branch berikutnya, buat branch dari `origin/staging`.
2. Pemeriksaan lokal (lihat di bawah).
3. Judul PR wajib `pr: <nama>`: singkat, lowercase, bahasa Inggris. Contoh: `pr: axum webhook`.
4. Body: perubahan, pemeriksaan yang benar-benar dijalankan, risiko/limitasi, catatan deploy
   (env baru, perubahan `docker-compose.yaml`, rebuild image), dan koordinasi pemanggil webhook bila kontrak API berubah.
5. Merge memakai merge commit (bukan squash/rebase), subject wajib `improved: #<nomor PR>`, body kosong.

```bash
gh pr create --base staging --head PDF-xxx/<type>-<nama> --title "pr: <nama>" --body-file <file>
gh pr merge <N> --merge --subject "improved: #<N>" --body ""
```

> Merge lewat GitHub UI: ganti judul commit jadi `improved: #<N>` dan **kosongkan kolom deskripsi**.
> GitHub otomatis mengisinya dengan judul PR. Itu tidak boleh ada.
> Lebih aman pakai `gh pr merge ... --body ""` seperti di atas.

## Pemeriksaan lokal sebelum PR

```bash
cargo fmt --check
cargo clippy --release --all-targets -- -D warnings
cargo test
```

Bila menyentuh kompresi, jalankan juga pada satu PDF nyata dan laporkan ukuran sebelum/sesudah:

```bash
cargo run --release -- contoh.pdf          # hasil: compressed_contoh.pdf
```

Laporkan pemeriksaan yang tidak dijalankan.

## Deploy

Merge bukan deploy. Staging dan master di-deploy oleh user di server dengan Docker Compose:

```bash
git pull
docker compose up -d --build
docker compose ps                          # status axum harus healthy
```

- `.env` di server tidak di-commit; salin dari `.env.example` dan isi `WEBHOOK_SECRET` (`openssl rand -hex 32`).
- Naikkan `APP_VERSION` di `.env` sesuai tag rilis supaya image ber-tag versi.

## Urutan bila kontrak API berubah

Webhook dulu sampai `staging` dan ter-deploy, baru pemanggil (backend Laravel dsb.) ke `staging`,
supaya pemanggil tidak memakai field/header yang belum ada.

Rilis ke master: [04-RELEASE-FLOW.md](04-RELEASE-FLOW.md).
