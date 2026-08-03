from lwk import *

env = LwkTestEnv.new_with_amp2()

# from amp2 mock
server_key = "[18778d8c]tpubD6NzVbkrYhZ4XpebgtJWPiggq824Dv4Y7zzdAqRn1BcmoWH5ff163zmB98FY1tEg518yPNG8hm3ghnHJwQ1qxynTDS7orfzQwwz45H9Hx7c"
url = env.amp2_url()

mnemonic = Mnemonic("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about")
network = Network.regtest_default()
signer = Signer(mnemonic, network)
xpub = signer.keyorigin_xpub(Bip.new_bip87())

expected_desc = "ct(slip77(0684e43749a3a3eb0362dcef8c66994bd51d33f8ce6b055126a800a626fc0d67),elwsh(multi(2,[18778d8c]tpubD6NzVbkrYhZ4XpebgtJWPiggq824Dv4Y7zzdAqRn1BcmoWH5ff163zmB98FY1tEg518yPNG8hm3ghnHJwQ1qxynTDS7orfzQwwz45H9Hx7c/<0;1>/*,[73c5da0a/87'/1'/0']tpubDCChhoz5Qdrkn7Z7KXawq6Ad6r3A4MUkCoVTqeWxfTkA6bHNJ3CHUEtALQdkNeixNz4446PcAmw4WKcj3mV2vb29H7sg9EPzbyCU1y2merw/<0;1>/*)))#a63khg64"
# tmp hardcoded blinding key
descriptor_blinding_key = "slip77(0684e43749a3a3eb0362dcef8c66994bd51d33f8ce6b055126a800a626fc0d67)"

amp2 = Amp2(server_key, url)
desc = amp2.descriptor_from_str(xpub, descriptor_blinding_key)

dwid = amp2.register_wallet(desc)
assert dwid == desc.descriptor().dwid(network)

wollet = Wollet(network, desc.descriptor(), datadir=None)
client = ElectrumClient.from_url(env.electrum_url())

sats = 100000
txid = env.send_to_address(wollet.address(0).address(), sats, asset=None)
wollet.wait_for_tx(txid, client)

sent_sats = 1000
node_address = env.get_new_address()

b = network.tx_builder()
b.add_lbtc_recipient(node_address, sent_sats)
pset = b.finish(wollet)

pset = signer.sign(pset)
pset = amp2.cosign(pset)

pset = wollet.finalize(pset)

tx = pset.extract_tx()
txid = client.broadcast(tx)
wollet.wait_for_tx(txid, client)

# note: this is might be rejected by amp2
custom_desc = "ct(1111111111111111111111111111111111111111111111111111111111111111,elwsh(and_v(v:pk(026a04ab98d9e4774ad806e302dddeb63bea16b5cb5f223ee77478e861bb583eb3),multi(2,[342c8926/87h/1h/0h]tpubDDWUA7YvBHxdurKUrYFkdjsB59koHqvGRJ3j9zDhwMycxXHXz1ujTfHMB66K4rEWDM8BoDKDdJx3rVGp2qUSPnXVpQXi8qtnXqa96nPnZAH/0/*,[af9e5bc2/87h/1h/0h]tpubDDRPayLs2vBkRkyQ9X2BEhojxCy9vvZpjhubEVosz5pi66LuuAuyZQiUtsPBN5wSfhWLoMYM3gqVqT3Po4GpcWGUfPh8514ZBB9hfWFNEUA/0/*,[57411aec/87h/1h/0h]tpubDDmweWcTcRb54kZqy3Gv5JF8SjAyuoK3uPYXp24uz6nfsKjJojxjdZAang5HXDmtS8tg5CJntUC4fzn4aY5Dsg6Aphvq42vK9edmgX83NFg/0/*))))";
amp2_desc = Amp2Descriptor.new_with_custom_descriptor(WolletDescriptor(custom_desc))

# e2e with elip153 desc
account = 0
desc = amp2.elip153_from_signer(signer, account)

# getting the descriptor with signer managed externally leads to the same descriptor
user_path = amp2.elip153_user_path(account)
view_path = amp2.elip153_view_path(account)
user_keyorigin_xpub = signer.keyorigin_xpub_from_path(user_path)
view_keyorigin_xpub = signer.keyorigin_xpub_from_path(view_path)
desc_from_str = amp2.elip153_from_str(user_keyorigin_xpub, view_keyorigin_xpub)
assert str(desc.descriptor()) == str(desc_from_str.descriptor())

dwid = amp2.register_wallet(desc)

wollet = Wollet(network, desc.descriptor(), datadir=None)
client = ElectrumClient.from_url(env.electrum_url())

sats = 100000
txid = env.send_to_address(wollet.address(0).address(), sats, asset=None)
wollet.wait_for_tx(txid, client)

sent_sats = 1000
node_address = env.get_new_address()

b = network.tx_builder()
b.add_lbtc_recipient(node_address, sent_sats)
pset = b.finish(wollet)

pset = signer.sign(pset)
pset = amp2.cosign(pset)

pset = wollet.finalize(pset)

tx = pset.extract_tx()
txid = client.broadcast(tx)
wollet.wait_for_tx(txid, client)
