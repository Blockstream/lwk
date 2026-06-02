#!/usr/bin/env python3
"""Authenticated Esplora client with LWK (OAuth2 / static token / headers).

Some Esplora servers — e.g. Blockstream Enterprise — require authentication.
The bindings expose this through `EsploraClientBuilder`'s `token_provider` and
`headers` fields.

This script does two things:

  * Offline checks (always run): build authenticated clients and assert the
    builder wiring behaves as expected. No network or credentials needed,
    because the OAuth token is only fetched on the first request.
  * Live check (opt-in): if CLIENT_ID and CLIENT_SECRET are set, actually hit
    an authenticated endpoint and assert we can read the chain tip. Point it at
    a different host/login with ESPLORA_BASE_URL / ESPLORA_LOGIN_URL.
"""

import logging
import os
import sys

from lwk import *

logging.basicConfig(level=logging.INFO, format="%(levelname)s %(message)s")
log = logging.getLogger("authenticated_esplora")

network = Network.mainnet()

client_id = os.environ.get("CLIENT_ID", "your_client_id")
client_secret = os.environ.get("CLIENT_SECRET", "your_client_secret")


def offline_checks():
    """Build authenticated clients and assert the wiring, without any network."""
    # ANCHOR: authenticated_esplora_client
    base_url = "https://enterprise.blockstream.info/liquid/api"
    login_url = "https://login.blockstream.com/realms/blockstream-public/protocol/openid-connect/token"

    builder = EsploraClientBuilder(
        base_url=base_url,
        network=network,
        token_provider=TokenProvider.BLOCKSTREAM(
            url=login_url,
            client_id=client_id,
            client_secret=client_secret,
        ),
    )
    client = EsploraClient.from_builder(builder)
    # ANCHOR_END: authenticated_esplora_client

    # The OAuth2 provider is recorded on the builder and the client is built.
    assert builder.token_provider.is_BLOCKSTREAM()
    assert client is not None
    log.info("built OAuth2 (Blockstream) client for %s", base_url)

    # A static token (when you already have one) and/or custom headers also work.
    builder = EsploraClientBuilder(
        base_url=base_url,
        network=network,
        token_provider=TokenProvider.STATIC(token="my-token"),
        headers={"X-Custom-Header": "value"},
    )
    client = EsploraClient.from_builder(builder)

    assert builder.token_provider.is_STATIC()
    assert builder.token_provider.token == "my-token"
    assert builder.headers == {"X-Custom-Header": "value"}
    assert client is not None
    log.info("built static-token client with custom headers")

    # No-auth builders must keep working (auth fields default to None).
    builder = EsploraClientBuilder(base_url=base_url, network=network)
    assert builder.token_provider is None
    assert builder.headers is None
    assert EsploraClient.from_builder(builder) is not None
    log.info("offline checks passed")


def live_check():
    """If credentials are provided, prove auth works by reading the chain tip."""
    if client_id == "your_client_id" or client_secret == "your_client_secret":
        log.info("skipping live check: set CLIENT_ID and CLIENT_SECRET to enable it")
        return

    base_url = os.environ.get(
        "ESPLORA_BASE_URL", "https://enterprise.uat.blockstream.info/liquid/api"
    )
    login_url = os.environ.get(
        "ESPLORA_LOGIN_URL",
        "https://login.uat.blockstream.com/realms/blockstream-public/protocol/openid-connect/token",
    )

    builder = EsploraClientBuilder(
        base_url=base_url,
        network=network,
        token_provider=TokenProvider.BLOCKSTREAM(
            url=login_url, client_id=client_id, client_secret=client_secret
        ),
        timeout=30,
    )
    client = EsploraClient.from_builder(builder)

    log.info("authenticating against %s ...", base_url)
    tip = client.tip()  # triggers the OAuth token fetch + an authenticated request
    log.info("authenticated OK: chain tip height=%d hash=%s", tip.height(), tip.block_hash())
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
