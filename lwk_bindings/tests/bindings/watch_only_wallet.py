#!/usr/bin/env python3
"""Watch-only wallet workflow with LWK (online watcher + offline signer).

In LWK a `Wollet` is *always* watch-only: it is built from a Confidential
Transaction (CT) descriptor and never holds any private key. Spending is a
separate step performed by a `Signer`. That separation is exactly what you want
for an air-gapped / cold-storage setup:

    offline (cold)                         online (hot, untrusted)
    ------------------                     ------------------------
    Signer (holds the seed)                Wollet (descriptor only)
        |  export public descriptor  ->        derive addresses
        |                                       scan the chain
        |                                       build an unsigned PSET
        |  <- transfer unsigned PSET ----  --
      sign the PSET
        -- transfer signed PSET ->  ------>    finalize + broadcast

This example plays both roles in one process to show the data that crosses the
boundary. The online wallet is reconstructed purely from the descriptor *string*
to make the point that no key material is ever needed to watch or to build a
transaction.

Runs against a throwaway regtest node (elementsd + electrs) started by
LwkTestEnv; no network access or credentials required.
"""

from lwk import *


def main():
    node = LwkTestEnv()  # launches elementsd + electrs

    network = Network.regtest_default()
    policy_asset = network.policy_asset()
    client = ElectrumClient.from_url(node.electrum_url())

    # ----------------------------------------------------- offline: the key
    # The signer lives offline. The only thing it ever exports is the *public*
    # CT descriptor; the mnemonic/seed never leaves this side.
    signer = Signer(Mnemonic.from_random(12), network)
    exported_descriptor = str(signer.wpkh_slip77_descriptor())

    # ------------------------------------------------- online: the watcher
    # The online wallet is built from the descriptor string alone. It has no
    # signer and no seed: it can only watch and prepare transactions.
    descriptor = WolletDescriptor(exported_descriptor)
    watch_only = Wollet(network, descriptor, datadir=None)

    # A watch-only wallet can hand out receive addresses without any key.
    receive = watch_only.address(0)
    assert receive.index() == 0
    print(f"watch-only receive address #0: {receive.address()}")

    # Fund it and scan: balances are visible to the watcher.
    funded = 100_000
    txid = node.send_to_address(receive.address(), funded, asset=None)
    watch_only.wait_for_tx(txid, client)
    assert watch_only.balance()[policy_asset] == funded
    assert len(watch_only.utxos()) == 1
    print(f"watch-only balance after funding: {watch_only.balance()[policy_asset]} sats")

    # ----------------------------------------- online: build unsigned PSET
    sent = 25_000
    destination = node.get_new_address()

    builder = network.tx_builder()
    builder.add_lbtc_recipient(destination, sent)
    unsigned_pset = builder.finish(watch_only)

    # The PSET is unsigned: the wallet could not have signed it even if asked.
    details = watch_only.pset_details(unsigned_pset)
    sigs = details.signatures()
    assert all(len(s.has_signature()) == 0 for s in sigs), "watch-only must not sign"
    # ...but the watcher can still verify *what* it is about to send.
    recipients = details.balance().recipients()
    assert len(recipients) == 1
    assert recipients[0].value() == sent
    print(f"unsigned PSET: sending {recipients[0].value()} sats, "
          f"fee {details.balance().fee()} sats")

    # The unsigned PSET is what you would move to the cold device (QR, file, …).
    pset_for_signing = Pset(str(unsigned_pset))

    # ------------------------------------------------------ offline: sign
    # On the cold side, the signer reconstructs/holds the seed and signs. It
    # needs nothing from the online wallet except the PSET bytes.
    signed_pset = signer.sign(pset_for_signing)

    after = watch_only.pset_details(signed_pset)
    assert any(len(s.has_signature()) > 0 for s in after.signatures()), "now signed"

    # ------------------------------------------ online: finalize + broadcast
    finalized = watch_only.finalize(signed_pset)
    tx = finalized.extract_tx()
    fee = tx.fee(policy_asset)
    txid = client.broadcast(tx)
    watch_only.wait_for_tx(txid, client)

    expected = funded - sent - fee
    assert watch_only.balance()[policy_asset] == expected
    print(f"broadcast {txid}; watch-only balance now {expected} sats")

    # ----------------------------------- a second, independent watch-only
    # Any number of watchers can be reconstructed from the same descriptor and
    # will agree on history/balance without ever touching a key.
    another_watcher = Wollet(network, WolletDescriptor(exported_descriptor), datadir=None)
    another_watcher.wait_for_tx(txid, client)
    assert another_watcher.balance()[policy_asset] == watch_only.balance()[policy_asset]

    print("OK: watch-only watched, prepared, and verified; signer signed offline")


if __name__ == "__main__":
    main()
