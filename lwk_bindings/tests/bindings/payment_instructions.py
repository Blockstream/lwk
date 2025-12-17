from lwk import Payment, PaymentKind, Network, Bip21

# Test Bitcoin address (no schema)
bitcoin_address = "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa"
pay = Payment(bitcoin_address)
assert pay.kind() == PaymentKind.BITCOIN_ADDRESS
assert str(pay.bitcoin_address()) == bitcoin_address
assert pay.liquid_address() is None
# Non-lightning payegories should return None for lightning_payment()
assert pay.lightning_payment() is None
assert pay.bitcoin_address().is_mainnet()

# Test Bitcoin address with schema
pay = Payment(f"bitcoin:{bitcoin_address}")
assert pay.kind() == PaymentKind.BITCOIN_ADDRESS
assert str(pay.bitcoin_address()) == bitcoin_address
assert pay.bitcoin_address().is_mainnet()

# Test Bitcoin address with uppercase schema
pay = Payment(f"BITCOIN:{bitcoin_address}")
assert pay.kind() == PaymentKind.BITCOIN_ADDRESS
assert str(pay.bitcoin_address()) == bitcoin_address
assert pay.bitcoin_address().is_mainnet()


# Test Bitcoin segwit address
bitcoin_segwit = "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq"
pay = Payment(bitcoin_segwit)
assert pay.kind() == PaymentKind.BITCOIN_ADDRESS
assert str(pay.bitcoin_address()) == bitcoin_segwit
assert pay.bitcoin_address().is_mainnet()

# Test Bitcoin testnet address
bitcoin_testnet = "tb1p0ypzcwy0wjxg5whycnl4vxsrcxcgplgfxqvgczv9l6j8kp333lusfht5tq"
pay = Payment(bitcoin_testnet)
assert pay.kind() == PaymentKind.BITCOIN_ADDRESS
assert str(pay.bitcoin_address()) == bitcoin_testnet
assert not pay.bitcoin_address().is_mainnet()

# Test Liquid address (no schema)
liquid_address = "lq1qqduq2l8maf4580wle4hevmk62xqqw3quckshkt2rex3ylw83824y4g96xl0uugdz4qks5v7w4pdpvztyy5kw7r7e56jcwm0p0"
pay = Payment(liquid_address)
assert pay.kind() == PaymentKind.LIQUID_ADDRESS
assert pay.liquid_address() is not None
assert str(pay.liquid_address()) == liquid_address
assert pay.liquid_address().network() == Network.mainnet()
assert pay.bitcoin_address() is None

# Test Liquid address with schema
pay = Payment(f"liquidnetwork:{liquid_address}")
assert pay.kind() == PaymentKind.LIQUID_ADDRESS
assert str(pay.liquid_address()) == liquid_address

# Test Liquid testnet address (no schema)
liquid_testnet_address = "tlq1qq02egjncr8g4qn890mrw3jhgupwqymekv383lwpmsfghn36hac5ptpmeewtnftluqyaraa56ung7wf47crkn5fjuhk422d68m"
pay = Payment(liquid_testnet_address)
assert pay.kind() == PaymentKind.LIQUID_ADDRESS
assert pay.liquid_address() is not None
assert str(pay.liquid_address()) == liquid_testnet_address
assert pay.liquid_address().network() == Network.testnet()

# Test Liquid testnet address with schema
pay = Payment(f"liquidtestnet:{liquid_testnet_address}")
assert pay.kind() == PaymentKind.LIQUID_ADDRESS
assert str(pay.liquid_address()) == liquid_testnet_address
assert pay.liquid_address().network() == Network.testnet()

# Test Lightning invoice
lightning_invoice = "lnbc23230n1p5sxxunsp5tep5yrw63cy3tk74j3hpzqzhhzwe806wk0apjfsfn5x9wmpkzkdspp5z4f40v2whks0aj3kx4zuwrrem094pna4ehutev2p63djtff02a2sdquf35kw6r5de5kueeqwpshjmt9de6qxqyp2xqcqzxrrzjqf6rgswuygn5qr0p5dt2mvklrrcz6yy8pnzqr3eq962tqwprpfrzkzzxeyqq28qqqqqqqqqqqqqqq9gq2yrzjqtnpp8ds33zeg5a6cumptreev23g7pwlp39cvcz8jeuurayvrmvdsrw9ysqqq9gqqqqqqqqpqqqqq9sq2g9qyysgqqufsg7s6qcmfmjxvkf0ulupufr0yfqeajnv3mvtyqzz2rfwre2796rnkzsw44lw3nja5frg4w4m59xqlwwu774h4f79ysm05uugckugqdf84yl"
pay = Payment(lightning_invoice)
assert pay.kind() == PaymentKind.LIGHTNING_INVOICE
invoice = pay.lightning_invoice()
assert invoice is not None
assert str(invoice) == lightning_invoice
assert invoice.amount_milli_satoshis() == 2323000
assert pay.lightning_offer() is None
# Test lightning_payment() returns a LightningPayment for invoices
lp = pay.lightning_payment()
assert lp is not None

