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
COPY --from=build /pixum/target/release/pixum .
ENTRYPOINT [ "./pixum" ]