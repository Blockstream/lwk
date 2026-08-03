# Changelog

## Unreleased

* Add Silent Payments (BIP-352 on Liquid, per the Liquid silent payments ELIP) behind the new opt-in `silentpayments` feature, in module `lwk_wollet::silentpayments`:
  * Send: `TxBuilder::add_silent_payment_recipient()` plus `TxBuilder::finish_silent_payment()`, which needs the funding inputs' private keys (via `SilentPaymentInputProvider`) because the output is derived from the transaction's own inputs. Calling plain `TxBuilder::finish()` with a pending silent payment recipient fails with `Error::SilentPaymentRequiresKeys` rather than dropping the recipient.
  * Receive: **scan-only** material (`SilentPaymentScanMaterial`, i.e. `b_scan` plus the *public* `B_spend`) is configured once with `WolletBuilder::with_silent_payment_material()`, then `Wollet::silent_payment_address()` and `Wollet::labeled_silent_payment_address()` produce reusable addresses. Obtain it from a signer via `lwk_common::silentpayments::SilentPaymentSigner::silent_payment_scan_material()`, implemented by `lwk_signer::SwSigner`, which derives both ELIP branches (`m/352'/{1776'|1'}/account'/{1'|0'}/0`) and exports only the scan half.
  * Trust boundary: a `Wollet` can detect, unblind, track and select silent payments, but holds no `b_spend` and so **cannot sign** for them. Scan results carry a `SpendTweak` (the scalar `t_k`, or `t_k + label_tweak_m`), never a completed key; correctness is checked publicly as `B_spend + spend_tweak*G == output spend pubkey`. Spending requires `lwk_signer::SwSigner::sign()`, which re-derives `b_spend` internally, verifies the wallet's tweak against the output being spent, and only then signs. Hardware signers are explicitly unsupported (`SignerError::UnsupportedSilentPayments`) until their protocols expose the necessary operation.
  * Discovery: `BlockchainBackend::scan_silent_payments()` finds them through a light client. Backends advertise `Capability::SilentPayments` and answer `BlockchainBackend::silent_payment_tweaks()` with each block's `T = input_hash·A`; the wallet derives the candidate scripts those tweaks would produce for its keys and confirms them with the ordinary script-history query, so only transactions that actually pay it are downloaded. Implemented for `EsploraClient`, which computes tweaks from blocks with no server change. A backend without the capability returns `Error::SilentPaymentsUnsupportedByBackend` rather than an empty result.
  * **Fixed**: silent-payment signatures now commit to the PSET's genesis hash (ELIP-101).
  * **Fixed**: a crafted PSET naming a silent-payment account with bit 31 set (e.g. `u32::MAX`) panicked the signer instead of erroring, since those values have no hardened BIP-32 form. `SilentPaymentAccount::from_raw` is now checked and returns `SilentPaymentAccountError`; the new `SilentPaymentPsetMetaError::Account` surfaces it at the decode boundary.
  * Silent-payment discovery and spending are covered end to end on regtest, including network acceptance of the finalized spend.
  * Spend, standalone: outputs from `scan_silent_payments()` that were never applied to the wallet go through `TxBuilder::add_silent_payment_utxos()`, which registers the funding view *and* keeps the spend tweak. `SilentPaymentUtxo::external_utxo()` alone funds the input but cannot carry the tweak, so it produces an input no signer can complete — its docs now say so and point here.
  * Spend: coin selection picks silent payment outputs like any other, and `TxBuilder` annotates each such PSET input with `lwk_common::silentpayments::SilentPaymentInputMeta` (account + spend tweak + expected `B_spend`; never a key). This is what stands in for the `bip32_derivation` an SP input cannot have: the ELIP path `m/352'/1776'/account'/0'/0` reaches `b_spend`, but the key that signs is `b_spend + t_k`, tweaked by the paying transaction and so off any BIP-32 path. `lwk_signer::SwSigner::sign()` verifies that metadata against the prevout and signs; a cache entry whose tweak does not verify against the wallet's own `B_spend` is refused at build time rather than shipped as a PSET that could only fail later.
  * Found outputs are ordinary wallet money: after `Wollet::apply_silent_payments()` they appear in `Wollet::balance()`, `Wollet::utxos()` and `Wollet::txos()`, and coin selection spends them with no silent-payment-specific handling. Only the spend *tweak* is cached, never a spending key. They can still be spent standalone via `SilentPaymentUtxo::external_utxo()`.
  * Discovery runs as part of `full_scan()`: a wallet built with `WolletBuilder::with_silent_payment_material()` finds silent payments during an ordinary scan, and after `apply_update()` they are in `balance()`, `utxos()` and `txos()` with no silent-payment-specific call. Findings travel in the `Update` (wire version 6) so they are persisted and replayed on restore like everything else a scan finds; older update versions decode unchanged. How far discovery has run is tracked separately from the wallet tip, since a wallet that scanned before its scan material was configured must not treat that history as already searched. `Wollet::status()` incorporates both `B_scan` and `B_spend` (never a secret), so changing signer or account invalidates an incompatible persisted cache.
  * `Wollet::scan_silent_payments()` remains available for callers holding transactions from a source of their own.
