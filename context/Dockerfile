FROM rust:1.85.0

RUN apt update -qq
RUN apt upgrade --no-install-recommends -yqq

# libxml2-utils for xmllint
# libudev-dev for jade serial
# clang for wasm
# libtinfo5 libncurses5 for swift
# chromium-driver for wasm tests
RUN apt install -y libudev-dev libxml2-utils clang libtinfo5 libncurses5 chromium-driver

RUN rustup component add rustfmt
RUN rustup component add clippy
RUN rustup component add llvm-tools-preview
RUN rustup target add wasm32-unknown-unknown
RUN rustup install nightly # for docs building

RUN cargo install cargo-audit
RUN cargo install cargo-nextest
RUN cargo install grcov
RUN cargo install just

# we don't need this, but it downloads and cache most of our dependency tree
RUN cargo install lwk_cli

COPY download_bins.sh /download_bins.sh
RUN ./download_bins.sh

# update the installation script with curl -fsSL https://get.docker.com -o get-docker.sh
COPY get-docker.sh /get-docker.sh
RUN sh ./get-docker.sh

# move following layers up once upper layers needs modification

RUN wget https://packages.microsoft.com/config/debian/12/packages-microsoft-prod.deb -O packages-microsoft-prod.deb && dpkg -i packages-microsoft-prod.deb && rm packages-microsoft-prod.deb
RUN apt update -qq && apt install -y dotnet-sdk-6.0

RUN cargo install uniffi-bindgen-cs --git https://github.com/RCasatta/uniffi-bindgen-cs --rev fa87c381f88c8cacd26cf3e91e5c63af60162c3f

# Install Node.js LTS using official binaries
RUN curl -fsSL https://nodejs.org/dist/v20.18.0/node-v20.18.0-linux-x64.tar.xz | tar -xJ -C /usr/local --strip-components=1

RUN cargo install uniffi-bindgen-cpp --git https://github.com/NordSecurity/uniffi-bindgen-cpp --rev f02896c3e9fdce2f374656a32c46ae14c0051a26 # v0.7.3+v0.28.3

RUN cargo install waterfalls --locked
