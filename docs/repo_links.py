#!/usr/bin/env python3
"""Rewrite repository-relative links for rendered mdBook output."""

from __future__ import annotations

import json
from pathlib import Path
import sys

from link_utils import MARKDOWN_LINK_RE, repository_link


def rewrite_content(
    content: str,
    current_page: Path,
    repo_dir: Path,
    source_dir: Path,
    source_url: str,
    branch: str,
) -> str:
    """Rewrite links outside the book source to their repository URLs."""

    def replace(match):
        bang, label, target = match.groups()
        if bang:
            return match.group(0)

        link = repository_link(
            current_page, target, repo_dir, source_dir, source_url, branch
        )
        if link is None:
            return match.group(0)
        return f"[{label}]({link.url})"

    return MARKDOWN_LINK_RE.sub(replace, content)


def rewrite_sections(
    sections: list,
    source_dir: Path,
    repo_dir: Path,
    source_url: str,
    branch: str,
) -> None:
    """Rewrite all chapters in an mdBook JSON document."""
    for section in sections:
        chapter = section.get("Chapter")
        if chapter is None:
            continue

        source_path = chapter.get("source_path")
        if source_path:
            current_page = source_dir / source_path
            chapter["content"] = rewrite_content(
                chapter["content"],
                current_page,
                repo_dir,
                source_dir,
                source_url,
                branch,
            )
        rewrite_sections(
            chapter.get("sub_items", []),
            source_dir,
            repo_dir,
            source_url,
            branch,
        )


def main() -> None:
    if len(sys.argv) > 1 and sys.argv[1] == "supports":
        raise SystemExit(0 if len(sys.argv) > 2 and sys.argv[2] == "html" else 1)

    context, book = json.load(sys.stdin)
    root = Path(context["root"]).resolve()
    config = context["config"]
    source_dir = (root / config.get("book", {}).get("src", "src")).resolve()
    repo_dir = root.parent.resolve()
    preprocessor = config.get("preprocessor", {}).get("repo-links", {})
    html = config.get("output", {}).get("html", {})
    source_url = preprocessor.get(
        "repository-url", html.get("git-repository-url", "")
    )
    branch = preprocessor.get("branch", "master")
    if not source_url:
        raise RuntimeError("repo-links requires repository-url or git-repository-url")

    rewrite_sections(book["sections"], source_dir, repo_dir, source_url, branch)
    json.dump(book, sys.stdout)


if __name__ == "__main__":
    main()
