#!/usr/bin/env python3
"""Authenticated Electrum client with LWK (OAuth2 / static token).

Some Electrum RPC proxies, e.g. Blockstream Enterprise, require authentication.
The bindings expose this through `ElectrumClientBuilder`'s `token_provider` field.

Unlike the Esplora client (HTTP, token fetched lazily on the first request), the
Electrum client connects when it is built, so building an authenticated client
needs a reachable proxy. This script therefore:

  * Offline checks (always run): assert the builder wiring. No network or
    credentials needed.
  * Live check (opt-in): if CLIENT_ID and CLIENT_SECRET are set, actually connect
    to an authenticated proxy (the URLs in `live_check`) and read the chain tip.
"""

import os
import sys

from lwk import *

client_id = os.environ.get("CLIENT_ID", "your_client_id")
client_secret = os.environ.get("CLIENT_SECRET", "your_client_secret")


def offline_checks():
    """Assert the builder wiring, no network."""
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

    # A static token (when you already have one) also works.
    builder = ElectrumClientBuilder(
        url=url,
        token_provider=TokenProvider.STATIC(token="my-token"),
    )
    assert builder.token_provider.is_STATIC()
    assert builder.token_provider.token == "my-token"

    print("offline checks passed")


def live_check():
    """If credentials are provided, connect to an authenticated proxy and read the tip."""
    if client_id == "your_client_id" or client_secret == "your_client_secret":
        print("skipping live check: set CLIENT_ID and CLIENT_SECRET to enable it")
        return

    print("authenticating ...")
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

    print(f"authenticated OK: chain tip height={tip.height()}")
    assert tip.height() > 100, f"unexpected tip height {tip.height()}"


def main():
    offline_checks()
    try:
        live_check()
    except LwkError as e:
        # A network/credential failure here shouldn't look like a wiring bug.
        print(f"live check failed (network or credentials): {e}")
        return 1
    print("done")
    return 0


if __name__ == "__main__":
    sys.exit(main())
