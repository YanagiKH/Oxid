FROM rust:1-slim-bookworm AS builder
WORKDIR /build
COPY . .
RUN cargo build --release --locked

FROM debian:bookworm-slim
RUN useradd --create-home --uid 10001 oxid
COPY --from=builder /build/target/release/oxid /usr/local/bin/oxid
USER oxid
WORKDIR /workspace
ENTRYPOINT ["oxid"]
CMD ["help"]
