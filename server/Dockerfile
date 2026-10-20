# Island store server — production image.
# Build from the REPOSITORY ROOT (the server depends on ../crates/island-formats):
#   docker build -f server/Dockerfile -t island-store .

# --- Admin panel (Vue) ------------------------------------------------------------------------
FROM node:22-bookworm-slim AS admin
WORKDIR /admin
RUN npm install -g pnpm@11
COPY server/admin/package.json server/admin/pnpm-lock.yaml ./
RUN pnpm install --frozen-lockfile
COPY server/admin/ ./
RUN pnpm build

# --- Server (Rust) ----------------------------------------------------------------------------
FROM rust:1.94-bookworm AS server
WORKDIR /src
COPY crates/island-formats crates/island-formats
COPY server/Cargo.toml server/Cargo.lock server/
COPY server/src server/src
COPY server/migrations server/migrations
RUN cargo build --release --locked --manifest-path server/Cargo.toml

# --- Runtime ----------------------------------------------------------------------------------
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --home /app island
WORKDIR /app
COPY --from=server /src/server/target/release/island-store /app/island-store
COPY --from=admin /admin/dist /app/admin/dist
USER island
ENV BIND=0.0.0.0:8080 \
    ADMIN_DIST=/app/admin/dist
EXPOSE 8080
# Migrations are embedded in the binary and run at startup.
CMD ["/app/island-store"]
