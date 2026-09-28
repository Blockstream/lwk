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

## Payment instructions

Fuzz user-provided payment instructions, including addresses, BIP21/BIP321
URIs, Lightning payments, LNURL, and BIP353 identifiers:

```sh
cargo fuzz run payment_instruction --sanitizer none
```

The target is entirely offline: it parses LNURL and BIP353 identifiers but does
not resolve them over HTTP or DNS. It limits fuzzer inputs to 4,096 bytes. For a
bounded run:

```sh
cargo fuzz run payment_instruction --sanitizer none -- -max_total_time=60 -max_len=4096
```

## Wallet descriptors

Fuzz strict and relaxed wallet descriptor parsing, including confidential
descriptors, Green-style two-line descriptors, and fixed script pubkey lists:

```sh
cargo fuzz run descriptor --sanitizer none
```

The target checks canonical parse/display round trips and that every accepted
descriptor derives scripts and addresses at small indices. Inputs are limited
to 4,096 bytes. For a bounded run:

```sh
cargo fuzz run descriptor --sanitizer none -- -max_total_time=60 -max_len=4096
```

Descriptors are grammar-like, so random byte mutations rarely produce valid
fragments. `fuzz/descriptor.dict` lists script types, keys and derivation
steps for libFuzzer to splice in:

```sh
cargo fuzz run descriptor --sanitizer none -- -dict=fuzz/descriptor.dict
```

## Seeding the corpus

Each target starts from `fuzz/corpus/<target>/`, one raw input per file. To
steer a target towards inputs it is unlikely to build on its own, add
handwritten seeds named by their SHA-1, as libFuzzer does. The corpus
directory is ignored by git, so add seeds explicitly:

```sh
printf '%s' '<input>' > seed
h=$(sha1sum seed | cut -d' ' -f1)
mv seed fuzz/corpus/descriptor/$h
git add -f fuzz/corpus/descriptor/$h
```
