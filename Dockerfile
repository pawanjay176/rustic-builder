FROM rust:1.85-slim as builder

WORKDIR /usr/src/rustic-builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    perl \
    cmake \
    libclang-dev \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

COPY . .
RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/rustic-builder/target/release/rustic-builder /usr/local/bin/

EXPOSE 8560

ENTRYPOINT ["rustic-builder"]
