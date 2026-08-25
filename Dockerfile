# The whole deployment: one small container that serves the compiled game and
# relays the lobby. The wasm bundle is built on the host by trunk and copied in,
# so the image never carries a Rust or wasm toolchain.

FROM rust:slim-bookworm AS build
WORKDIR /src
# Manifests first, so a source-only change reuses the dependency layer.
COPY Cargo.toml Cargo.lock ./
COPY proto/Cargo.toml proto/Cargo.toml
COPY server/Cargo.toml server/Cargo.toml
# The workspace root package is the game; cargo needs its target to exist to
# read the workspace, but never builds it here.
RUN mkdir -p src proto/src server/src \
 && echo 'fn main() {}' > src/main.rs \
 && echo '' > proto/src/lib.rs \
 && echo 'fn main() {}' > server/src/main.rs \
 && cargo build --release -p td-server \
 && rm -rf proto/src server/src
COPY proto/src proto/src
COPY server/src server/src
# Touch, or cargo keeps the stub build from the layer above.
RUN touch proto/src/lib.rs server/src/main.rs \
 && cargo build --release -p td-server

FROM debian:bookworm-slim
RUN useradd --uid 10001 --no-create-home --shell /usr/sbin/nologin app
WORKDIR /app
COPY --from=build /src/target/release/td-server /usr/local/bin/td-server
COPY dist /app/static
ENV TD_STATIC=/app/static
ENV PORT=8080
EXPOSE 8080
USER 10001
ENTRYPOINT ["/usr/local/bin/td-server"]
