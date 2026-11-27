# Deployment

This project is independent from the compressor webhook. Never sync it over the existing
`compresspdf-axum-webhook` directory or reuse that service's Docker Compose project name.

```bash
cp .env.example .env
docker compose build
docker compose up -d
curl -f http://127.0.0.1:8520/health
```

The Compose service binds only `127.0.0.1:8520` and does not register any Traefik labels.
Add a dedicated domain/reverse proxy explicitly when ready. Do not replace `pdf.multimedika.id`.
Container runs non-root, read-only, with writable temporary mounts (`pdf520_data:/var/lib/520pdf` for `DATA_DIR` Argon2id accounts, tokens, and CORS settings), 2 GiB memory,
2 CPUs and a 256-process ceiling. Upload request limit: 50 MiB. Processing concurrency: 2.
API CORS defaults to `*` (`CORS_ORIGINS=*`) and can be restricted via `.env` or `/api-keys`.

For untrusted/public deployment, isolate converter workers with restricted network egress,
HTTPS and request rate limiting. Default bridge networking alone does not block SSRF from
malicious Office documents. AI requires allowlisted provider egress and explicit job consent.
Do not mount host documents, credentials, Docker sockets or private SSH keys.

`cargo run --release` binds to loopback on port 8080 by default. It reads environment
variables directly, not `.env` automatically. Docker Compose loads `.env`.
