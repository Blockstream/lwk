from pathlib import Path
from tempfile import TemporaryDirectory
import unittest

import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from repo_links import rewrite_content


class RewriteContentTests(unittest.TestCase):
    def setUp(self):
        self.temp_dir = TemporaryDirectory()
        self.repo = Path(self.temp_dir.name)
        self.source = self.repo / "docs" / "src"
        self.source.mkdir(parents=True)
        self.page = self.source / "page.md"
        self.page.touch()
        (self.repo / "crate").mkdir()
        (self.repo / "crate" / "example.rs").touch()

    def tearDown(self):
        self.temp_dir.cleanup()

    def rewrite(self, content: str) -> str:
        return rewrite_content(
            content,
            self.page,
            self.repo,
            self.source,
            "https://github.com/example/project",
            "main",
        )

    def test_rewrites_repository_file(self):
        actual = self.rewrite("[example](../../crate/example.rs#demo)")
        self.assertEqual(
            actual,
            "[example](https://github.com/example/project/blob/main/crate/example.rs#demo)",
        )

    def test_rewrites_repository_directory(self):
        actual = self.rewrite("[crate](../../crate)")
        self.assertEqual(
            actual,
            "[crate](https://github.com/example/project/tree/main/crate)",
        )

    def test_preserves_book_and_external_links(self):
        (self.source / "other.md").touch()
        content = "[chapter](other.md) [site](https://example.com)"
        self.assertEqual(self.rewrite(content), content)

    def test_preserves_images(self):
        content = "![example](../../crate/example.rs)"
        self.assertEqual(self.rewrite(content), content)


if __name__ == "__main__":
    unittest.main()
