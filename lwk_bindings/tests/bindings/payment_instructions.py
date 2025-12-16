from lwk import Payment, PaymentKind

# Test Bitcoin address (no schema)
bitcoin_address = "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa"
cat = Payment(bitcoin_address)
assert cat.kind() == PaymentKind.BITCOIN_ADDRESS
assert cat.bitcoin_address() == bitcoin_address
assert cat.liquid_address() is None
# Non-lightning categories should return None for lightning_payment()
assert cat.lightning_payment() is None

# Test Bitcoin address with schema
cat = Payment(f"bitcoin:{bitcoin_address}")
assert cat.kind() == PaymentKind.BITCOIN_ADDRESS
assert cat.bitcoin_address() == bitcoin_address

# Test Bitcoin address with uppercase schema
cat = Payment(f"BITCOIN:{bitcoin_address}")
assert cat.kind() == PaymentKind.BITCOIN_ADDRESS
assert cat.bitcoin_address() == bitcoin_address

# Test Bitcoin segwit address
bitcoin_segwit = "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq"
cat = Payment(bitcoin_segwit)
assert cat.kind() == PaymentKind.BITCOIN_ADDRESS
assert cat.bitcoin_address() == bitcoin_segwit

# Test Liquid address (no schema)
liquid_address = "lq1qqduq2l8maf4580wle4hevmk62xqqw3quckshkt2rex3ylw83824y4g96xl0uugdz4qks5v7w4pdpvztyy5kw7r7e56jcwm0p0"
cat = Payment(liquid_address)
assert cat.kind() == PaymentKind.LIQUID_ADDRESS
assert cat.liquid_address() is not None
assert str(cat.liquid_address()) == liquid_address
assert cat.bitcoin_address() is None

# Test Liquid address with schema
cat = Payment(f"liquidnetwork:{liquid_address}")
assert cat.kind() == PaymentKind.LIQUID_ADDRESS
assert str(cat.liquid_address()) == liquid_address

# Test Lightning invoice
lightning_invoice = "lnbc23230n1p5sxxunsp5tep5yrw63cy3tk74j3hpzqzhhzwe806wk0apjfsfn5x9wmpkzkdspp5z4f40v2whks0aj3kx4zuwrrem094pna4ehutev2p63djtff02a2sdquf35kw6r5de5kueeqwpshjmt9de6qxqyp2xqcqzxrrzjqf6rgswuygn5qr0p5dt2mvklrrcz6yy8pnzqr3eq962tqwprpfrzkzzxeyqq28qqqqqqqqqqqqqqq9gq2yrzjqtnpp8ds33zeg5a6cumptreev23g7pwlp39cvcz8jeuurayvrmvdsrw9ysqqq9gqqqqqqqqpqqqqq9sq2g9qyysgqqufsg7s6qcmfmjxvkf0ulupufr0yfqeajnv3mvtyqzz2rfwre2796rnkzsw44lw3nja5frg4w4m59xqlwwu774h4f79ysm05uugckugqdf84yl"
cat = Payment(lightning_invoice)
assert cat.kind() == PaymentKind.LIGHTNING_INVOICE
assert cat.lightning_invoice() is not None
assert cat.lightning_offer() is None
# Test lightning_payment() returns a LightningPayment for invoices
lp = cat.lightning_payment()
assert lp is not None

# Test Lightning invoice with schema
cat = Payment(f"lightning:{lightning_invoice}")
assert cat.kind() == PaymentKind.LIGHTNING_INVOICE
assert cat.lightning_payment() is not None

# Test Bolt12 offer
bolt12 = "lno1zcss9sy46p548rukhu2vt7g0dsy9r00n2jswepsrngjt7w988ac94hpv"
cat = Payment(bolt12)
assert cat.kind() == PaymentKind.LIGHTNING_OFFER
assert cat.lightning_offer() is not None
assert cat.lightning_invoice() is None
# Test lightning_payment() returns a LightningPayment for offers
assert cat.lightning_payment() is not None

# Test Bolt12 offer with schema
cat = Payment(f"lightning:{bolt12}")
assert cat.kind() == PaymentKind.LIGHTNING_OFFER
assert cat.lightning_payment() is not None

# Test LNURL
lnurl = "lnurl1dp68gurn8ghj7ctsdyhxwetewdjhytnxw4hxgtmvde6hymp0wpshj0mswfhk5etrw3ykg0f3xqcs2mcx97"
cat = Payment(lnurl)
assert cat.kind() == PaymentKind.LN_URL
assert cat.lnurl() is not None
# Test lightning_payment() returns a LightningPayment for lnurl
assert cat.lightning_payment() is not None

# Test LNURL with lightning schema
cat = Payment(f"lightning:{lnurl}")
assert cat.kind() == PaymentKind.LN_URL
assert cat.lightning_payment() is not None

# Test lnurlp schema
lnurlp = "lnurlp://geyser.fund/.well-known/lnurlp/citadel"
cat = Payment(lnurlp)
assert cat.kind() == PaymentKind.LN_URL
assert cat.lnurl() is not None

# Test BIP21
bip21 = "bitcoin:1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa?amount=50"
cat = Payment(bip21)
assert cat.kind() == PaymentKind.BIP21
assert cat.bip21() == bip21
assert cat.bitcoin_address() is None  # BIP21 is different from plain address

# Test BIP353
bip353 = "₿matt@mattcorallo.com"
cat = Payment(bip353)
assert cat.kind() == PaymentKind.BIP353
assert cat.bip353() == "matt@mattcorallo.com"  # Without the ₿ prefix

# Test Liquid BIP21
address = "VJLDJCJZja8GZNBkLFAHWSNwuxMrzs1BpX1CAUqvfwgtRtDdVtPFWiQwnYMf76rMamsUgFFJVgf36eag"
asset = "ce091c998b83c78bb71a632313ba3760f1763d9cfcffae02258ffa9865a37bd2"
amount = 10
liquid_bip21 = f"liquidnetwork:{address}?amount={amount}&assetid={asset}"
cat = Payment(liquid_bip21)
assert cat.kind() == PaymentKind.LIQUID_BIP21
bip21_data = cat.liquid_bip21()
assert bip21_data is not None
assert str(bip21_data.address) == address
assert bip21_data.asset == asset
assert bip21_data.amount == amount
assert cat.liquid_address() is None  # LiquidBip21 is different from plain address

# Test Liquid BIP21 on testnet
address_testnet = "tlq1qq02egjncr8g4qn890mrw3jhgupwqymekv383lwpmsfghn36hac5ptpmeewtnftluqyaraa56ung7wf47crkn5fjuhk422d68m"
liquid_bip21_testnet = f"liquidtestnet:{address_testnet}?amount={amount}&assetid={asset}"
cat = Payment(liquid_bip21_testnet)
assert cat.kind() == PaymentKind.LIQUID_BIP21
bip21_data = cat.liquid_bip21()
assert bip21_data is not None
assert str(bip21_data.address) == address_testnet

print("All payment instructions tests passed!")

