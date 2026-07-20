# syntax=docker/dockerfile:1.7

ARG RUST_VERSION=1.94.1

FROM rust:${RUST_VERSION}-bookworm AS builder

WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends clang libssl-dev pkg-config \
    && rm -rf /var/lib/apt/lists/*

RUN rustup target add wasm32-unknown-unknown
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo install trunk --version 0.21.14 --locked

COPY Cargo.toml Cargo.lock ./
COPY core_lib ./core_lib
COPY ui_lib ./ui_lib
COPY infra_lib ./infra_lib
COPY desktop_app ./desktop_app
COPY data_tools ./data_tools
COPY web_back_end ./web_back_end
COPY web_front_end ./web_front_end
COPY img ./img

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    trunk build --config web_front_end/Trunk.toml --release
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    cargo build --release -p web_back_end \
    && mkdir -p /build-output \
    && cp /app/target/release/web_back_end /build-output/tallytail-web

FROM debian:bookworm-slim AS runtime

WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libssl3 \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir -p /app/data

ENV PORT=8080
ENV TALLYTAIL_DATA_DIR=/app/data

COPY --from=builder /build-output/tallytail-web /usr/local/bin/tallytail-web
COPY --from=builder /app/web_front_end/dist /app/web_front_end/dist
COPY --from=builder /app/img /app/img

EXPOSE 8080

CMD ["tallytail-web"]
