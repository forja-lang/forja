# Forja — entorno reproducible (F7-T1)
#
# Imagen base para desarrollo y CI local. Reproduce el toolchain Linux que usa
# GitHub Actions (ubuntu-latest) sin depender de WSL ni de cargo.exe.
FROM rust:1.85-bookworm

RUN apt-get update && apt-get install -y --no-install-recommends \
        build-essential \
        ca-certificates \
        clang \
        cmake \
        curl \
        git \
        libssl-dev \
        llvm \
        pkg-config \
        python3 \
        sqlite3 \
    && rm -rf /var/lib/apt/lists/*

# Componentes usados por scripts/verify.sh y asm-coverage.sh.
RUN rustup component add rustfmt clippy llvm-tools-preview

WORKDIR /workspace
COPY . .
RUN cargo fetch --locked

CMD ["bash", "scripts/verify.sh"]
