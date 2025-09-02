from lwk import *

amp0 = Amp0(Network.mainnet(), "userleo456", "userleo456", "")
assert amp0.last_index() > 20
addr0 = str(amp0.address(1).address())
assert addr0 == 'VJL5Z9HekG84akxnGnvrtqB3t2Vg9mTP9k5k6FRNHuJhvzayGugagPZBt4omtWhg1VoeqwH2TDSsaW4w'
