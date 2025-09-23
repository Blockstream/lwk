# Transaction Creation
With a `Wollet` you can generate an address,
which can be used to receive some funds.
You can fetch the transactions receiving the funds using a "client",
and apply them to the wallet.
Now that the `Wollet` has a balance, it is able to craft transactions sending funds to desired destination.

The first step is construction a `TxBuilder` (or `WolletTxBuilder`), using `TxBuilder::new()` or `Wollet::tx_builder()`.
You can now specify how to build the transaction using the methods exposed by the `TxBuilder`.
