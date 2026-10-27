#!/usr/bin/env python3
"""Bound a wallet scan in time and size.

A scan downloads whatever the server sends, so a slow or misbehaving server
could keep it busy forever or make it hold a huge response in memory.
Both `EsploraClientBuilder` and `WaterfallsClientBuilder` accept:

  * `scan_timeout`: maximum duration in seconds of a whole scan, default 10 minutes.
  * `max_response_size`: maximum size in bytes of a single response, default 64 MiB.

Increase them for wallets with a very large history.

This script runs offline against a local HTTP server.
"""

import http.server
import threading
import time

from lwk import *


class SlowHandler(http.server.BaseHTTPRequestHandler):
    """Start a response and never finish it."""

    def do_GET(self):
        self.send_response(200)
        self.end_headers()
        time.sleep(30)

    def log_message(self, *args):
        pass


class SmallHandler(http.server.BaseHTTPRequestHandler):
    """Answer every request with a 2 bytes body."""

    def do_GET(self):
        self.send_response(200)
        self.send_header("Content-Length", "2")
        self.end_headers()
        self.wfile.write(b"[]")

    def log_message(self, *args):
        pass


def serve(handler):
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler)
    server.daemon_threads = True
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return f"http://127.0.0.1:{server.server_address[1]}"


def assert_scan_fails(client, wollet, expected_error):
    start = time.monotonic()
    try:
        client.full_scan(wollet)
    except LwkError as e:
        assert expected_error in str(e), f"unexpected error: {e}"
        return time.monotonic() - start
    raise AssertionError(f"scan should fail with {expected_error}")


network = Network.testnet()
mnemonic = Mnemonic("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about")
desc = Signer(mnemonic, network).wpkh_slip77_descriptor()
wollet = Wollet(network, desc, datadir=None)

slow_url = serve(SlowHandler)
small_url = serve(SmallHandler)

builder = EsploraClientBuilder(
    base_url=slow_url,
    network=network,
    scan_timeout=1,  # seconds
    max_response_size=1024 * 1024,  # bytes
)
client = EsploraClient.from_builder(builder)

# The server never completes a response: the scan stops after `scan_timeout`,
# well before the per-request timeout (5 seconds) would.
elapsed = assert_scan_fails(client, wollet, "ScanTimeout")
assert elapsed < 4, f"scan took {elapsed:.1f}s"

# The server answers with more bytes than allowed.
builder = EsploraClientBuilder(base_url=small_url, network=network, max_response_size=1)
client = EsploraClient.from_builder(builder)
assert_scan_fails(client, wollet, "ResponseTooLarge")

# The same options are available on Waterfalls clients.
builder = WaterfallsClientBuilder(base_url=slow_url, network=network, scan_timeout=1)
client = WaterfallsClient.from_builder(builder)
elapsed = assert_scan_fails(client, wollet, "ScanTimeout")
assert elapsed < 4, f"scan took {elapsed:.1f}s"

builder = WaterfallsClientBuilder(base_url=small_url, network=network, max_response_size=1)
client = WaterfallsClient.from_builder(builder)
assert_scan_fails(client, wollet, "ResponseTooLarge")
