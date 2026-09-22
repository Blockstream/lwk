# LWK docs

## Style
For new additions or improvement, follow our [guidelines](GUIDE.md).

## mdBook

From the repository root, build both the HTML and LLM-friendly documentation
and check all local links with:

```
just docs-check
```

Serve the checked book locally with:

```
just mdbook-serve
```

These commands require mdBook and its preprocessors. They are available in the
optional Nix development environment, which can run the checks with
`direnv exec . just docs-check`. Alternatively, install them directly:

```
cd docs
cargo install mdbook --version 0.4.52 --locked --force
cargo install mdbook-mermaid --version 0.16.2 --locked --force
cargo install --path ./snippets/processor
cd ..
just docs-check
```

Repository-relative links in `docs/src` should point to their real local
targets, for example `../../lwk_wollet`. The `repo-links` preprocessor rewrites
such links to the configured source repository when rendering the HTML book.

## LLM-friendly docs

Generate the `llms.txt` entrypoint and the merged Markdown page:
```
just llms
```

The generated files are published at:
* https://blockstream.github.io/lwk/book/llms.txt
* https://blockstream.github.io/lwk/book/index.md

`just mdbook` also runs this step after building the HTML book.

## Publish

To publish the book at https://blockstream.github.io/lwk/book/

The site root at https://blockstream.github.io/lwk/ is handled by `docs/index.html`, which redirects to the book.

```
git fetch github gh-pages master
git switch gh-pages
git reset --hard github/master
just docs-check
git add -f docs/book
git commit -m "docs: add book"
git push --force-with-lease github gh-pages
```
