FROM rust:1.58-buster as builder
ADD . /app
WORKDIR /app
RUN cd /app/docbot-controller && cargo build --release

FROM debian:buster-slim
RUN apt-get update \
    && apt-get install -y libssl1.1 ca-certificates\
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/docbot-controller /srv/docbot/docbot-controller
WORKDIR /srv/docbot
ENTRYPOINT ["/srv/docbot/docbot-controller"]
