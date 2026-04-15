# High-Volume Wallets

When the number of transactions grows significantly,
handling a wallet can become challenging.
To make it easier, LWK provides various utilities to handle high-volume wallets.
In this section we provide an overview.
You can use one, or combinations of them.

The approaches in this guide are complex or experimental.
Before trying them,
our first and most obvious suggestion is to increase the computing resources on your machine.
This might be the simplest approach, with little to none engineering overhead.
If this is not possible, or not enough consider applying one or more of the suggestions below.

## Transaction Batching
To reduce the number of transaction consider using a single transaction for multiple "send" operation (e.g. calling `add_recipient` multiple times).
In this way you can do a "batch send" with a single transaction.

## Rotate Wallets
Once a `Wollet` has too many transactions it might become impractical to use it,
it can become too slow or have unsustainable resource requirements.

A simple approach to avoid this is to rotate the wallet.
Once it reaches a certain number of transactions,
you stop using it and you start to use another.

Note that you dont need to generate another BIP39 mnemonic/seed.
You can use the same secret,
and use the next BIP32 account, just by bumping the index.