# Test Lightning invoice with schema
pay = Payment(f"lightning:{lightning_invoice}")
assert pay.kind() == PaymentKind.LIGHTNING_INVOICE
assert pay.lightning_payment() is not None

# Test Bolt12 offer
bolt12 = "lno1zcss9sy46p548rukhu2vt7g0dsy9r00n2jswepsrngjt7w988ac94hpv"
pay = Payment(bolt12)
assert pay.kind() == PaymentKind.LIGHTNING_OFFER
assert pay.lightning_offer() is not None
assert pay.lightning_invoice() is None
# Test lightning_payment() returns a LightningPayment for offers
assert pay.lightning_payment() is not None

# Test Bolt12 offer with schema
pay = Payment(f"lightning:{bolt12}")
assert pay.kind() == PaymentKind.LIGHTNING_OFFER
assert pay.lightning_payment() is not None

# Test LNURL
lnurl = "lnurl1dp68gurn8ghj7ctsdyhxwetewdjhytnxw4hxgtmvde6hymp0wpshj0mswfhk5etrw3ykg0f3xqcs2mcx97"
pay = Payment(lnurl)
assert pay.kind() == PaymentKind.LN_URL
assert pay.lnurl() is not None
# Test lightning_payment() returns a LightningPayment for lnurl
assert pay.lightning_payment() is not None

# Test LNURL with lightning schema
pay = Payment(f"lightning:{lnurl}")
assert pay.kind() == PaymentKind.LN_URL
assert pay.lightning_payment() is not None

# Test lnurlp schema
lnurlp = "lnurlp://geyser.fund/.well-known/lnurlp/citadel"
pay = Payment(lnurlp)
assert pay.kind() == PaymentKind.LN_URL
assert pay.lnurl() is not None

# Test LNURL with email format (lightning:email@domain)
lnurl_email = "citadel@geyser.fund"
pay = Payment(f"lightning:{lnurl_email}")
assert pay.kind() == PaymentKind.LN_URL
assert pay.lnurl() is not None

# Test BIP21 - simple with amount
bip21_simple = "bitcoin:1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa?amount=50"
pay = Payment(bip21_simple)
assert pay.kind() == PaymentKind.BIP21
bip21_obj = pay.bip21()
assert bip21_obj is not None
assert bip21_obj.as_str() == bip21_simple
assert str(bip21_obj.address()) == "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa"
assert bip21_obj.address().is_mainnet()
assert bip21_obj.amount() == 5000000000  # 50 BTC in sats
assert bip21_obj.label() is None
assert bip21_obj.message() is None
assert pay.bitcoin_address() is None  # BIP21 is different from plain address

# Test BIP21 with amount, label, and message
bip21_full = "bitcoin:1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa?amount=0.0001&label=Test&message=Hello%2C%20world!"
pay = Payment(bip21_full)
assert pay.kind() == PaymentKind.BIP21
bip21_obj = pay.bip21()
assert bip21_obj is not None
assert str(bip21_obj.address()) == "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa"
assert bip21_obj.amount() == 10000  # 0.0001 BTC = 10000 sats
assert bip21_obj.label() == "Test"
assert bip21_obj.message() == "Hello, world!"

# Test BIP21 directly constructed
bip21_direct = Bip21("bitcoin:1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa?amount=0.0001&label=Test&message=Hello%2C%20world!")
assert str(bip21_direct.address()) == "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa"
assert bip21_direct.amount() == 10000
assert bip21_direct.label() == "Test"
assert bip21_direct.message() == "Hello, world!"

# Test unified BIP21 with BOLT11 lightning invoice (using segwit address)
unified_bolt11_invoice = "LNBC10U1P3PJ257PP5YZTKWJCZ5FTL5LAXKAV23ZMZEKAW37ZK6KMV80PK4XAEV5QHTZ7QDPDWD3XGER9WD5KWM36YPRX7U3QD36KUCMGYP282ETNV3SHJCQZPGXQYZ5VQSP5USYC4LK9CHSFP53KVCNVQ456GANH60D89REYKDNGSMTJ6YW3NHVQ9QYYSSQJCEWM5CJWZ4A6RFJX77C490YCED6PEMK0UPKXHY89CMM7SCT66K8GNEANWYKZGDRWRFJE69H9U5U0W57RRCSYSAS7GADWMZXC8C6T0SPJAZUP6"
unified_bolt11 = f"bitcoin:BC1QYLH3U67J673H6Y6ALV70M0PL2YZ53TZHVXGG7U?amount=0.00001&label=sbddesign%3A%20For%20lunch%20Tuesday&message=For%20lunch%20Tuesday&lightning={unified_bolt11_invoice}"
pay = Payment(unified_bolt11)
assert pay.kind() == PaymentKind.BIP21
bip21_obj = pay.bip21()
assert bip21_obj is not None
# Segwit addresses are case-insensitive, compare lowercase
assert str(bip21_obj.address()).lower() == "bc1qylh3u67j673h6y6alv70m0pl2yz53tzhvxgg7u"
assert bip21_obj.address().is_mainnet()
assert bip21_obj.amount() == 1000  # 0.00001 BTC = 1000 sats
assert bip21_obj.label() == "sbddesign: For lunch Tuesday"
assert bip21_obj.message() == "For lunch Tuesday"
# Test that the embedded lightning invoice is accessible
lightning_inv = bip21_obj.lightning()
assert lightning_inv is not None

