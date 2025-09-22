# Signers
In LWK, the management of private keys is delegated to a specialized component called **Signer**.

The primary tasks of a signer are:
* provide `xpub`s, which are used to create wallets
* sign transactions

## Types of Signers
LWK has two signer types:
* **Software Signers**: store the private keys in memory. This is the simplest signer to integrate and interact with.
* **Hardware Signers**: LWK provides specific integrations for hardware wallets, such as the Blockstream Jade. These signers keep the private keys completely isolated from the computer.

While hardware signers are inherently more secure, LWK's design allows you to enhance the security of software signers as well. For example, a software signer can be run on an isolated machine or a mobile app might store the mnemonic (seed) encrypted, only decrypting it when a signature is required.

This guide will focus on software signers. For more details on hardware signers, please see the [Jade documentation](jade.md).

## Create Signer
To create a signer you need a mnemonic.
You can generate a new one with `bip39::Mnemonic::generate()`.
Then you can create a software signer with `SwSigner::new()`.

## Get Xpub
Once you have a signer you need to get some an extended public key (`xpub`),
which can be used to create a wallet that requires signature from the signer.

The xpub is obtained with `Signer::keyorigin_xpub()`, which also includes the keyorigin information: signer fingerprint and derivation path from master key to the returned xpub, e.g. `[ffffffff/84h/1h/0h]xpub...`.

For particularly simple cases, such as single sig, you can get the CT descriptor directly from the signer, for instance using `Signer::wpkh_slip77_descriptor()`.

----

Next: [Watch-Only Wallets](wollet.md)
