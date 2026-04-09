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

## Publish

To publish the book at https://blockstream.github.io/lwk/book/

```
git checkout gh-pages
git reset --hard HEAD~10
git rebase master
just mdbook
git add -f docs/book
git commit -m "docs: add book"
git push
```
