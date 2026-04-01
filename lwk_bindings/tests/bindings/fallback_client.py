from lwk import *

node = LwkTestEnv()

network = Network.regtest_default()

signer = Signer(Mnemonic.from_random(12), network)
wollet = Wollet(network, signer.wpkh_slip77_descriptor(), datadir=None)

primary_url = node.electrum_url()
fallback_url = node.electrum_url()  # In practice, this would be a different server

# ANCHOR: fallback_client
client = ElectrumClient.from_url(primary_url)

try:
    update = client.full_scan(wollet)

except Exception:
    # Falling into a retryable error, making a request with the fallback client
    fallback_client = ElectrumClient.from_url(fallback_url)
    update = fallback_client.full_scan(wollet)

wollet.apply_update(update)
# ANCHOR_END: fallback_client
