FROM rust:1.66-slim-bullseye as build
# install https://lib.rs/crates/cargo-build-dependencies to cache deps in a separate layer
RUN cargo install cargo-build-dependencies
RUN USER=root cargo new --bin pixum
WORKDIR /pixum
COPY Cargo.toml Cargo.lock ./
RUN cargo build-dependencies --release
# build application
COPY ./src ./src
RUN cargo build --release

FROM debian:bullseye-slim
RUN apt-get update && apt-get -y upgrade && apt-get autoremove
# run as non-root user
RUN useradd --create-home pixum --shell /bin/false
USER pixum
COPY --from=build /pixum/target/release/pixum .
ENTRYPOINT [ "./pixum" ]