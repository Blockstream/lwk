# Simplicity

Library to interact with Simplicity language using LWK.

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


