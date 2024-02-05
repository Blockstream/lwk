from lwk_bindings import *


class PythonPersister(ForeignPersister):
  data = []

  def get(self, i):
    try:
      return self.data[i]
    except:
      None

  def push(self, update):
    self.data.append(update)


desc = WolletDescriptor("ct(slip77(ab5824f4477b4ebb00a132adfd8eb0b7935cf24f6ac151add5d1913db374ce92),elwpkh([759db348/84'/1'/0']tpubDCRMaF33e44pcJj534LXVhFbHibPbJ5vuLhSSPFAw57kYURv4tzXFL6LSnd78bkjqdmE3USedkbpXJUPA1tdzKfuYSL7PianceqAhwL2UkA/<0;1>/*))#cch6wrnp");

network = Network.testnet()
assert(str(network) == "LiquidTestnet")

client = network.default_electrum_client()

persister = ForeignPersisterLink(PythonPersister())

w = Wollet.with_custom_persister(network, desc, persister)
update = client.full_scan(w)
w.apply_update(update)
total_txs = len(w.transactions())
assert(total_txs == 11)
w = None

w2 = Wollet.with_custom_persister(network, desc, persister)
assert(total_txs == len(w2.transactions()))
