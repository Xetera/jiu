FROM rust:1.53.0 as builder
ENV NAME=rust-docker

# First build a dummy project with our dependencies to cache them in Docker
WORKDIR /usr/src
RUN cargo new --bin ${NAME}
WORKDIR /usr/src/${NAME}
COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml
RUN cargo build --release
RUN rm src/*.rs

# Now copy the sources and do the real build
COPY . .
RUN cargo test
RUN cargo build --release 

# Second stage putting the build result into a debian jessie-slim image
FROM debian:jessie-slim
ENV NAME=rust-docker

COPY --from=builder /usr/src/${NAME}/target/release/${NAME} /usr/local/bin/${NAME}
CMD ${NAME}
