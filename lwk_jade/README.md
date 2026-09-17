# LWK Jade

Library to interact with the [Jade](https://blockstream.com/jade/) hardware wallet.

It's available in blocking ([`Jade`]) and asynchronous ([`asyncr::Jade`]) variants.

Some of the implemented features include, unlocking the Jade, registering multisig wallets, signing PSETs.

[jade docs](https://github.com/Blockstream/Jade/blob/master/docs/index.rst)

## Tests
Most test run with a Jade emulator
```sh
cargo test
```

It's possible to test with a Jade connected over serial, checking what is displayed on screen
```sh
cargo test --features serial -- --include-ignored
```

Coverage with a regtest environment is in `lwk_wollet`.
