"""Exercise Waterfalls reconnect updates through the generated Python bindings."""

from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from threading import Event, Thread
from urllib.request import urlopen

from lwk import (
    LwkTestEnv,
    Mnemonic,
    Network,
    Signer,
    WaterfallsClient,
    WaterfallsSubscriptionUpdate,
)


node = LwkTestEnv()
network = Network.regtest_default()
descriptor = Signer(Mnemonic.from_random(12), network).wpkh_slip77_descriptor()

# The local SSE server uses the test Waterfalls server's recipient so descriptor
# encryption follows the same path as a real subscription.
with urlopen(f"{node.waterfalls_url()}/v1/server_recipient") as response:
    recipient = response.read()


class Handler(BaseHTTPRequestHandler):
    subscriptions = 0

    def do_GET(self):
        if self.path == "/v1/server_recipient":
            body = recipient
            content_type = "application/json"
        elif self.path.startswith("/v1/subscribe?"):
            Handler.subscriptions += 1
            event_type = "tip" if Handler.subscriptions == 1 else "mempool"
            body = f'event: update\ndata: {{"type":"{event_type}"}}\n\n'.encode()
            content_type = "text/event-stream"
        else:
            self.send_error(404)
            return

        self.send_response(200)
        self.send_header("Content-Type", content_type)
        if content_type != "text/event-stream":
            self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)
        self.wfile.flush()
        # The first SSE handler returns here, closing its HTTP response. The
        # client sees EOF as a disconnect and opens a new subscription.
        # Keep the second response open until the test closes the subscription.
        if content_type == "text/event-stream" and Handler.subscriptions == 2:
            self.server.hold.wait(timeout=10)

    def log_message(self, format, *args):
        pass


server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
server.hold = Event()
server_thread = Thread(target=server.serve_forever, daemon=True)
server_thread.start()

subscription = None
try:
    client = WaterfallsClient(f"http://127.0.0.1:{server.server_port}", network)
    subscription = client.subscribe_reconnecting(descriptor)

    update = subscription.next_update()
    assert isinstance(update, WaterfallsSubscriptionUpdate.EVENT)
    assert update.event.kind == "tip"

    update = subscription.next_update()
    assert isinstance(update, WaterfallsSubscriptionUpdate.DISCONNECTED)
    assert update.error is None

    update = subscription.next_update()
    assert isinstance(update, WaterfallsSubscriptionUpdate.RECONNECTED)

    update = subscription.next_update()
    assert isinstance(update, WaterfallsSubscriptionUpdate.EVENT)
    assert update.event.kind == "mempool"
    assert Handler.subscriptions >= 2
finally:
    if subscription is not None:
        subscription.close_subscription()
        assert subscription.next_update() is None
    server.hold.set()
    server.shutdown()
    server.server_close()
