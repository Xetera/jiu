FROM rust:1.55.0-buster as builder

# First build a dummy project with our dependencies to cache them in Docker
WORKDIR /usr/src
RUN cargo new --bin builder
WORKDIR /usr/src/builder
COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml
RUN --mount=type=cache,target=/usr/local/cargo/registry cargo build --release
RUN rm src/*.rs

# Now copy the sources and do the real build
ADD src src
ADD sqlx-data.json sqlx-data.json
ENV SQLX_OFFLINE true

RUN cargo build --release

# Second stage putting the build result into a debian jessie-slim image
FROM debian:buster-slim

RUN apt-get update \
    && apt-get install -y ca-certificates tzdata libc6 \
    && rm -rf /var/lib/apt/lists/*
ENV NAME=rust-docker
ENV RUST_LOG=jiu=debug
COPY --from=builder /usr/src/builder/target/release/jiu /usr/local/bin/jiu
CMD ["/usr/local/bin/jiu"]
