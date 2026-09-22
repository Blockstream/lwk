"""Shared helpers for resolving links in the LWK documentation."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
import re
from urllib.parse import unquote, urlsplit


MARKDOWN_LINK_RE = re.compile(r"(!?)\[([^\]]*)\]\(([^)]+)\)")


@dataclass(frozen=True)
class RepositoryLink:
    """A local repository target and its corresponding hosted URL."""

    path: Path
    url: str


def slugify(text: str) -> str:
    """Convert a heading to the anchor format used by the generated LLM docs."""
    text = text.strip().lower()
    text = re.sub(r"<[^>]+>", "", text)
    text = re.sub(r"[^\w\s-]", "", text)
    text = re.sub(r"[\s_]+", "-", text)
    return text.strip("-")


def local_target(current_page: Path, target: str) -> Path | None:
    """Resolve a local Markdown target, ignoring its query and fragment."""
    parsed = urlsplit(target)
    if parsed.scheme or parsed.netloc or not parsed.path:
        return None
    return (current_page.parent / unquote(parsed.path)).resolve()


def repository_link(
    current_page: Path,
    target: str,
    repo_dir: Path,
    source_dir: Path,
    source_url: str,
    branch: str,
) -> RepositoryLink | None:
    """Return a hosted URL for a link leaving the mdBook source directory."""
    path = local_target(current_page, target)
    if path is None or not path.exists():
        return None

    repo_dir = repo_dir.resolve()
    source_dir = source_dir.resolve()
    if not path.is_relative_to(repo_dir) or path.is_relative_to(source_dir):
        return None

    parsed = urlsplit(target)
    relative = path.relative_to(repo_dir).as_posix()
    kind = "tree" if path.is_dir() else "blob"
    url = f"{source_url.rstrip('/')}/{kind}/{branch}/{relative}"
    if parsed.query:
        url += f"?{parsed.query}"
    if parsed.fragment:
        url += f"#{parsed.fragment}"
    return RepositoryLink(path, url)
