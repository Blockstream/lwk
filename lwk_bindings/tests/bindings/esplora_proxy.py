#!/usr/bin/env python3
"""Esplora client through a proxy with LWK.

The `proxy` field of `EsploraClientBuilder` and `WaterfallsClientBuilder` sends
every request through a proxy, e.g. a local Tor daemon on `socks5h://127.0.0.1:9050`.
With the `socks5h` scheme the host of the server is resolved by the proxy and not
locally, so `.onion` urls work and no DNS query for the server leaves the machine.

This script runs offline: a minimal SOCKS5 proxy on localhost records the request
it receives and refuses it, which is enough to check that the client goes through
the proxy and hands it the host name.
"""

import socket
import threading

from lwk import *


def read_exact(conn, size):
    data = b""
    while len(data) < size:
        chunk = conn.recv(size - len(data))
        if not chunk:
            raise ConnectionError("connection closed")
        data += chunk
    return data


def refusing_proxy(listener, requests):
    """Accept one SOCKS5 CONNECT request, record its target and refuse it."""
    conn, _ = listener.accept()
    with conn:
        _version, methods = read_exact(conn, 2)
        read_exact(conn, methods)
        conn.sendall(bytes([0x05, 0x00]))  # no authentication required
        _version, command, _reserved, address_type, length = read_exact(conn, 5)
        host = read_exact(conn, length).decode()
        port = int.from_bytes(read_exact(conn, 2), "big")
        requests.append((command, address_type, host, port))
        # reply "host unreachable", bound address 0.0.0.0:0
        conn.sendall(bytes([0x05, 0x04, 0x00, 0x01, 0, 0, 0, 0, 0, 0]))


listener = socket.socket()
listener.bind(("127.0.0.1", 0))
listener.listen(1)
requests = []
thread = threading.Thread(target=refusing_proxy, args=(listener, requests))
thread.start()

url = "http://example.onion/api"
network = Network.mainnet()
# The url of the proxy, e.g. "socks5h://127.0.0.1:9050" for a local Tor daemon
proxy = "socks5h://127.0.0.1:%d" % listener.getsockname()[1]

# ANCHOR: esplora_proxy
builder = EsploraClientBuilder(base_url=url, network=network, proxy=proxy)
client = EsploraClient.from_builder(builder)
# ANCHOR_END: esplora_proxy
assert builder.proxy == proxy

# The request goes to the proxy, which here refuses to go any further.
try:
    client.tip()
    raise AssertionError("the proxy refused the connection")
except LwkError:
    pass

thread.join()
listener.close()

# CONNECT (1) to a domain name (3): the host was not resolved locally
assert requests == [(1, 3, "example.onion", 80)], requests

# The Waterfalls builder takes the same field
builder = WaterfallsClientBuilder(base_url=url, network=network, proxy=proxy)
assert builder.proxy == proxy