* Add Waterfalls descriptor subscriptions, returning `tip`, `mempool`, `block`, and `reorg` events that callers can use as wallet rescan hints.
* Add the `electrum_oidc` feature: `TokenProvider::Blockstream` support for `ElectrumClient` (automatic OAuth2 token fetch, plus invalidate and retry once when the server denies a call with an authentication error). Not available on wasm.
* `Wollet::assets_owned()` returns all assets ever owned instead of only unspent ones.
* `Contract` no longer contains public fields. To build a `Contract` for issuance, use `Contract::builder`.

## 0.18.0

* Remove `lwk_wollet::ElementsNetwork`, replaced with `lwk_common::Network`. Notes for migration:
  * `lwk_wollet::ElementsNetwork::genesis_block_hash()` replaced by `lwk_common::Network::genesis_hash()`;
  * `lwk_wollet::ElementsNetwork::policy_asset()` returned `AssetId`, `lwk_common::Network::policy_asset()` returns `&AssetId`;
  * `lwk_wollet::ElementsNetwork::LiquidTestnet` replaced by `lwk_common::Network::TestnetLiquid`;
  * `lwk_wollet::ElementsNetwork::ElementsRegtest {policy_asset: AssetId}` replaced by `lwk_common::Network::CustomElements(ElementsParams)`, where `ElementsParams` contains custom `policy_asset` and `genesis_hash`, and should be constructed with builder;
* Remove `lwk_common::PsetBalance::fee`, replaced with `lwk_common::PsetBalance::fees` as `HashMap<AssetId, u64>` and method `lwk_common::PsetBalance::fees_in(asset_id)` ([ELIP-0204](https://github.com/ElementsProject/ELIPs/blob/main/elip-0204.mediawiki))

## 0.17.0

* Add `WolletBuilder::utxo_only()`
* `full_scan` now fails if the `Client` is "utxo only" and the `Wollet` is not "utxo only" (or viceversa)
* Deprecated `Wollet::apply_update_no_persist()` and `Wollet::apply_transaction_no_persist()`
* Deprecated `WolletBuilder::with_store()`, use `WolletBuilder::with_stores()` or `WolletBuilder::with_updates_store()`
* change `lwk_common::Network` regtest option from `LocaltestLiquid` to `CustomElements(ElementsParams)`, where `ElementsParams` contains custom `policy_asset` and `genesis_hash`, and should be constructed with builder

## 0.16.0

There have been changes in how the wollet handle persistence.

* If you're a lwk_cli user, a lwk_wasm/lwk_node user, a lwk_bindings user using lwk_bindings::Wollet::new there are no change.

* if you're a lwk_bindings user using a custom persister:
  * removed `ForeignPersister` (trait), replacement `ForeignStore`
  * removed `ForeignPersisterLink` (concrete "trait"), replacement `ForeignStoreLink`
  * removed `Wollet::with_custom_persister()`, added `Wollet::with_custom_store()`

* if you're a lwk_wollet user:
  * removed `PersistError`
  * removed `Persister` (trait), replacement `lwk_common::Store` and `lwk_common::DynStore`
  * removed `NoPersist`, replacement `lwk_common::FakeStore`
  * removed `FsPersister`, replacement `lwk_common::FileStore` and `lwk_common::EncryptedStore`
  * removed `WolletBuilder::with_persister()`, replacement `WolletBuilder::with_store()`
  * changed `Wollet::new()`: 2nd argument is a `DynStore` instead of a "`Persister`"


* Removed `Wollet::as_ref()`, replaced with `Wollet::ct_descriptor()`
* The following methods now return a `Result`:
  * `Wollet::descriptor()`
  * `WolletDescriptor::descriptor()`
  * `WolletDescriptor::url_encoded_descriptor()`
  * `WolletDescriptor::bitcoin_descriptor_without_key_origin()`
  * `WolletDescriptor::single_bitcoin_descriptors()`
* `Wollet::transaction()`, `Wollet::transactions()`, `Wollet::transactions_paginated()` populate inputs and outputs with explicit wallet inputs and outputs
* `Wollet::transactions()`, `Wollet::transactions_paginated()` also return transactions with explicit wallet inputs or outputs
* `Wollet::txos()` returns also explicit wallet txos
