from lwk import *

mnemonic = Mnemonic("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about")
network = Network.mainnet()
client = network.default_electrum_client()
signer = Signer(mnemonic, network)
desc = signer.wpkh_slip77_descriptor()
wollet = Wollet(network, desc, datadir=None)
claim_address = wollet.address(2).address()
print(claim_address)

lightning_session = LightningSession(network, client, 10)

invoice_response = lightning_session.invoice(1000, "ciao", claim_address)
bolt11_invoice = invoice_response.bolt11_invoice()
print(bolt11_invoice)
assert bolt11_invoice.startswith("lnbc1")

# invoice_response.complete_pay() # pay the invoice externally


## Pay invoice
# prepare_pay_response = lightning_session.prepare_pay(invoice)
# uri = prepare_pay_response.uri() # pay the liquid uri
# prepare_pay_response.complete_pay()
