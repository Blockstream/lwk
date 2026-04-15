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

## Update Pruning
The largest component in memory and disk usage are Liquid transactions.
They are huge, and their largest part are rangeproofs.
Those are used when unblinding transactions,
but later they're not used anymore (unless in extermely particular cases).

You can remove them calling `Update::prune()` before applying and persiting the update.

## Merge Updates
Every time you get new transactions,
the `Wollet` fetches a new `Update` which is applied and (optionally) persisted.
These updates are sequential, the new one applies on top of the previous.
When the `Wollet` restarts,
it reads the `Update`s from where they were perstised and applies them to reconstruct the last `Wollet` state.

These `Update`s can become quite a lot and it could be useful to compact them.

One way it's to sync specifying another directory,
all transactions will be downloaded again,
and you will have a single compacted update.

However for large wallets, this might not be ideal.
For them we have `Wollet::with_merge_thresold()`.
It allows to specify a threshold after which all updates are compacted into one.

## Waterfalls
If your wallet consists in a large number of scriptpubkeys,
using `Esplora` or `Electrum` will require a large number of network roundtrips to perform a full scan of the wallet.
If this is an issue for your setup,
consider using `Waterfalls` to fetch blockchain data.
`Waterfalls` is an optimized scriptpubkey/address data indexer that reduces server load, client load, network roundtrip.
Switching to it makes full scan faster.

This improvement comes with a trade off, the client shares with the Waterfalls server its descriptor (without the descriptor blinding key) revealing all the descriptor scriptpubkeys.
We think this trade off is reasonable,
moreover if you're using a self-hosted Waterfalls server, you have no downside.
