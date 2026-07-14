from lwk import *


node = LwkTestEnv()
network = Network.regtest_default()
policy_asset = network.policy_asset()

mnemonic = Mnemonic.from_random(12)
signer = Signer(mnemonic, network)
desc = signer.wpkh_slip77_descriptor()
wollet = Wollet(network, desc, datadir=None)

client = WaterfallsClient(node.waterfalls_url(), network)
subscription = client.subscribe(desc)

update = client.full_scan(wollet)
if update is not None:
    wollet.apply_update(update)

sats = 100_000
address = wollet.address(0).address()
node.send_to_address(address, sats, asset=None)
node.generate(1)

event = subscription.next_update()
assert event is not None
assert event.kind in ["mempool", "block"]
assert event.tip is not None

if event.kind == "mempool":
    event = subscription.next_update()
    assert event is not None
    assert event.kind == "block"
    assert event.tip is not None

update = client.full_scan(wollet)
assert update is not None
wollet.apply_update(update)

assert wollet.balance()[policy_asset] == sats

node.generate(1)
event = subscription.next_update()
assert event is not None
assert event.kind == "tip"
assert event.tip is not None

subscription.close_subscription()
