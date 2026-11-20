# Branch Convention

| Branch | Kegunaan |
| --- | --- |
| `staging` | Branch integrasi. Deploy ke server staging. |
| `master` | Branch rilis. Deploy ke server production. |
| `PDF-xxx/<type>-<xx>` | Branch kerja. `<type>` menyatakan jenis perubahan (lihat di bawah). |
| `main` | Branch awal repo (Initial commit). Tidak dipakai untuk pekerjaan baru. |

## Nama branch: `PDF-xxx/<type>-<xx>`

`<type>` wajib, diikuti apa yang di-feat / di-fix / di-improve, dalam kebab-case bahasa Inggris.
`xxx` selalu tiga digit (`001`, `002`, …, `099`, `100`).

| Prefix | Kapan | Contoh |
| --- | --- | --- |
| `feat-` | Fitur atau kapabilitas baru | `PDF-001/feat-axum-webhook` |
| `fix-` | Perbaikan bug | `PDF-002/fix-empty-pdf-200` |
| `improvement-` | Peningkatan fitur yang sudah ada (UX, perilaku, performa ringan) | `PDF-003/improvement-higher-dpi` |
| `refactor-` | Ubah struktur kode tanpa ubah perilaku | `PDF-004/refactor-font-strip` |
| `perf-` | Optimasi performa (kecepatan, ukuran hasil, memori) | `PDF-005/perf-parallel-quality-search` |
| `docs-` | Dokumentasi saja | `PDF-006/docs-api-examples` |
| `chore-` | Pemeliharaan, dependency, config, CI, Docker | `PDF-007/chore-upgrade-axum` |
| `test-` | Menambah/memperbaiki test saja | `PDF-008/test-webhook-auth` |
| `hotfix-` | Perbaikan darurat yang langsung menuju master | `PDF-009/hotfix-upload-limit` |

Prefix branch menentukan type commit utamanya: `feat-` -> `feat(...)`, `fix-`/`hotfix-` -> `fix(...)`,
`improvement-` -> `feat(...)` atau `fix(...)` sesuai isinya, sisanya sama dengan namanya.

## Nomor branch (wajib cek sebelum membuat branch baru)

Nomor dimulai dari **001** (`PDF-001` ke atas). `xxx` = nomor terbesar yang sudah dipakai + 1.

```bash
git fetch --prune origin
# nomor terbesar dari branch remote + branch PR (termasuk yang sudah merged/closed)
{ git branch -r; gh pr list --state all -L 500 --json headRefName --jq '.[].headRefName'; } \
  | grep -oE 'PDF-[0-9]{3}' | sort -t- -k2 -n -u | tail -1
```

Ambil hasilnya + 1. Contoh: terbesar `PDF-007` -> branch baru `PDF-008/<type>-<kebab-case>`.
Kalau belum ada hasil sama sekali, mulai dari `PDF-001`.
Satu branch per PR; branch yang PR-nya sudah di-merge tidak dipakai ulang.

Buat branch dari `origin/staging`:

```bash
git switch -c PDF-008/fix-<kebab-case> origin/staging
```
