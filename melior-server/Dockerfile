# Keep this version equal to rust-toolchain.toml.
FROM rust:1.99.1-bookworm AS build
WORKDIR /usr/src/melior-server
COPY . .
RUN cargo build --release --locked --bin melior-sfu

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && apt-get clean \
    && useradd --system --create-home --uid 10001 melior
COPY --from=build /usr/src/melior-server/target/release/melior-sfu /usr/local/bin/melior-sfu
USER melior
EXPOSE 5000
ENTRYPOINT ["melior-sfu"]
