#!/usr/bin/env python3
"""Check local source links and links in generated documentation artifacts."""

from __future__ import annotations

from html.parser import HTMLParser
from pathlib import Path
import sys
from urllib.parse import unquote, urlsplit

from link_utils import MARKDOWN_LINK_RE, local_target, slugify


DOCS_DIR = Path(__file__).resolve().parent
SRC_DIR = DOCS_DIR / "src"
BOOK_DIR = DOCS_DIR / "book"


class HtmlLinks(HTMLParser):
    def __init__(self) -> None:
        super().__init__()
        self.hrefs: list[str] = []
        self.ids: set[str] = set()

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        values = dict(attrs)
        if values.get("id"):
            self.ids.add(values["id"] or "")
        if tag == "a" and values.get("href"):
            self.hrefs.append(values["href"] or "")


def check_source_links() -> list[str]:
    errors: list[str] = []
    for page in sorted(SRC_DIR.glob("*.md")):
        text = page.read_text(encoding="utf-8")
        for match in MARKDOWN_LINK_RE.finditer(text):
            target = match.group(3)
            path = local_target(page, target)
            if path is not None and not path.exists():
                errors.append(f"{page.relative_to(DOCS_DIR)}: missing {target}")
    return errors


def load_html() -> dict[Path, HtmlLinks]:
    pages: dict[Path, HtmlLinks] = {}
    for page in BOOK_DIR.rglob("*.html"):
        parser = HtmlLinks()
        parser.feed(page.read_text(encoding="utf-8"))
        pages[page.resolve()] = parser
    return pages


def check_html_links() -> list[str]:
    errors: list[str] = []
    pages = load_html()
    for page, parser in pages.items():
        for href in parser.hrefs:
            parsed = urlsplit(href)
            if parsed.scheme or parsed.netloc or href.startswith(("mailto:", "javascript:")):
                continue

            target = page if not parsed.path else (page.parent / unquote(parsed.path)).resolve()
            if target.is_dir():
                target = target / "index.html"
            if not target.is_file():
                errors.append(f"{page.relative_to(BOOK_DIR)}: missing {href}")
                continue

            if parsed.fragment and target.suffix == ".html":
                target_parser = pages.get(target.resolve())
                if target_parser is not None and unquote(parsed.fragment) not in target_parser.ids:
                    errors.append(f"{page.relative_to(BOOK_DIR)}: missing anchor {href}")
    return errors


def check_llm_links() -> list[str]:
    errors: list[str] = []
    page = BOOK_DIR / "index.md"
    text = page.read_text(encoding="utf-8")
    anchors = {
        slugify(line.lstrip("#").strip())
        for line in text.splitlines()
        if line.startswith("#")
    }

    for match in MARKDOWN_LINK_RE.finditer(text):
        target = match.group(3)
        parsed = urlsplit(target)
        if parsed.scheme or parsed.netloc:
            continue
        if parsed.path:
            path = (page.parent / unquote(parsed.path)).resolve()
            if not path.exists():
                errors.append(f"book/index.md: missing {target}")
        if parsed.fragment and unquote(parsed.fragment) not in anchors:
            errors.append(f"book/index.md: missing anchor {target}")
    return errors


def main() -> None:
    errors = check_source_links() + check_html_links() + check_llm_links()
    if errors:
        for error in errors:
            print(error, file=sys.stderr)
        raise SystemExit(f"Found {len(errors)} broken documentation link(s)")
    print("All documentation links are valid")


if __name__ == "__main__":
    main()
