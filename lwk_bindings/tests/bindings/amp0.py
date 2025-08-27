from lwk import *

amp0 = Amp0(Network.mainnet(), "userleo456", "userleo456", "")
assert amp0.last_index() > 20
addr0 = str(amp0.address(0).address())
assert addr0 == 'VJLHASNov3RqftSBp6QwmfsS5XEfNKnpfjWqYpz8im7rf4TGL1nXUGXLjVjhEX1fz4BXVV4Zyx79BAi6'
