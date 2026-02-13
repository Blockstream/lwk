from lwk import *


class PythonStore(ForeignStore):
  data = {}

  def get(self, key):
    return self.data.get(key)

  def put(self, key, value):
    self.data[key] = value

  def remove(self, key):
    self.data.pop(key, None)


desc = WolletDescriptor("ct(slip77(ab5824f4477b4ebb00a132adfd8eb0b7935cf24f6ac151add5d1913db374ce92),elwpkh([759db348/84'/1'/0']tpubDCRMaF33e44pcJj534LXVhFbHibPbJ5vuLhSSPFAw57kYURv4tzXFL6LSnd78bkjqdmE3USedkbpXJUPA1tdzKfuYSL7PianceqAhwL2UkA/<0;1>/*))#cch6wrnp");

network = Network.testnet()
assert(str(network) == "LiquidTestnet")

client = network.default_electrum_client()

store = ForeignStoreLink(PythonStore())

wollet = Wollet.with_custom_store(network, desc, store)
update = client.full_scan(wollet)
wollet.apply_update(update)
total_txs = len(wollet.transactions())
assert(total_txs >= 11)
wollet = None

w2 = Wollet.with_custom_store(network, desc, store)
assert(total_txs == len(w2.transactions()))
