# LWK docs

## mdBook

Serve the docs locally:
```
cd docs
cargo install mdbook
cargo install mdbook-mermaid
cargo install --path ./snippets/processor
mdbook build
mdbook serve
```
