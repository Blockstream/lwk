package main

import (
	"log"

	"lwk-go/lwk"
)

func main() {
	// Start nodes
	node := lwk.NewLwkTestEnv()

	network := node.Network()
	policyAsset := network.PolicyAsset()
	client, err := lwk.ElectrumClientFromUrl(node.ElectrumUrl())
	if err != nil {
		log.Fatal(err)
	}

	signer, err := lwk.SignerRandom(network)
	if err != nil {
		log.Fatal(err)
	}
	desc, err := signer.WpkhSlip77Descriptor()
	if err != nil {
		log.Fatal(err)
	}
	wollet, err := lwk.NewWollet(network, desc, nil)
	if err != nil {
		log.Fatal(err)
	}

	// Fund both wallets (script says both, but only sets up one)
	sats := uint64(100_000)

	idx := uint32(0)
	addrResult, err := wollet.Address(&idx)
	if err != nil {
		log.Fatal(err)
	}

	txid := node.SendToAddress(addrResult.Address(), sats, nil)
	_, err = wollet.WaitForTx(txid, client)
	if err != nil {
		log.Fatal(err)
	}

	nodeAddr := node.GetNewAddress()

	// ANCHOR: drain_lbtc_wallet
	// Create a PSET sending all LBTC to the node address
	builder := network.TxBuilder()
	if err := builder.DrainLbtcWallet(); err != nil {
		log.Fatal(err)
	}
	if err := builder.DrainLbtcTo(nodeAddr); err != nil {
		log.Fatal(err)
	}
	pset, err := builder.Finish(wollet)
	if err != nil {
		log.Fatal(err)
	}
	// ANCHOR_END: drain_lbtc_wallet

	pset, err = signer.Sign(pset)
	if err != nil {
		log.Fatal(err)
	}

	tx, err := pset.Finalize()
	if err != nil {
		log.Fatal(err)
	}

	txid2, err := client.Broadcast(tx)
	if err != nil {
		log.Fatal(err)
	}

	_, err = wollet.WaitForTx(txid2, client)
	if err != nil {
		log.Fatal(err)
	}

	balance, err := wollet.Balance()
	if err != nil {
		log.Fatal(err)
	}
	if balance[policyAsset] != 0 {
		panic("expected 0 lbtc in wollet after drain")
	}

	txs, err := wollet.Transactions()
	if err != nil {
		log.Fatal(err)
	}
	if len(txs) != 2 {
		panic("expected exactly 2 transactions in wollet")
	}
}
