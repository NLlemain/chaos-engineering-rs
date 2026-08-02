FROM rust:slim-bookworm AS builder

RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build
COPY . .
RUN cargo build --locked --release -p chaos_cli

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates iproute2 iptables \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 10001 chaos

COPY --from=builder /build/target/release/chaos /usr/local/bin/chaos
COPY --from=builder /build/scenarios /opt/chaos/scenarios
COPY --from=builder /build/scenario-packs /opt/chaos/scenario-packs

USER chaos
WORKDIR /opt/chaos
EXPOSE 8080 9898
ENTRYPOINT ["chaos"]
CMD ["--help"]
