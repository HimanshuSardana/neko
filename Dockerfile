# ---- build ----
FROM rust:1-bookworm AS build
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY migrations ./migrations
RUN cargo build --release

# ---- runtime ----
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=build /app/target/release/neko /app/neko
ENV BIND_ADDR=0.0.0.0:3000 \
    DATABASE_URL=sqlite:///app/data/neko.db?mode=rwc \
    MEDIA_ROOT=/app/media \
    RUST_LOG=neko=debug
VOLUME ["/app/media", "/app/data"]
EXPOSE 3000
CMD ["/app/neko"]
