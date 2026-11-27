FROM oven/bun:1.4.4-slim AS web-builder
WORKDIR /app/web
COPY web/package.json web/bun.lock ./
RUN bun install --frozen-lockfile
COPY web ./
RUN bun run build

FROM rust:1.98.1-bookworm AS builder
RUN apt-get update && apt-get install -y --no-install-recommends nasm cmake make g++ pkg-config && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs && cargo build --release --locked && rm -rf src
COPY ./src ./src
RUN touch src/main.rs && cargo build --release --locked

FROM ubuntu:24.04
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl ghostscript qpdf poppler-utils imagemagick tesseract-ocr \
    tesseract-ocr-eng tesseract-ocr-ind tesseract-ocr-vie ocrmypdf img2pdf mupdf-tools \
    libreoffice-writer libreoffice-calc libreoffice-impress python3-matplotlib fonts-liberation fonts-dejavu-core fonts-noto-cjk default-jre-headless unzip gnupg \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 1001 circleit && useradd --uid 1001 --gid circleit --create-home circleit \
    && install -d -o circleit -g circleit -m 700 /tmp/pdf520 /var/lib/520pdf
COPY docker/axum/verapdf-install.xml /tmp/verapdf-install.xml
RUN set -eux; \
    base=https://software.verapdf.org/releases/1.30/verapdf-greenfield-1.30.4-installer.zip; \
    curl -fsSL "$base" -o /tmp/vera.zip; \
    curl -fsSL "$base.asc" -o /tmp/vera.zip.asc; \
    curl -fsSL https://software.verapdf.org/keys/KEY -o /tmp/vera.key; \
    export GNUPGHOME=/tmp/vera-gpg; mkdir -m 700 "$GNUPGHOME"; \
    gpg --batch --import /tmp/vera.key; \
    gpg --batch --with-colons --fingerprint | grep -q '13DD102B4DD69354D12DE5A83184863278B17FE7'; \
    gpg --batch --status-fd 1 --verify /tmp/vera.zip.asc /tmp/vera.zip | grep -q 'VALIDSIG 13DD102B4DD69354D12DE5A83184863278B17FE7'; \
    unzip -q /tmp/vera.zip -d /tmp/vera; \
    java -jar $(find /tmp/vera -name '*.jar' | head -1) /tmp/verapdf-install.xml; \
    ln -s /opt/verapdf/verapdf /usr/local/bin/verapdf; \
    rm -rf /tmp/vera /tmp/vera.zip /tmp/vera.zip.asc /tmp/vera.key /tmp/vera-gpg /tmp/verapdf-install.xml
WORKDIR /srv/520pdf
COPY --from=builder /app/target/release/pdf520 /usr/local/bin/pdf520
COPY --from=web-builder /app/web/build/client /srv/520pdf/web/build/client
COPY tools/smoke.sh /usr/local/share/pdf520/smoke.sh
COPY tests/conversions-smoke.sh /usr/local/share/pdf520/conversions-smoke.sh
ENV TMPDIR=/tmp/pdf520 DATA_DIR=/var/lib/520pdf PORT=8080 BIND_ADDR=0.0.0.0:8080 WEB_DIR=/srv/520pdf/web/build/client HOME=/home/circleit
USER circleit
EXPOSE 8080
HEALTHCHECK --interval=15s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -sf http://127.0.0.1:8080/ready >/dev/null || exit 1
ENTRYPOINT ["pdf520"]
CMD ["serve"]
