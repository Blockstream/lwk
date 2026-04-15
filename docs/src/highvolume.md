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
