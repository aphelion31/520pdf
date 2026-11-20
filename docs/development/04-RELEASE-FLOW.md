# Release Flow: staging -> master

Lihat juga [01-GIT-COMMIT.md](01-GIT-COMMIT.md), [02-GIT-BRANCH.md](02-GIT-BRANCH.md), [03-GIT-PR-FLOW.md](03-GIT-PR-FLOW.md).

## Branch

| Branch | Isi | Deploy |
|---|---|---|
| `staging` | Hasil merge semua fitur/fix | Server staging (Docker Compose) |
| `master` | Versi rilis | Server master (Docker Compose) |

Deploy dilakukan user. Merge PR tidak sama dengan deploy.

## 1. Fitur / perbaikan ke staging

1. Cek nomor lalu buat branch dari `origin/staging`: `PDF-xxx/<type>-<kebab-case>` (feat-/fix-/improvement-/…, mulai 001, lihat 02).
2. Commit Conventional Commits (English) + `Refs: PDF-xxx`.
3. PR ke `staging` dengan judul `pr: <nama>`, merge dengan subject `improved: #<nomor PR>`, body kosong.

## 2. Rilis ke master

1. Buat PR **`staging` -> `master`** dengan judul:

   ```
   pr: ready to master
   ```

   Isi body: daftar perubahan (Fixes / Features) dengan nomor PR, plus catatan deploy
   (env baru, perubahan `docker-compose.yaml`, rebuild image).

2. **Sebelum merge, tentukan versi** (`vMAJOR.MINOR.PATCH`) dari isi PR:

   | Naik | Kapan | Contoh |
   |---|---|---|
   | **MAJOR** (`v1.0.0`) | Dirombak total: kontrak API tidak kompatibel dengan versi lama | ganti endpoint/field wajib |
   | **MINOR** (`v0.2.0`) | Perubahan kecil-menengah: fitur baru, perbaikan perilaku, header/field baru | opsi `target_kb`, header baru |
   | **PATCH** (`v0.1.1`) | Ringan sekali: typo, perbaikan kecil satu titik, tanpa fitur baru | salah pesan error |

   Kalau dalam satu rilis ada beberapa jenis, ambil yang paling besar.
   Naik MAJOR -> MINOR dan PATCH kembali 0. Naik MINOR -> PATCH kembali 0.
   Samakan `version` di `Cargo.toml` dengan versi rilis (commit `release(project): bump to vX.Y.Z` di `staging` sebelum PR).

3. **Merge** PR dengan subjek commit merge **wajib**:

   ```
   release(project): bump to vX.Y.Z
   ```

4. **Wajib pastikan tag dan GitHub Release `vX.Y.Z` terbit.** Workflow `Auto Release`
   (`.github/workflows/release.yaml`) membuatnya otomatis dari subject merge. Verifikasi:

   ```bash
   gh run list --workflow "Auto Release" -L 1
   gh release view vX.Y.Z --json tagName,publishedAt
   ```

   Bila workflow gagal atau tag tidak ada, buat manual:

   ```bash
   gh release create vX.Y.Z --target master --title vX.Y.Z --generate-notes --latest
   ```

   Rilis belum dianggap selesai sebelum tag dan Release terbit.

5. Deploy master oleh user (lihat [03-GIT-PR-FLOW.md](03-GIT-PR-FLOW.md#deploy)). Set `APP_VERSION=X.Y.Z` di `.env`.

## Riwayat versi

| Versi | Tanggal | Ringkasan |
|---|---|---|
| v0.1.0 | - | Rilis pertama: CLI `compress-pdf` + webhook Axum `POST /compress`, Docker Compose + Traefik |
