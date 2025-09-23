# Transaction Broadcast
When a PSET has enough signatures, it's ready to be broadcasted to the Liquid Network.

## Finalize the PSET
First you need to call `Wollet::finalize()` to finalize the PSET and extract the signed transaction.

## Broadcast the Transaction
The transaction returned by the previous step can be sent to the Liquid Network with `EsploraClient::broadcast()`.
