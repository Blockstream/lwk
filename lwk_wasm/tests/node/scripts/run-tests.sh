#!/usr/bin/env bash

set -uo pipefail

DIR="$(cd "$(dirname "$0")/.." && pwd)"
FAILED=0

for f in "$DIR"/*.js; do
    name="$(basename "$f")"
    echo "RUN  $name"
    if node "$f"; then
        echo "PASS $name"
    else
        echo "FAIL $name"
        FAILED=1
    fi
done

if [ "$FAILED" -eq 1 ]; then
    echo "Some tests failed"
    exit 1
fi

echo "All tests passed"