# Test unified BIP21 with BOLT12 offer
bolt12_offer = "lno1zcss9sy46p548rukhu2vt7g0dsy9r00n2jswepsrngjt7w988ac94hpv"
unified_bolt12 = f"bitcoin:BC1QYLH3U67J673H6Y6ALV70M0PL2YZ53TZHVXGG7U?amount=0.00001&label=sbddesign%3A%20For%20lunch%20Tuesday&message=For%20lunch%20Tuesday&lno={bolt12_offer}"
pay = Payment(unified_bolt12)
assert pay.kind() == PaymentKind.BIP21
bip21_obj = pay.bip21()
assert bip21_obj is not None
assert str(bip21_obj.address()).lower() == "bc1qylh3u67j673h6y6alv70m0pl2yz53tzhvxgg7u"
assert bip21_obj.amount() == 1000  # 0.00001 BTC = 1000 sats
assert bip21_obj.label() == "sbddesign: For lunch Tuesday"
assert bip21_obj.message() == "For lunch Tuesday"
# Test that the embedded BOLT12 offer is accessible
offer = bip21_obj.offer()
assert offer is not None
assert offer == bolt12_offer

# Test BIP21 with payjoin parameters
payjoin_bip21 = "bitcoin:12c6DSiU4Rq3P4ZxziKxzrL5LmMBrzjrJX?pj=https://example.com&pjos=0"
pay = Payment(payjoin_bip21)
assert pay.kind() == PaymentKind.BIP21
bip21_obj = pay.bip21()
assert bip21_obj is not None
assert bip21_obj.payjoin() == "https://example.com/"
assert bip21_obj.payjoin_output_substitution() == False
# Test pjos defaults to True when absent
payjoin_bip21_default_pjos = "bitcoin:12c6DSiU4Rq3P4ZxziKxzrL5LmMBrzjrJX?pj=https://example.com"
bip21_obj = Bip21(payjoin_bip21_default_pjos)
assert bip21_obj.payjoin() == "https://example.com/"
assert bip21_obj.payjoin_output_substitution() == True

# Test BIP353
bip353 = "₿matt@mattcorallo.com"
pay = Payment(bip353)
assert pay.kind() == PaymentKind.BIP353
assert pay.bip353() == "matt@mattcorallo.com"  # Without the ₿ prefix

# Test Liquid BIP21
address = "VJLDJCJZja8GZNBkLFAHWSNwuxMrzs1BpX1CAUqvfwgtRtDdVtPFWiQwnYMf76rMamsUgFFJVgf36eag"
asset = "ce091c998b83c78bb71a632313ba3760f1763d9cfcffae02258ffa9865a37bd2"
amount = 10
liquid_bip21 = f"liquidnetwork:{address}?amount={amount}&assetid={asset}"
pay = Payment(liquid_bip21)
assert pay.kind() == PaymentKind.LIQUID_BIP21
bip21_data = pay.liquid_bip21()
assert bip21_data is not None
assert str(bip21_data.address) == address
assert bip21_data.address.network() == Network.mainnet()
assert bip21_data.asset == asset
assert bip21_data.amount == amount
assert pay.liquid_address() is None  # LiquidBip21 is different from plain address

# Test Liquid BIP21 on testnet
address_testnet = "tlq1qq02egjncr8g4qn890mrw3jhgupwqymekv383lwpmsfghn36hac5ptpmeewtnftluqyaraa56ung7wf47crkn5fjuhk422d68m"
liquid_bip21_testnet = f"liquidtestnet:{address_testnet}?amount={amount}&assetid={asset}"
pay = Payment(liquid_bip21_testnet)
assert pay.kind() == PaymentKind.LIQUID_BIP21
bip21_data = pay.liquid_bip21()
assert bip21_data is not None
assert str(bip21_data.address) == address_testnet
assert bip21_data.address.network() == Network.testnet()

print("All payment instructions tests passed!")

