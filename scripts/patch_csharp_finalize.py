#!/usr/bin/env python3

"""Patch generated C# bindings to avoid the special `Finalize` member name."""

from __future__ import annotations

import re
import sys
from pathlib import Path


PATTERN = re.compile(r"\bFinalize(\s*\()")
REPLACEMENT = r"FinalizeLwk\1"


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: patch_csharp_finalize.py <path-to-lwk.cs>", file=sys.stderr)
        return 1

    path = Path(sys.argv[1])
    content = path.read_text(encoding="utf-8")
    patched, replacements = PATTERN.subn(REPLACEMENT, content)

    if replacements == 0:
        print(f"error: no Finalize members found in {path}", file=sys.stderr)
        return 1

    path.write_text(patched, encoding="utf-8")
    print(f"patched {replacements} Finalize occurrences in {path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
