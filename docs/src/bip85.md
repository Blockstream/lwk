# BIP85

The [BIP85 specification](https://github.com/bitcoin/bips/blob/master/bip-0085.mediawiki) allows you to deterministically derive new BIP39 mnemonics from an existing mnemonic. This is useful for creating separate wallets or accounts while maintaining a single backup.

## Deriving a Mnemonic

To derive a BIP85 mnemonic, you need a software signer initialized with a mnemonic (hardware wallet-based signers are not supported at the moment). The derived mnemonic is obtained with `Signer::derive_bip85_mnemonic()`, which takes an `index` (0-based) and a `word_count` (12 or 24).

<custom-tabs category="lang">
<div slot="title">Rust</div>
<section>

```rust,ignore
{{#include ../../lwk_signer/src/software.rs:test_bip85_derivation}}
```
</section>

<div slot="title">Python</div>
<section>

```python
{{#include ../../lwk_bindings/tests/bindings/bip85.py:bip85}}
```
</section>

<div slot="title">Javascript</div>
<section>

```typescript
{{#include ../../lwk_wasm/tests/node/bip85.js:bip85}}
```
</section>
</custom-tabs>

----

Next: [CLI](cli.md)

