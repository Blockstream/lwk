from lwk import *


network = Network.regtest_default()

mnemonic1 = Mnemonic("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about")
mnemonic2 = Mnemonic("tissue mix draw siren diesel escape menu misery tube yellow zoo measure")

signer1 = Signer(mnemonic1, network)
signer2 = Signer(mnemonic2, network)
xpub1 = signer1.keyorigin_xpub(Bip.new_bip87())
xpub2 = signer2.keyorigin_xpub(Bip.new_bip87())

desc = f"ct(elip151,elwsh(multi(2,{xpub1}/<0;1>/*,{xpub2}/<0;1>/*)))"
desc = WolletDescriptor(desc)

assert str(desc) == "ct(f68ce538e5774697e9b8e45528597ccb93f40b0a67e6853a954fc2db4afc8e9b,elwsh(multi(2,[73c5da0a/87'/1'/0']tpubDCChhoz5Qdrkn7Z7KXawq6Ad6r3A4MUkCoVTqeWxfTkA6bHNJ3CHUEtALQdkNeixNz4446PcAmw4WKcj3mV2vb29H7sg9EPzbyCU1y2merw/<0;1>/*,[0f04356d/87'/1'/0']tpubDD2NZt5nWoiA5uuWNNWw8eKiexd8EFs8kwChV5DrzkXQ3ZoNu3SZdAmD82z78oYGmt4aihPi5rPfEFNZGs7C9WiAshoD7UEtL5R79Jo76TA/<0;1>/*)))#d8apxcd6"
