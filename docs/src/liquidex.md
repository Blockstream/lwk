# LiquiDEX

LiquiDEX is a 2-step atomic swap protocol for the Liquid Network that enables trustless peer-to-peer asset exchanges. It allows users to swap Liquid Bitcoin (LBTC) and other Liquid assets without requiring a trusted third party or centralized exchange.

## Overview

A LiquiDEX swap involves two parties:
- **Maker**: Creates a swap proposal offering to exchange one asset for another
- **Taker**: Accepts the proposal and completes the swap

The protocol uses an incomplete but signed transaction (unbalanced and without fees) created by the maker. The taker completes the transaction by adding inputs and outputs, balancing the amounts and adding fees. This ensures atomicity: either both parties get what they want, or the transaction cannot be broadcast.

The proposal always spend a full utxo, due there is no a way to add change address for the **maker**.

## How It Works

1. **Maker creates a proposal**: The maker creates a PSET with one input (UTXO to be spent) and one output (the asset he want to receive). The transaction is signed but incomplete. The PSET is comverted in a Liquid Proposal, a structure cointaining all the relevant information for the swap.

2. **Proposal validation**: The taker receives the proposal and validates it using primitives available in LWK.

3. **Taker completes the swap**: The taker recerate the PSET and adds inputs and outputs to balance the transaction, adds fees, and signs their part.

4. **Transaction broadcast**: The completed transaction is broadcast to the Liquid Network, executing the atomic swap.

## Key Concepts

### LiquidexProposal

A `LiquidexProposal` represents a swap offer. It comes in two states:
- **Unvalidated**: The proposal has been created but not yet verified
- **Validated**: The proposal has been verified and is ready to be taken

## Creating a Swap Proposal (Maker)

The maker creates a swap proposal by building a transaction with `liquidex_make()`, signing it, and converting it to a proposal:

<custom-tabs category="lang">
<div slot="title">Rust</div>
<section>

```rust,ignore
{{#include ../../lwk_wollet/tests/e2e.rs:liquidex_make}}
```

</section>

<div slot="title">Python</div>
<section>

```python
{{#include ../../lwk_bindings/tests/bindings/liquidex.py:liquidex_make}}
```

</section>
</custom-tabs>

## Validating a Proposal (Taker)

Before accepting a proposal, the taker must validate it by fetching the previous transaction and verifying the proposal:

<custom-tabs category="lang">
<div slot="title">Rust</div>
<section>

```rust,ignore
{{#include ../../lwk_wollet/tests/e2e.rs:liquidex_validate}}
```

</section>

<div slot="title">Python</div>
<section>

```python
{{#include ../../lwk_bindings/tests/bindings/liquidex.py:liquidex_validate}}
```

</section>
</custom-tabs>

## Taking a Proposal (Taker)

Once validated, the taker can accept the proposal by using `liquidex_take()` to complete the transaction:

<custom-tabs category="lang">
<div slot="title">Rust</div>
<section>

```rust,ignore
{{#include ../../lwk_wollet/tests/e2e.rs:liquidex_take}}
{{#include ../../lwk_wollet/tests/e2e.rs:liquidex_take_2}}
```

</section>

<div slot="title">Python</div>
<section>

```python
{{#include ../../lwk_bindings/tests/bindings/liquidex.py:liquidex_take}}
```

</section>
</custom-tabs>

## Additional Resources

- [LiquiDEX Blog Post](https://blog.blockstream.com/liquidex-2-step-atomic-swaps-on-the-liquid-network/)

---

Previous: [Ledger](ledger.md)