FROM rust:1.98.1-bookworm AS builder
RUN apt-get update && apt-get install -y --no-install-recommends nasm cmake make g++ pkg-config && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs && cargo build --release --locked && rm -rf src
COPY ./src ./src
COPY ./static ./static
RUN touch src/main.rs && cargo build --release --locked

FROM ubuntu:24.04
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl ghostscript qpdf poppler-utils imagemagick tesseract-ocr \
    tesseract-ocr-eng tesseract-ocr-ind tesseract-ocr-vie ocrmypdf img2pdf mupdf-tools \
    libreoffice-writer libreoffice-calc libreoffice-impress python3-weasyprint fonts-liberation fonts-dejavu-core \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 1001 circleit && useradd --uid 1001 --gid circleit --create-home circleit \
    && install -d -o circleit -g circleit -m 700 /tmp/pdf520
COPY --from=builder /app/target/release/pdf520 /usr/local/bin/pdf520
COPY tests/conversions-smoke.sh /usr/local/share/pdf520/conversions-smoke.sh
ENV TMPDIR=/tmp/pdf520 PORT=8080 BIND_ADDR=0.0.0.0:8080 HOME=/home/circleit
USER circleit
EXPOSE 8080
ENTRYPOINT ["pdf520"]
CMD ["serve"]
