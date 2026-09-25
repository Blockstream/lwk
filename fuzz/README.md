# Fuzzing

Run commands from the repository root. The Nix development shell provides
`cargo-fuzz`. In other environments, install it first, for example with
`cargo install cargo-fuzz`.

Because the repository pins stable Rust, disable sanitizers:

```sh
cargo fuzz run update_deserialize --sanitizer none
```

This target mostly exercises safe Rust, so sanitizer-free fuzzing still catches
the relevant panics, aborts, and hangs. Occasional nightly runs with the default
AddressSanitizer remain useful for detecting memory errors in unsafe code and
dependencies.

To run for a fixed amount of time, for example 60 seconds:

```sh
cargo fuzz run update_deserialize --sanitizer none -- -max_total_time=60
```

To replay a saved crash artifact:

```sh
cargo fuzz run update_deserialize --sanitizer none fuzz/artifacts/update_deserialize/<artifact>
```

## Jade response framing

Fuzz raw, fragmented, and concatenated CBOR responses from a Jade device:

```sh
cargo fuzz run jade_response --sanitizer none
```

The target limits inputs to Jade's 4,096-byte response buffer. A bounded run can
also set libFuzzer's maximum generated input length explicitly:

```sh
cargo fuzz run jade_response --sanitizer none -- -max_total_time=60 -max_len=4096
```
