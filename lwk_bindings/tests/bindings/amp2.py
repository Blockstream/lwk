from lwk import *

mnemonic = Mnemonic("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about")
network = Network.regtest_default()

expected_desc = "ct(slip77(0684e43749a3a3eb0362dcef8c66994bd51d33f8ce6b055126a800a626fc0d67),elwsh(multi(2,[3d970d04/87'/1'/0']tpubDC347GyKEGtyd4swZDaEmBTcNuqseyX7E3Yw58FoeV1njuBcUmBMr5vBeBh6eRsxKYHeCAEkKj8J2p2dBQQJwB8n33uyAPrdgwFxLFTCXRd/<0;1>/*,[73c5da0a/87'/1'/0']tpubDCChhoz5Qdrkn7Z7KXawq6Ad6r3A4MUkCoVTqeWxfTkA6bHNJ3CHUEtALQdkNeixNz4446PcAmw4WKcj3mV2vb29H7sg9EPzbyCU1y2merw/<0;1>/*)))#k3449ejr"
descriptor_blinding_key = "slip77(0684e43749a3a3eb0362dcef8c66994bd51d33f8ce6b055126a800a626fc0d67)"

signer = Signer(mnemonic, network)

amp2 = Amp2.new_testnet()
xpub = signer.keyorigin_xpub(Bip.new_bip87())
desc = amp2.descriptor_from_str(xpub, descriptor_blinding_key)
assert str(desc) == expected_desc

server_key = "[3d970d04/87'/1'/0']tpubDC347GyKEGtyd4swZDaEmBTcNuqseyX7E3Yw58FoeV1njuBcUmBMr5vBeBh6eRsxKYHeCAEkKj8J2p2dBQQJwB8n33uyAPrdgwFxLFTCXRd"
url = "http://127.0.0.1:5000"
amp2 = Amp2(server_key, url)
desc = amp2.descriptor_from_str(xpub, descriptor_blinding_key)
assert str(desc) == expected_desc

# note: this is might be rejected by amp2
custom_desc = "ct(1111111111111111111111111111111111111111111111111111111111111111,elwsh(and_v(v:pk(026a04ab98d9e4774ad806e302dddeb63bea16b5cb5f223ee77478e861bb583eb3),multi(2,[342c8926/87h/1h/0h]tpubDDWUA7YvBHxdurKUrYFkdjsB59koHqvGRJ3j9zDhwMycxXHXz1ujTfHMB66K4rEWDM8BoDKDdJx3rVGp2qUSPnXVpQXi8qtnXqa96nPnZAH/0/*,[af9e5bc2/87h/1h/0h]tpubDDRPayLs2vBkRkyQ9X2BEhojxCy9vvZpjhubEVosz5pi66LuuAuyZQiUtsPBN5wSfhWLoMYM3gqVqT3Po4GpcWGUfPh8514ZBB9hfWFNEUA/0/*,[57411aec/87h/1h/0h]tpubDDmweWcTcRb54kZqy3Gv5JF8SjAyuoK3uPYXp24uz6nfsKjJojxjdZAang5HXDmtS8tg5CJntUC4fzn4aY5Dsg6Aphvq42vK9edmgX83NFg/0/*))))";
amp2_desc = Amp2Descriptor.new_with_custom_descriptor(WolletDescriptor(custom_desc))
