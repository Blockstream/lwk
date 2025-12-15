from lwk import PaymentCategory, PaymentCategoryKind

# Test Bitcoin address (no schema)
bitcoin_address = "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa"
cat = PaymentCategory(bitcoin_address)
assert cat.kind() == PaymentCategoryKind.BITCOIN_ADDRESS
assert cat.bitcoin_address() == bitcoin_address
assert cat.liquid_address() is None

# Test Bitcoin address with schema
cat = PaymentCategory(f"bitcoin:{bitcoin_address}")
assert cat.kind() == PaymentCategoryKind.BITCOIN_ADDRESS
assert cat.bitcoin_address() == bitcoin_address

# Test Bitcoin segwit address
bitcoin_segwit = "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq"
cat = PaymentCategory(bitcoin_segwit)
assert cat.kind() == PaymentCategoryKind.BITCOIN_ADDRESS
assert cat.bitcoin_address() == bitcoin_segwit

# Test Liquid address (no schema)
liquid_address = "lq1qqduq2l8maf4580wle4hevmk62xqqw3quckshkt2rex3ylw83824y4g96xl0uugdz4qks5v7w4pdpvztyy5kw7r7e56jcwm0p0"
cat = PaymentCategory(liquid_address)
assert cat.kind() == PaymentCategoryKind.LIQUID_ADDRESS
assert cat.liquid_address() is not None
assert str(cat.liquid_address()) == liquid_address
assert cat.bitcoin_address() is None

# Test Liquid address with schema
cat = PaymentCategory(f"liquidnetwork:{liquid_address}")
assert cat.kind() == PaymentCategoryKind.LIQUID_ADDRESS
assert str(cat.liquid_address()) == liquid_address

# Test Lightning invoice
lightning_invoice = "lnbc23230n1p5sxxunsp5tep5yrw63cy3tk74j3hpzqzhhzwe806wk0apjfsfn5x9wmpkzkdspp5z4f40v2whks0aj3kx4zuwrrem094pna4ehutev2p63djtff02a2sdquf35kw6r5de5kueeqwpshjmt9de6qxqyp2xqcqzxrrzjqf6rgswuygn5qr0p5dt2mvklrrcz6yy8pnzqr3eq962tqwprpfrzkzzxeyqq28qqqqqqqqqqqqqqq9gq2yrzjqtnpp8ds33zeg5a6cumptreev23g7pwlp39cvcz8jeuurayvrmvdsrw9ysqqq9gqqqqqqqqpqqqqq9sq2g9qyysgqqufsg7s6qcmfmjxvkf0ulupufr0yfqeajnv3mvtyqzz2rfwre2796rnkzsw44lw3nja5frg4w4m59xqlwwu774h4f79ysm05uugckugqdf84yl"
cat = PaymentCategory(lightning_invoice)
assert cat.kind() == PaymentCategoryKind.LIGHTNING_INVOICE
assert cat.lightning_invoice() is not None
assert cat.lightning_offer() is None

# Test Lightning invoice with schema
cat = PaymentCategory(f"lightning:{lightning_invoice}")
assert cat.kind() == PaymentCategoryKind.LIGHTNING_INVOICE

# Test Bolt12 offer
bolt12 = "lno1zcss9sy46p548rukhu2vt7g0dsy9r00n2jswepsrngjt7w988ac94hpv"
cat = PaymentCategory(bolt12)
assert cat.kind() == PaymentCategoryKind.LIGHTNING_OFFER
assert cat.lightning_offer() is not None
assert cat.lightning_invoice() is None

# Test Bolt12 offer with schema
cat = PaymentCategory(f"lightning:{bolt12}")
assert cat.kind() == PaymentCategoryKind.LIGHTNING_OFFER

# Test LNURL
lnurl = "lnurl1dp68gurn8ghj7ctsdyhxwetewdjhytnxw4hxgtmvde6hymp0wpshj0mswfhk5etrw3ykg0f3xqcs2mcx97"
cat = PaymentCategory(lnurl)
assert cat.kind() == PaymentCategoryKind.LN_URL
assert cat.lnurl() is not None

# Test LNURL with lightning schema
cat = PaymentCategory(f"lightning:{lnurl}")
assert cat.kind() == PaymentCategoryKind.LN_URL

# Test lnurlp schema
lnurlp = "lnurlp://geyser.fund/.well-known/lnurlp/citadel"
cat = PaymentCategory(lnurlp)
assert cat.kind() == PaymentCategoryKind.LN_URL
assert cat.lnurl() is not None

# Test BIP21
bip21 = "bitcoin:1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa?amount=50"
cat = PaymentCategory(bip21)
assert cat.kind() == PaymentCategoryKind.BIP21
assert cat.bip21() == bip21
assert cat.bitcoin_address() is None  # BIP21 is different from plain address

# Test BIP353
bip353 = "₿matt@mattcorallo.com"
cat = PaymentCategory(bip353)
assert cat.kind() == PaymentCategoryKind.BIP353
assert cat.bip353() == "matt@mattcorallo.com"  # Without the ₿ prefix

# Test Liquid BIP21
address = "VJLDJCJZja8GZNBkLFAHWSNwuxMrzs1BpX1CAUqvfwgtRtDdVtPFWiQwnYMf76rMamsUgFFJVgf36eag"
asset = "ce091c998b83c78bb71a632313ba3760f1763d9cfcffae02258ffa9865a37bd2"
amount = 10
liquid_bip21 = f"liquidnetwork:{address}?amount={amount}&assetid={asset}"
cat = PaymentCategory(liquid_bip21)
assert cat.kind() == PaymentCategoryKind.LIQUID_BIP21
bip21_data = cat.liquid_bip21()
assert bip21_data is not None
assert str(bip21_data.address) == address
assert bip21_data.asset == asset
assert bip21_data.amount == amount
assert cat.liquid_address() is None  # LiquidBip21 is different from plain address

# Test Liquid BIP21 on testnet
address_testnet = "tlq1qq02egjncr8g4qn890mrw3jhgupwqymekv383lwpmsfghn36hac5ptpmeewtnftluqyaraa56ung7wf47crkn5fjuhk422d68m"
liquid_bip21_testnet = f"liquidtestnet:{address_testnet}?amount={amount}&assetid={asset}"
cat = PaymentCategory(liquid_bip21_testnet)
assert cat.kind() == PaymentCategoryKind.LIQUID_BIP21
bip21_data = cat.liquid_bip21()
assert bip21_data is not None
assert str(bip21_data.address) == address_testnet

print("All payment instructions tests passed!")

