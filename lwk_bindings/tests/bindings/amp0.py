from lwk import *

amp0 = Amp0(Network.testnet(), "userleo34567", "userleo34567", "")
assert amp0.last_index() > 20
addr0 = str(amp0.address(1).address())
assert addr0 == 'vjTvpDMQx3EQ2bS3pmmy7RivU3QTjGyyJFJy1Y5basdKmwpW3R4YRdsxFNT7B3bPNmJkgKCRCS63AtjR'
