#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

python3 -m unittest discover -s tests
mdbook build
python3 generate_llms.py
python3 check_links.py
