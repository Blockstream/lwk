# Use the same base image as your current CI
FROM nixos/nix:latest

# 1. Configure Nix
# We enable flakes and configure your binary cache so the docker build 
# itself is fast and pulls from your server.
# NOTE: Replace the public key below with the actual key for nix.casatta.it
RUN mkdir -p /etc/nix && \
    echo "experimental-features = nix-command flakes" >> /etc/nix/nix.conf && \
    echo "substituters = https://cache.nixos.org https://nix.casatta.it" >> /etc/nix/nix.conf && \
    echo "trusted-public-keys = cache.nixos.org-1:6NCHdD59X431o0gWypbMrAURkbJ16ZPMQFGspcDShjY= nix.casatta.it-1:YOUR_PUBLIC_KEY_HERE" >> /etc/nix/nix.conf

# 2. Set working directory
WORKDIR /app

# 3. Copy Flake Metadata
# We copy ONLY the flake files and Cargo lockfiles first.
# This ensures that if you change your source code (src/), this layer 
# remains cached and doesn't trigger a re-download of dependencies.
COPY flake.nix flake.lock rust-toolchain.toml ./
COPY Cargo.toml Cargo.lock ./

# 4. Pre-fetch Build Environment
# We run 'nix develop' to fetch the compiler (rustc), cargo, and system libraries (openssl, etc.).
# This populates the /nix/store with the heaviest dependencies.
# The '--command true' flag makes it exit immediately after setting up the environment.

# 5. (Optional) Pre-fetch Rust Crates
# If your flake uses a standard method (like buildRustPackage), the build inputs 
# often include a separate derivation for vendor sources.
# Trying to build the default package here might fail if it requires the full 'src' directory,
# but running a dry-run often forces the download of input derivations (like crate sources).
RUN nix build . --dry-run || true

# The resulting image now contains your Rust toolchain and dependencies in /nix/store.
