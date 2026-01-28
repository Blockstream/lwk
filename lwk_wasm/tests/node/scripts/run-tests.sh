#!/usr/bin/env bash

# Skip amp0-* tests: they require a live AMP0 backend and credentials.
set -euo pipefail

DIR="$(cd "$(dirname "$0")/.." && pwd)"

for f in "$DIR"/*.js; do
    name="$(basename "$f")"
    case "$name" in amp0-*) echo "SKIP $name"; continue;; esac
    echo "RUN  $name"
    node "$f"
done
