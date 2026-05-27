#!/usr/bin/env python3
"""Generate LLM-friendly documentation artifacts for the published docs site."""

from __future__ import annotations

import argparse
import re
from pathlib import Path


DOCS_DIR = Path(__file__).resolve().parent
REPO_DIR = DOCS_DIR.parent
SRC_DIR = DOCS_DIR / "src"
BOOK_DIR = DOCS_DIR / "book"
SUMMARY = SRC_DIR / "SUMMARY.md"
DEPLOY_URL = "https://blockstream.github.io/lwk/book"
SOURCE_URL = "https://github.com/blockstream/lwk"

SUMMARY_TITLE_RE = re.compile(r"\[([^\]]+)\]")
SUMMARY_TARGET_RE = re.compile(r"\(([^)]*\.md)\)")
INCLUDE_RE = re.compile(r"\{\{#include\s+([^}]+)\}\}")
TAB_TITLE_RE = re.compile(r'<div slot="title">(.+)</div>')
MARKDOWN_LINK_RE = re.compile(r"(!?)\[([^\]]*)\]\(([^)]+)\)")
HEADING_RE = re.compile(r"^(#{1,6})(\s+.+)$")
IGNORE_FENCE_RE = re.compile(r"^(```[A-Za-z0-9_#+-]+),ignore$", re.MULTILINE)


def slugify(text: str) -> str:
    text = text.strip().lower()
    text = re.sub(r"<[^>]+>", "", text)
    text = re.sub(r"[^\w\s-]", "", text)
    text = re.sub(r"[\s_]+", "-", text)
    return text.strip("-")


def summary_pages() -> list[tuple[str, Path]]:
    """Return all source markdown pages, with SUMMARY.md entries first."""
    seen: set[Path] = set()
    pages: list[tuple[str, Path]] = []

    for line in SUMMARY.read_text(encoding="utf-8").splitlines():
        title = SUMMARY_TITLE_RE.search(line)
        targets = SUMMARY_TARGET_RE.findall(line)
        if not title or not targets:
            continue

        path = (SRC_DIR / targets[-1]).resolve()
        if path not in seen:
            seen.add(path)
            pages.append((title.group(1), path))

    for path in sorted(SRC_DIR.glob("*.md")):
        if path.name == "SUMMARY.md":
            continue

        resolved = path.resolve()
        if resolved not in seen:
            seen.add(resolved)
            pages.append((path.stem, resolved))

    return pages


def page_title(path: Path, fallback: str) -> str:
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.startswith("# "):
            return line[2:].strip()
    return fallback


def page_has_body(path: Path) -> bool:
    lines = path.read_text(encoding="utf-8").splitlines()
    for index, line in enumerate(lines):
        if index == 0 and line.startswith("# "):
            continue
        if line.strip():
            return True
    return False


def include_file(current_page: Path, include: str) -> str:
    parts = include.strip().split(":")
    path = (current_page.parent / parts[0]).resolve()
    anchor = parts[1] if len(parts) > 1 and not parts[1].isdigit() else None
    lines = path.read_text(encoding="utf-8").splitlines()

    if anchor:
        start = None
        end = None
        for i, line in enumerate(lines):
            if f"ANCHOR: {anchor}" in line:
                start = i + 1
            elif f"ANCHOR_END: {anchor}" in line:
                end = i
                break

        if start is None or end is None:
            raise RuntimeError(f"anchor {anchor!r} not found in {path}")
        lines = lines[start:end]

    lines = [line for line in lines if "ANCHOR" not in line]
    return "\n".join(lines)


def expand_includes(text: str, current_page: Path) -> str:
    def replace(match: re.Match[str]) -> str:
        return include_file(current_page, match.group(1))

    return INCLUDE_RE.sub(replace, text)


def clean_tabs(text: str) -> str:
    cleaned: list[str] = []
    in_tabs = False
    keep_tab = False

    for line in text.splitlines():
        if line.startswith("<custom-tabs"):
            in_tabs = True
            keep_tab = False
            continue

        if line == "</custom-tabs>":
            in_tabs = False
            keep_tab = False
            continue

        title = TAB_TITLE_RE.fullmatch(line)
        if title:
            keep_tab = title.group(1) == "Rust"
            continue

        if in_tabs:
            if line in {"<section>", "</section>"}:
                continue
            if keep_tab:
                cleaned.append(line)
        elif line in {"<section>", "</section>"}:
            continue
        else:
            cleaned.append(line)

    return "\n".join(cleaned)


