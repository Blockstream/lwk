# Transaction Signing
Once you have created a PSET now you need to add some signatures to it.
This is done by the `Signer`,
however the signer might be isolated,
so we need some mechanisms to allow the signer to understand what is signing.

## Get the PSET details
This is done with `Wollet::get_details()`, which returns:
* missing signatures and the respective signers' fingerprints
* net balance, the effect that transaction has on wallet (e.g. how much funds are sent out of the wallet)

If the `Signer` fingeprint is included in the missing signatures,
then a `Signer` with that fingeprint expected to sign.

The balance can be shown to the user or validated against the `Signer` expectations.

It's worth noticing that `Wollet`s can work without internet,
so offline `Signer`s can have `Wollet`s instance to enhance the validation performed before signing.
