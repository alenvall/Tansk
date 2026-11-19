# TANSK 2026: builds the WASM client (trunk) and the server, then runs the
# server, which serves the client at / and the game at /ws/arena. Railway
# sets PORT.
#
# Dependencies are compiled in their own layers (cargo-chef), so a rebuild
# after a source change only compiles the workspace crates.

FROM rust:1-bookworm AS chef
ARG TRUNK_VERSION=0.21.14
RUN rustup target add wasm32-unknown-unknown \
 && cargo install cargo-chef --version 0.1.78 --locked \
 && curl -fsSL "https://github.com/trunk-rs/trunk/releases/download/v${TRUNK_VERSION}/trunk-x86_64-unknown-linux-gnu.tar.gz" \
    | tar -xz -C /usr/local/bin \
 && apt-get update && apt-get install -y --no-install-recommends brotli && rm -rf /var/lib/apt/lists/*
WORKDIR /src

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS build
COPY --from=planner /src/recipe.json recipe.json
RUN cargo chef cook --release -p tansk-server --recipe-path recipe.json \
 && cargo chef cook --release --target wasm32-unknown-unknown -p arena-client --recipe-path recipe.json
COPY . .
# trunk fetches the matching wasm-bindgen and wasm-opt itself.
RUN cd crates/arena-client && trunk build --release --dist /src/dist
# Precompressed copies; the server sends them to browsers that accept them.
RUN find /src/dist -type f \( -name '*.wasm' -o -name '*.js' -o -name '*.html' -o -name '*.css' \) \
      -exec gzip -k -9 {} \; -exec brotli -k -q 11 {} \;
RUN cargo build --release -p tansk-server

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/tansk-server /usr/local/bin/tansk-server
COPY --from=build /src/dist /app/dist
ENV TANSK_STATIC_DIR=/app/dist PORT=8080
EXPOSE 8080
CMD ["tansk-server"]
