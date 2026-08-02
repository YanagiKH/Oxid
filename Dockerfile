FROM rust:1-slim-bookworm AS builder
RUN apt-get update \
    && apt-get install --yes --no-install-recommends g++ \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /build
COPY . .
RUN cargo build --release --locked

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install --yes --no-install-recommends libstdc++6 \
    && useradd --create-home --uid 10001 oxid \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/target/release/oxid /usr/local/bin/oxid
USER oxid
WORKDIR /workspace
ENTRYPOINT ["oxid"]
CMD ["help"]
