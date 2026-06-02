#!/usr/bin/env python3
"""Non-trivial watch-only test driven by an authenticated Esplora client.

This scans a *real* mainnet Liquid wallet through an authenticated Blockstream
Enterprise endpoint and asserts non-trivial facts about its on-chain history.

It needs NO private keys: the wallet is watch-only, built from a public CT
descriptor (an xpub). Scanning only ever reads the chain. The only secrets
involved are the OAuth2 *API* credentials for the authenticated endpoint, which
are unrelated to wallet signing keys.

This mirrors the Rust `esplora_authenticated_wallet_scan_*` integration tests:
it exercises both the plain Esplora and the Waterfalls code paths through the
authenticated client and checks they agree.

Run it (credentials required):

    CLIENT_ID=... CLIENT_SECRET=... \
    PYTHONPATH=$PWD/target/release/bindings \
    python3 lwk_bindings/tests/bindings/authenticated_wallet_scan.py

Override the endpoint/realm with ESPLORA_BASE_URL / ESPLORA_LOGIN_URL
(defaults target the production enterprise endpoint). For UAT, e.g.:

    ESPLORA_BASE_URL=https://enterprise.uat.blockstream.info/liquid/api
    ESPLORA_LOGIN_URL=https://login.uat.blockstream.com/realms/blockstream-public/protocol/openid-connect/token
"""

import logging
import os
import sys

from lwk import *

logging.basicConfig(level=logging.INFO, format="%(levelname)s %(message)s")
log = logging.getLogger("authenticated_wallet_scan")

# A mainnet (coin type 1776) watch-only wp2kh descriptor with known USDt history.
# Same descriptor used by the Rust authenticated-scan integration tests; it has
# more than 16 transactions on chain. No private key material here.
DESCRIPTOR = (
    "ct(slip77(2411e278affa5c47010eab6d313c1ec66628ec0dd03b6fc98d1a05a0618719e6),"
    "elwpkh([a8874235/84'/1776'/0']xpub6DLHCiTPg67KE9ksCjNVpVHTRDHzhCSmoBTKzp2K4Fx"
    "LQwQvvdNzuqxhK2f9gFVCN6Dori7j2JMLeDoB4VqswG7Et9tjqauAvbDmzF8NEPH/<0;1>/*))"
    "#upsg7h8m"
)

MIN_TXS = 16  # the wallet has more than this many transactions on chain

BASE_URL = os.environ.get(
    "ESPLORA_BASE_URL", "https://enterprise.blockstream.info/liquid/api"
)
LOGIN_URL = os.environ.get(
    "ESPLORA_LOGIN_URL",
    "https://login.blockstream.com/realms/blockstream-public/protocol/openid-connect/token",
)


def make_client(network, *, waterfalls):
    """Build an authenticated Esplora (or Waterfalls) client."""
    client_id = os.environ["CLIENT_ID"]
    client_secret = os.environ["CLIENT_SECRET"]
    # The Waterfalls API is served under a `/waterfalls` sub-path of the
    # enterprise endpoint, e.g. `.../liquid/api/waterfalls`.
    base_url = BASE_URL.rstrip("/") + "/waterfalls" if waterfalls else BASE_URL
    builder = EsploraClientBuilder(
        base_url=base_url,
        network=network,
        waterfalls=waterfalls,
        timeout=30,
        token_provider=TokenProvider.BLOCKSTREAM(
            url=LOGIN_URL, client_id=client_id, client_secret=client_secret
        ),
    )
    return EsploraClient.from_builder(builder)


def invalid_creds_check(network):
    """An authenticated request with bad credentials must error clearly.

    This needs network but neither valid credentials nor credits: the OAuth
    token request itself is rejected, so `tip()` must raise rather than return.
    """
    builder = EsploraClientBuilder(
        base_url=BASE_URL,
        network=network,
        timeout=30,
        token_provider=TokenProvider.BLOCKSTREAM(
            url=LOGIN_URL,
            client_id="definitely-not-a-real-client",
            client_secret="definitely-not-a-real-secret",
        ),
    )
    client = EsploraClient.from_builder(builder)
    try:
        client.tip()
    except LwkError as e:
        log.info("invalid credentials correctly rejected: %s", e)
        return
    raise AssertionError("expected an error when using invalid credentials")


def scan(network, *, waterfalls):
    """Watch-only full scan through the authenticated client; return the wallet."""
    label = "waterfalls" if waterfalls else "esplora"
    client = make_client(network, waterfalls=waterfalls)

    # Reading the tip forces the OAuth token fetch and proves auth works.
    tip = client.tip()
    log.info("[%s] authenticated; chain tip height=%d", label, tip.height())
    assert tip.height() > 100, f"[{label}] unexpected tip height {tip.height()}"

    wollet = Wollet(network, WolletDescriptor(DESCRIPTOR), datadir=None)
    update = client.full_scan(wollet)
    assert update is not None, f"[{label}] expected the first scan to return an update"
    wollet.apply_update(update)

    txs = wollet.transactions()
    balance = wollet.balance()
    utxos = wollet.utxos()
    log.info(
        "[%s] scanned: %d txs, %d assets, %d utxos",
        label, len(txs), len(balance), len(utxos),
    )
    for asset, amount in balance.items():
        log.info("[%s]   balance %s = %d", label, asset, amount)

    # ---- non-trivial assertions about the recovered on-chain state ----
    assert len(txs) > MIN_TXS, f"[{label}] expected > {MIN_TXS} txs, got {len(txs)}"

    # Every UTXO must be confidential and consistent with the wallet.
    for utxo in utxos:
        secrets = utxo.unblinded()
        assert secrets.value() >= 0
        assert utxo.ext_int() in (Chain.EXTERNAL, Chain.INTERNAL)
        assert balance.get(secrets.asset(), 0) >= secrets.value()

    # Balance is the sum of UTXO values per asset (sanity-check the accounting).
    by_asset = {}
    for utxo in utxos:
        s = utxo.unblinded()
        by_asset[s.asset()] = by_asset.get(s.asset(), 0) + s.value()
    assert by_asset == dict(balance), f"[{label}] utxo sums {by_asset} != balance {dict(balance)}"

    # Transactions are ordered/queryable and round-trip by txid.
    first = txs[0]
    assert str(wollet.transaction(first.txid()).txid()) == str(first.txid())

    return wollet


def main():
    if not os.environ.get("CLIENT_ID") or not os.environ.get("CLIENT_SECRET"):
        log.info("skipping: set CLIENT_ID and CLIENT_SECRET to run the authenticated scan")
        return 0

    network = Network.mainnet()
    try:
        # Negative path: bad credentials must fail clearly.
        invalid_creds_check(network)
        # Positive path: scan via both code paths with valid credentials.
        w_esplora = scan(network, waterfalls=False)
        w_waterfalls = scan(network, waterfalls=True)
    except LwkError as e:
        log.error("authenticated scan failed (network or credentials): %s", e)
        return 1

    # The two independent code paths must agree on the wallet's balance.
    assert w_esplora.balance() == w_waterfalls.balance(), (
        f"esplora {w_esplora.balance()} != waterfalls {w_waterfalls.balance()}"
    )
    log.info("esplora and waterfalls scans agree; OK")
    return 0


if __name__ == "__main__":
    sys.exit(main())
