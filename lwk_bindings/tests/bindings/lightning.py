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
mnemonic_lightning = signer.derive_bip85_mnemonic(0, 12) # for security reasons using a different mnemonic for the lightning session
lightning_session = LightningSession(network=network, client=client, timeout=10, logging=logger, mnemonic=mnemonic_lightning)

invoice_response = lightning_session.invoice(amount=1000, description="ciao", claim_address=claim_address, webhook=None)
bolt11_invoice_obj = invoice_response.bolt11_invoice()
bolt11_invoice = str(bolt11_invoice_obj)
print(bolt11_invoice)
assert bolt11_invoice.startswith("lnbc1")

# invoice_response.complete_pay() # pay the invoice externally

## Pay invoice with MRH
## just an example of how to handle the MagicRoutingHint error by reusing our invoice
## in the real world any invoice generated from a Boltz-enabled wallet will contain a MRH
try:
    refund_address = wollet.address(3).address()
    prepare_pay_response = lightning_session.prepare_pay(bolt11_invoice_obj, refund_address, None) # optionally accept a WebHook("https://example.com/webhook")
except LwkError.MagicRoutingHint as e:
    # Handle the specific MagicRoutingHint error
    print(f"Magic routing hint detected!")
    print(f"You can pay directly {e.uri}")
except Exception as e:
    # Handle any other unexpected errors
    print(f"Unexpected error: {e}")

# uri = prepare_pay_response.uri() # pay the liquid uri
# prepare_pay_response.complete_pay()
