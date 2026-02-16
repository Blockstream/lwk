# Transaction Signing
Once you have created a PSET now you need to add some signatures to it.
This is done by the `Signer`,
however the signer might be isolated,
so we need some mechanisms to allow the signer to understand what is signing.

## Get the PSET details
This is done with `Wollet::get_details()`, which returns:
* missing signatures and the respective signers' fingerprints
* net balance, the effect that transaction has on wallet (e.g. how much funds are sent out of the wallet)

If the `Signer` fingerprint is included in the missing signatures,
then a `Signer` with that fingerprint expected to sign.

The balance can be shown to the user or validated against the `Signer` expectations.

It's worth noticing that `Wollet`s can work without internet,
so offline `Signer`s can have `Wollet`s instance to enhance the validation performed before signing.

<custom-tabs category="lang">
<div slot="title">Rust</div>
<section>

```rust,ignore
{{#include ../../lwk_wollet/tests/e2e.rs:pset-details}}
```
</section>

<div slot="title">Python</div>
<section>

```python
{{#include ../../lwk_bindings/tests/bindings/basics.py:pset-details:ignore}}
```
</section>

<div slot="title">Javascript</div>
<section>

```typescript
{{#include ../../lwk_wasm/tests/node/basics.js:pset-details:ignore}}
```
</section>
</custom-tabs>

## Sign the PSET
Once you have performed enough validation, you can call `Signer::sign`.
Which adds signatures from `Signer` to the PSET.

Once the PSET has enough signatures, you can broadcast to the Liquid Network.

<custom-tabs category="lang">
<div slot="title">Rust</div>
<section>

```rust,ignore
{{#include ../../lwk_wollet/tests/e2e.rs:sign}}
```
</section>

<div slot="title">Python</div>
<section>

```python
{{#include ../../lwk_bindings/tests/bindings/basics.py:sign:ignore}}
```
</section>

<div slot="title">Javascript</div>
<section>

```typescript
{{#include ../../lwk_wasm/tests/node/basics.js:sign:ignore}}
```
</section>
</custom-tabs>

----

Next: [Broadcast a Transaction](broadcast.md)
