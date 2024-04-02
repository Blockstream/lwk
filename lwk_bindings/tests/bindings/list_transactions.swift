import lwk

let mnemonic = try Mnemonic(s: "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about")
let network = Network.testnet()

assert("\(network)" == "LiquidTestnet", "wrong network")

let client = try network.defaultElectrumClient()

let signer = try Signer(mnemonic: mnemonic, network: network)
let desc = try signer.wpkhSlip77Descriptor()

assert("\(desc)" == "ct(slip77(9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023),elwpkh([73c5da0a/84'/1'/0']tpubDC8msFGeGuwnKG9Upg7DM2b4DaRqg3CUZa5g8v2SRQ6K4NSkxUgd7HsL2XVWbVm39yBA4LAxysQAm397zwQSQoQgewGiYZqrA9DsP4zbQ1M/<0;1>/*))#2e4n992d", "wrong desc")

let w = try Wollet(network: network, descriptor: desc, datadir: nil)

let update = try client.fullScan(wollet: w)
try w.applyUpdate(update: update!)

let txs = try w.transactions()
assert(!txs.isEmpty, "no transactions")
