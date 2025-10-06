from lwk import *

# Example implementation of the Logging trait
class MyLogger(Logging):
    def log(self, level: LogLevel, message: str):
        # Only print Info level and greater (Info, Warn, Error)
        if level in (LogLevel.INFO, LogLevel.WARN, LogLevel.ERROR):
            level_str = {
                LogLevel.INFO: "INFO",
                LogLevel.WARN: "WARN",
                LogLevel.ERROR: "ERROR",
            }.get(level, "UNKNOWN")
            print(f"[{level_str}] {message}")

mnemonic = Mnemonic("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about")
network = Network.mainnet()
client = network.default_electrum_client()
signer = Signer(mnemonic, network)
desc = signer.wpkh_slip77_descriptor()
wollet = Wollet(network, desc, datadir=None)
claim_address = wollet.address(2).address()
print(claim_address)

# Create a lightning session with custom logging
logger = MyLogger()
lightning_session = LightningSession(network, client, 10, logger)

invoice_response = lightning_session.invoice(1000, "ciao", claim_address)
bolt11_invoice = invoice_response.bolt11_invoice()
print(bolt11_invoice)
assert bolt11_invoice.startswith("lnbc1")

# invoice_response.complete_pay() # pay the invoice externally

## Pay invoice with MRH
## just an example of how to handle the MagicRoutingHint error by reusing our invoice
## in the real world any invoice generated from a Boltz-enabled wallet will contain a MRH
try:
    refund_address = wollet.address(3).address()
    prepare_pay_response = lightning_session.prepare_pay(bolt11_invoice, refund_address)
except LwkError.MagicRoutingHint as e:
    # Handle the specific MagicRoutingHint error
    print(f"Magic routing hint detected!")
    print(f"You can pay directly {e.uri}")
except Exception as e:
    # Handle any other unexpected errors
    print(f"Unexpected error: {e}")

# uri = prepare_pay_response.uri() # pay the liquid uri
# prepare_pay_response.complete_pay()
