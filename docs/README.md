# LWK docs

## Style
For new additions or improvement, follow our [guidelines](GUIDE.md).

## mdBook

Serve the docs locally:
```
cd docs
cargo install mdbook --version 0.4.52 --locked --force
cargo install mdbook-mermaid --version 0.16.2 --locked --force
cargo install --path ./snippets/processor
mdbook build
mdbook serve
```

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
git checkout gh-pages
git reset --hard HEAD~10
git rebase master
just mdbook
git add -f docs/book
git commit -m "docs: add book"
git push --force-with-lease
```
