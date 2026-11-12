#!/usr/bin/env python3
"""Electrum client through a SOCKS5 proxy with LWK.

`ElectrumClientBuilder`'s `socks5` field sends the Electrum connection through a
SOCKS5 proxy, e.g. a local Tor daemon on `127.0.0.1:9050`. The host of the Electrum
server is resolved by the proxy and not locally, so `.onion` urls work and no DNS
query for the server leaves the machine.

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

url = "tcp://example.onion:50001"
# The `host:port` of the SOCKS5 proxy, e.g. "127.0.0.1:9050" for a local Tor daemon
proxy = "127.0.0.1:%d" % listener.getsockname()[1]

# ANCHOR: electrum_socks5
builder = ElectrumClientBuilder(url=url, socks5=proxy)
# ANCHOR_END: electrum_socks5
assert builder.socks5 == proxy

# Building the client connects, here to a proxy that refuses to go any further.
try:
    ElectrumClient.from_builder(builder)
    raise AssertionError("the proxy refused the connection")
except LwkError:
    pass

thread.join()
listener.close()

# CONNECT (1) to a domain name (3): the host was not resolved locally
assert requests == [(1, 3, "example.onion", 50001)], requests
