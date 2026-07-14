FROM rust:1.80-slim-bookworm AS builder

WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/rem0te-server /app/rem0te-server
EXPOSE 8080 21116 21117 21118 21119
CMD ["/app/rem0te-server"]
