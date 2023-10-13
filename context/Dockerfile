FROM rust:1.70

RUN apt update -qq
RUN apt upgrade --no-install-recommends -yqq
RUN apt install -y libudev-dev

RUN rustup component add rustfmt
RUN rustup component add clippy
RUN rustup component add llvm-tools-preview

RUN cargo install cargo-audit
RUN cargo install grcov

COPY download_bins.sh /download_bins.sh
RUN ./download_bins.sh

# update the installation script with curl -fsSL https://get.docker.com -o get-docker.sh
COPY get-docker.sh /get-docker.sh
RUN sh ./get-docker.sh