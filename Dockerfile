# Tansk: builds the WASM client (trunk) and the server, then runs the server,
# which serves the client at / and the game at /ws. Railway sets PORT.
FROM rust:1-bookworm AS build
ARG TRUNK_VERSION=0.21.14
RUN rustup target add wasm32-unknown-unknown \
 && curl -fsSL "https://github.com/trunk-rs/trunk/releases/download/v${TRUNK_VERSION}/trunk-x86_64-unknown-linux-gnu.tar.gz" \
    | tar -xz -C /usr/local/bin
WORKDIR /src
COPY . .
RUN cd crates/tansk-web && trunk build --release --dist /src/dist
RUN cargo build --release -p tansk-server

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/tansk-server /usr/local/bin/tansk-server
COPY --from=build /src/dist /app/dist
ENV TANSK_STATIC_DIR=/app/dist PORT=8080
EXPOSE 8080
CMD ["tansk-server"]