def rewrite_links(text: str, current_page: Path, slug_by_file: dict[str, str]) -> str:
    def replace(match: re.Match[str]) -> str:
        bang, label, target = match.groups()
        if "://" in target or target.startswith("#"):
            return match.group(0)

        if bang:
            path = (current_page.parent / target).resolve()
            if path.exists() and SRC_DIR in path.parents:
                target = path.relative_to(SRC_DIR).as_posix()
            return f"![{label}]({target})"

        target_path, separator, target_anchor = target.partition("#")
        name = Path(target_path).name
        if name not in slug_by_file:
            path = (current_page.parent / target_path).resolve()
            if not path.exists() and target_path.startswith("../"):
                path = (REPO_DIR / target_path.removeprefix("../")).resolve()
            if not path.exists() and target_path.startswith("./"):
                path = (REPO_DIR / target_path.removeprefix("./")).resolve()
            if path.exists() and REPO_DIR in path.parents:
                rel = path.relative_to(REPO_DIR).as_posix()
                kind = "tree" if path.is_dir() else "blob"
                anchor = f"#{target_anchor}" if separator else ""
                return f"[{label}]({SOURCE_URL}/{kind}/master/{rel}{anchor})"
            return match.group(0)

        slug = slug_by_file[name]
        if separator:
            slug = f"{slug}-{slugify(target_anchor)}"
        return f"[{label}](#{slug})"

    return MARKDOWN_LINK_RE.sub(replace, text)


def clean_code_fences(text: str) -> str:
    return IGNORE_FENCE_RE.sub(r"\1", text)


def demote_headings(text: str, title: str) -> str:
    lines = text.splitlines()
    output: list[str] = []
    skipped_title = False
    in_code_block = False

    for line in lines:
        if line.startswith("```"):
            in_code_block = not in_code_block
            output.append(line)
            continue

        if not in_code_block:
            if line == f"# {title}" and not skipped_title:
                skipped_title = True
                continue

            heading = HEADING_RE.match(line)
            if heading:
                level = 3 if len(heading.group(1)) == 1 else min(len(heading.group(1)) + 1, 6)
                line = f"{'#' * level}{heading.group(2)}"

        output.append(line)

    return "\n".join(output).strip()


def render_index(pages: list[tuple[str, Path]]) -> str:
    pages = [(fallback, path) for fallback, path in pages if page_has_body(path)]
    titles = {path: page_title(path, fallback) for fallback, path in pages}
    slug_by_file = {path.name: slugify(title) for path, title in titles.items()}

    lines = [
        "<!-- Generated by docs/generate_llms.py; do not edit manually. -->",
        "",
        "# LWK Documentation",
        "",
        "This file merges the LWK mdBook documentation into one Markdown document for LLM context. "
        "The human-readable HTML book is available at [./](./).",
        "",
        "## Table of Contents",
        "",
    ]

    for _, path in pages:
        title = titles[path]
        lines.append(f"- [{title}](#{slugify(title)})")

    for _, path in pages:
        title = titles[path]
        text = path.read_text(encoding="utf-8")
        text = expand_includes(text, path)
        text = clean_tabs(text)
        text = rewrite_links(text, path, slug_by_file)
        text = clean_code_fences(text)
        text = demote_headings(text, title)

        lines.extend(["", "---", "", f"## {title}", ""])
        if text:
            lines.append(text)

    return "\n".join(lines).rstrip() + "\n"


def render_llms(base_url: str) -> str:
    base_url = base_url.rstrip("/")
    return f"""# LWK Documentation

> Liquid Wallet Kit (LWK) is a Rust workspace with libraries and bindings for building Liquid Network wallets and applications.

This file follows the llms.txt proposal and points to the LLM-friendly Markdown version of the LWK documentation.

## Docs

- [LWK documentation]({base_url}/index.md): All mdBook documentation merged into a single Markdown page, with mdBook snippets expanded.

## Optional

- [LWK HTML book]({base_url}/): The human-readable mdBook version of the same documentation.
- [LWK source repository](https://github.com/blockstream/lwk): Source code for the Liquid Wallet Kit workspace.
"""


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--base-url", default=DEPLOY_URL)
    args = parser.parse_args()

    pages = summary_pages()
    BOOK_DIR.mkdir(exist_ok=True)
    (BOOK_DIR / "index.md").write_text(render_index(pages), encoding="utf-8")
    (BOOK_DIR / "llms.txt").write_text(render_llms(args.base_url), encoding="utf-8")


if __name__ == "__main__":
    main()
