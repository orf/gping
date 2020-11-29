FROM rust as builder
WORKDIR /usr/src/gping
COPY src/ src/
COPY Cargo.* ./
RUN cargo install --path .

FROM debian:buster-slim
RUN apt-get update && apt-get install -y inetutils-ping && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/cargo/bin/gping /usr/local/bin/gping
ENTRYPOINT ["gping"]
