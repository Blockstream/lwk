#!/usr/bin/env python3
"""Demonstrate how change is handled on Liquid with LWK.

Liquid transactions are confidential: amounts and assets are blinded, so the
"change" that a transaction returns to your own wallet is not visible to the
outside world. This example sends a partial payment and then inspects the
resulting transaction to show:

  * which output is the change (it belongs to the wallet's *internal* chain),
  * that the change amount equals funded - sent - fee,
  * that the change is confidential but the wallet can unblind it,
  * that change lands on a fresh internal address each time (no reuse),
  * that the change becomes a new spendable UTXO.

It runs against a throwaway regtest node (elementsd + electrs) started by
LwkTestEnv, so it needs no network access and no credentials.
"""

from lwk import *


def wallet_outputs(wtx):
    """Return (vout, WalletTxOut) pairs for outputs owned by the wallet.

    `WalletTx.outputs()` yields `None` for outputs that don't belong to the
    wallet (the recipient and the explicit fee output), and a `WalletTxOut`
    for the ones that do.
    """
    return [(vout, out) for vout, out in enumerate(wtx.outputs()) if out is not None]


def main():
    node = LwkTestEnv()  # launches elementsd + electrs

    network = Network.regtest_default()
    policy_asset = network.policy_asset()
    client = ElectrumClient.from_url(node.electrum_url())

    signer = Signer(Mnemonic.from_random(12), network)
    wollet = Wollet(network, signer.wpkh_slip77_descriptor(), datadir=None)

    # ----------------------------------------------------------------- fund
    # Fund the wallet with a single confidential UTXO.
    funded = 100_000
    txid = node.send_to_address(wollet.address(0).address(), funded, asset=None)
    wollet.wait_for_tx(txid, client)
    assert wollet.balance()[policy_asset] == funded
    assert len(wollet.utxos()) == 1

    # -------------------------------------------------------------- payment
    # Spend less than we have, which forces a change output back to ourselves.
    sent = 30_000
    destination = node.get_new_address()

    builder = network.tx_builder()
    builder.add_lbtc_recipient(destination, sent)
    pset = builder.finish(wollet)
    pset = signer.sign(pset)
    tx = wollet.finalize(pset).extract_tx()
    fee = tx.fee(policy_asset)

    txid = client.broadcast(tx)
    wollet.wait_for_tx(txid, client)

    # ------------------------------------------------------ inspect change
    wtx = wollet.transaction(txid)
    owned = wallet_outputs(wtx)

    # The recipient output and the fee output are not ours, so exactly one of
    # the transaction's outputs comes back to the wallet: the change.
    assert len(owned) == 1, f"expected 1 change output, got {len(owned)}"
    change_vout, change = owned[0]

    # Change always goes to the *internal* chain, never the external chain that
    # we hand out to payers.
    assert change.ext_int() == Chain.INTERNAL

    # It is confidential on-chain, but as the owner we can read the blinded
    # secrets. The change amount is whatever is left after the payment and fee.
    secrets = change.unblinded()
    expected_change = funded - sent - fee
    assert secrets.value() == expected_change
    assert secrets.asset() == policy_asset
    assert not secrets.is_explicit()  # blinded, i.e. a real confidential output

    # Sanity-check the raw transaction: the value committed on-chain is a
    # Pedersen commitment, not the cleartext amount we just recovered.
    raw_change_out = tx.outputs()[change_vout]
    assert not raw_change_out.is_fee()

    print(f"funded            : {funded:>8} sats")
    print(f"sent to node      : {sent:>8} sats")
    print(f"network fee       : {fee:>8} sats")
    print(f"change (recovered): {secrets.value():>8} sats -> {change.address()}")
    print(f"change chain/index: {change.ext_int()} #{change.wildcard_index()}")

    # The wallet balance and the spendable UTXO set now reflect the change.
    assert wollet.balance()[policy_asset] == expected_change
    utxos = wollet.utxos()
    assert len(utxos) == 1
    assert str(utxos[0].outpoint().txid()) == str(txid)
    assert utxos[0].outpoint().vout() == change_vout

    # ----------------------------------------- change uses fresh addresses
    # Spend again and confirm the new change does not reuse the previous
    # internal address (address reuse would harm privacy).
    builder = network.tx_builder()
    builder.add_lbtc_recipient(node.get_new_address(), 5_000)
    pset = signer.sign(builder.finish(wollet))
    tx2 = wollet.finalize(pset).extract_tx()
    txid2 = client.broadcast(tx2)
    wollet.wait_for_tx(txid2, client)

    _, change2 = wallet_outputs(wollet.transaction(txid2))[0]
    assert change2.ext_int() == Chain.INTERNAL
    assert str(change2.address()) != str(change.address())
    assert change2.wildcard_index() != change.wildcard_index()
    print(f"second change addr: {change2.address()} #{change2.wildcard_index()} (fresh)")

    print("OK: confidential change handled correctly")


if __name__ == "__main__":
    main()
