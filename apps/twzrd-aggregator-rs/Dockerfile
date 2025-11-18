FROM rust:1.79 as builder
WORKDIR /app
COPY . .
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
RUN cargo build --release

FROM debian:bookworm-slim
RUN useradd -ms /bin/bash app
WORKDIR /srv
COPY --from=builder /app/target/release/twzrd-aggregator-rs /usr/local/bin/twzrd-aggregator-rs
ENV RUST_LOG=info
USER app
CMD ["/usr/local/bin/twzrd-aggregator-rs"]

