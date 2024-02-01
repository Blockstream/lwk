FROM rust:1.74.1

RUN apt update -qq
RUN apt upgrade --no-install-recommends -yqq

# libxml2-utils for xmllint
# libudev-dev for jade serial
RUN apt install -y libudev-dev libxml2-utils

RUN rustup component add rustfmt
RUN rustup component add clippy
RUN rustup component add llvm-tools-preview
RUN rustup toolchain install 1.70.0

RUN cargo install cargo-audit
RUN cargo install cargo-nextest
RUN cargo install grcov
RUN cargo install just

COPY download_bins.sh /download_bins.sh
RUN ./download_bins.sh

# update the installation script with curl -fsSL https://get.docker.com -o get-docker.sh
COPY get-docker.sh /get-docker.sh
RUN sh ./get-docker.sh

COPY env.sh /env.sh

# move following layers up once upper layers needs modification

# needed for WASM
RUN apt install -y clang
RUN rustup target add wasm32-unknown-unknown
