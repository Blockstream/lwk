# Use the same base image as your current CI
FROM nixos/nix:latest

WORKDIR /app

# We are doing things in steps to avoid a single huge layer

# Copy flake files and rust-toolchain to download dependencies
COPY flake.nix flake.lock rust-toolchain.toml ./

# Download flake inputs (nixpkgs, rust-overlay, etc)
RUN nix --extra-experimental-features 'nix-command flakes' flake archive

# Copy only what is needed to build mdbook-snippets to trigger rust toolchain download
COPY docs/snippets/processor ./docs/snippets/processor

# Build mdbook-snippets to download and cache the rust toolchain
RUN nix --extra-experimental-features 'nix-command flakes' build .#mdbook-snippets

# Copy the rest of the source code
COPY . .

# Build the main application
RUN nix --extra-experimental-features 'nix-command flakes' build .
