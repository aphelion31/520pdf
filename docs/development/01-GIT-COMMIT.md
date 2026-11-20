# Commit Convention

Agent tidak boleh commit, push, buat PR, atau merge kecuali user memintanya secara eksplisit.

## Bahasa

Semua pesan commit (subject, body, footer) dalam bahasa Inggris. Identifier, path route,
dan teks yang dikutip tetap apa adanya, misalnya `/compress` atau `X-Compress-Status`.

## Format

[Conventional Commits](https://www.conventionalcommits.org/):

```text
<type>(<scope>): <subject>

<body>

Refs: PDF-xxx
```

### Type

| Type | Kegunaan |
| --- | --- |
| `feat` | Kapabilitas baru (endpoint, opsi kompresi, header respons). |
| `fix` | Perbaikan bug. |
| `refactor` | Perubahan kode tanpa fitur baru atau perbaikan bug. |
| `perf` | Peningkatan performa (kecepatan kompresi, ukuran hasil, memori). |
| `docs` | Dokumentasi saja. |
| `test` | Menambah atau memperbaiki test. |
| `build` | Dependency (Cargo) atau Docker image. |
| `ci` | GitHub Actions / CI. |
| `chore` | Pemeliharaan tanpa perubahan perilaku aplikasi. |
| `revert` | Membatalkan commit sebelumnya. |

### Scope

Area lowercase: `webhook`, `auth`, `compress`, `image`, `font`, `cli`, `docker`, `config`, dan sebagainya. Kosongkan bila lintas area.

### Subject

- Imperative ("add", "fix", "remove"), huruf kecil, tanpa titik akhir.
- Maksimal 72 karakter termasuk type dan scope.
- Jelaskan hasil, bukan aktivitas.

### Body

Jelaskan **mengapa** dan dampaknya. Wrap sekitar 72 karakter. Sebutkan env baru atau perubahan Docker yang perlu dilakukan saat deploy.

### Footer

- `Refs: PDF-xxx`, sama dengan nomor branch.
- Breaking change: `BREAKING CHANGE: <apa yang rusak dan cara migrasi>` atau `!` setelah type.

## Merge commit PR

| PR | Subject merge commit (wajib, body kosong) |
| --- | --- |
| Fitur -> `staging` | `improved: #<nomor PR>` |
| `staging` -> `master` | `release(project): bump to vX.Y.Z` (lihat [04-RELEASE-FLOW.md](04-RELEASE-FLOW.md)) |

```bash
gh pr merge <N> --merge --subject "improved: #<N>" --body ""
```

## Cakupan commit

- Sebelum commit: `git pull origin staging` di branch fitur, selesaikan konflik di sana.
- Satu commit per perubahan koheren. Stage file eksplisit, tinjau `git diff --cached`.
- **Jangan commit**: `.env*` (kecuali `.env.example`), `target/`, `.DS_Store`, file PDF uji, kredensial.
- Hindari `git add -A`, amend, rebase, force-push tanpa kebutuhan jelas dan izin.
