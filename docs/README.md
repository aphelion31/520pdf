# Dokumentasi compresspdf-axum-webhook

## Development (git)

| Dokumen | Isi |
| --- | --- |
| [01-GIT-COMMIT.md](development/01-GIT-COMMIT.md) | Format commit (Conventional Commits, English, `Refs: PDF-xxx`), merge commit `improved: #N` |
| [02-GIT-BRANCH.md](development/02-GIT-BRANCH.md) | Branch `staging`/`master`, nama `PDF-xxx/<type>-<kebab-case>` (feat-/fix-/improvement-/…), nomor mulai 001 (wajib cek) |
| [03-GIT-PR-FLOW.md](development/03-GIT-PR-FLOW.md) | PR ke `staging` (`pr: <nama>`), pemeriksaan lokal, deploy via Docker Compose |
| [04-RELEASE-FLOW.md](development/04-RELEASE-FLOW.md) | Rilis `staging` -> `master` (`pr: ready to master`, `release(project): bump to vX.Y.Z`) |

Konvensi ini sama dengan `doktercares-laravel-backend/docs/development/`, kecuali prefix branch `PDF-xxx`.

## Aplikasi

| Dokumen | Isi |
| --- | --- |
| [API.md](API.md) | Endpoint webhook, env, contoh pemanggilan |
| [DEPLOY.md](DEPLOY.md) | Traefik, Nginx, Axum, produksi `pdf.multimedika.id` |
| [releases.md](releases.md) | Ringkasan aturan rilis master |
