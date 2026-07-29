#!/usr/bin/env python3
"""Authenticated Electrum client with LWK (OAuth2 / static token).

Some Electrum RPC proxies, e.g. Blockstream Enterprise, require authentication.
The bindings expose this through `ElectrumClientBuilder`'s `token_provider` field.

Unlike the Esplora client (HTTP, token fetched lazily on the first request), the
Electrum client connects when it is built, so building an authenticated client
needs a reachable proxy. This script therefore:

  * Offline checks (always run): assert the builder wiring, and that a token over
    a plaintext `tcp://` url is refused unless `allow_plaintext_with_token` is set.
    No network or credentials needed (the refusal is checked before connecting).
  * Live check (opt-in): if CLIENT_ID and CLIENT_SECRET are set, actually connect
    to an authenticated proxy (the URLs in `live_check`) and read the chain tip.
"""

import logging
import os
import sys

from lwk import *

logging.basicConfig(level=logging.INFO, format="%(levelname)s %(message)s")
log = logging.getLogger("authenticated_electrum")

client_id = os.environ.get("CLIENT_ID", "your_client_id")
client_secret = os.environ.get("CLIENT_SECRET", "your_client_secret")


def offline_checks():
    """Assert the builder wiring and the plaintext-token safeguard, no network."""
    url = "ssl://enterprise.blockstream.info:50002"
    login_url = "https://login.blockstream.com/realms/blockstream-public/protocol/openid-connect/token"

    # The OAuth2 provider is recorded on the builder (the connection, and thus the
    # token fetch, only happens when the client is built against a reachable proxy).
    builder = ElectrumClientBuilder(
        url=url,
        token_provider=TokenProvider.BLOCKSTREAM(
            url=login_url,
            client_id=client_id,
            client_secret=client_secret,
        ),
    )
    assert builder.token_provider.is_BLOCKSTREAM()
    assert builder.url == url
    log.info("built OAuth2 (Blockstream) Electrum builder for %s", url)

    # A static token (when you already have one) also works.
    builder = ElectrumClientBuilder(
        url=url,
        token_provider=TokenProvider.STATIC(token="my-token"),
    )
    assert builder.token_provider.is_STATIC()
    assert builder.token_provider.token == "my-token"
    log.info("built static-token Electrum builder")

    # A token over a plaintext `tcp://` url is refused unless explicitly allowed. This is
    # checked before connecting, so it fails offline (no proxy needed).
    refused = ElectrumClientBuilder(
        url="tcp://localhost:50001",
        token_provider=TokenProvider.STATIC(token="my-token"),
    )
    try:
        ElectrumClient.from_builder(refused)
        assert False, "expected a plaintext-token refusal"
    except LwkError as e:
        assert "plaintext" in str(e).lower(), f"unexpected error: {e}"
    log.info("plaintext token refused without allow_plaintext_with_token")

    # ... with `allow_plaintext_with_token` set, the same url would instead be allowed to
    # connect (only do this for a localhost or already-tunneled proxy).
    allowed = ElectrumClientBuilder(
        url="tcp://localhost:50001",
        token_provider=TokenProvider.STATIC(token="my-token"),
        allow_plaintext_with_token=True,
    )
    assert allowed.allow_plaintext_with_token
    log.info("offline checks passed")


def live_check():
    """If credentials are provided, connect to an authenticated proxy and read the tip."""
    if client_id == "your_client_id" or client_secret == "your_client_secret":
        log.info("skipping live check: set CLIENT_ID and CLIENT_SECRET to enable it")
        return

    log.info("authenticating ...")
    # ANCHOR: authenticated_electrum_client
    url = "ssl://enterprise.blockstream.info:50002"
    login_url = "https://login.blockstream.com/realms/blockstream-public/protocol/openid-connect/token"

    builder = ElectrumClientBuilder(
        url=url,
        token_provider=TokenProvider.BLOCKSTREAM(
            url=login_url, client_id=client_id, client_secret=client_secret
        ),
        timeout=30,
    )
    # Building connects to the proxy and mints the OAuth token (the connection's first
    # message carries it); the client is then ready for authenticated calls.
    client = ElectrumClient.from_builder(builder)
    tip = client.tip()
    # ANCHOR_END: authenticated_electrum_client

    log.info("authenticated OK: chain tip height=%d", tip.height())
    assert tip.height() > 100, f"unexpected tip height {tip.height()}"


def main():
    offline_checks()
    try:
        live_check()
    except LwkError as e:
        # A network/credential failure here shouldn't look like a wiring bug.
        log.error("live check failed (network or credentials): %s", e)
        return 1
    log.info("done")
    return 0


if __name__ == "__main__":
    sys.exit(main())
