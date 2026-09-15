# Simplicity

Library to interact with Simplicity language using LWK.

> [!WARNING]
> This crate and all `simplicity`-gated functionality across the entire LWK workspace is NOT production ready.
> It is intended only for tinkering and experimentation with the Simplicity language.
> Do not use in production or with real funds.

## Lending

### Run the integration tests of lending flow

Install `simplicity-lending-indexer` binary:

```shell
cargo install --git https://github.com/BlockstreamResearch/simplicity-lending.git lending-indexer
```

Set `LENDING_INDEXER_EXEC` environment variable to installation path of `simplicity-lending-indexer` binary

```shell
export LENDING_INDEXER_EXEC="path/to/simplicity-lending-indexer"
```

Run tests of `lwk_simplicity` with `lending` feature:

```shell
cargo test -p lwk_simplicity --features lending
```


