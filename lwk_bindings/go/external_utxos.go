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

	// Wallet 1
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

	// Wallet 2
	externalSigner, err := lwk.SignerRandom(network)
	if err != nil {
		log.Fatal(err)
	}
	extDesc, err := externalSigner.WpkhSlip77Descriptor()
	if err != nil {
		log.Fatal(err)
	}
	externalWollet, err := lwk.NewWollet(network, extDesc, nil)
	if err != nil {
		log.Fatal(err)
	}

	// Fund both wallets
	satsAsset := uint64(10)
	satsLbtc := uint64(100000)

	asset := node.IssueAsset(satsAsset)

	idx0 := uint32(0)
	addrResult1, err := wollet.Address(&idx0)
	if err != nil {
		log.Fatal(err)
	}
	wolletAddr := addrResult1.Address()

	addrResult2, err := externalWollet.Address(&idx0)
	if err != nil {
		log.Fatal(err)
	}
	externalWolletAddr := addrResult2.Address()

	txid1 := node.SendToAddress(wolletAddr, satsAsset, &asset)
	txid2 := node.SendToAddress(externalWolletAddr, satsLbtc, nil)

	_, err = wollet.WaitForTx(txid1, client)
	if err != nil {
		log.Fatal(err)
	}
	_, err = externalWollet.WaitForTx(txid2, client)
	if err != nil {
		log.Fatal(err)
	}

	// Get the utxo from external_wollet
	// ANCHOR: external_utxo_create
	extUtxos, err := externalWollet.Utxos()
	if err != nil {
		log.Fatal(err)
	}
	if len(extUtxos) == 0 {
		panic("expected external utxos")
	}
	extUtxo := extUtxos[0]

	extTxs, err := externalWollet.Transactions()
	if err != nil {
		log.Fatal(err)
	}

	matchingTx := extTxs[0].Tx()
	maxWeightToSatisfy, err := externalWollet.MaxWeightToSatisfy()
	if err != nil {
		log.Fatal(err)
	}

	isSegwit, err := externalWollet.IsSegwit()
	if err != nil {
		log.Fatal(err)
	}

	externalUtxo, err := lwk.NewExternalUtxo(
		extUtxo.Outpoint().Vout(),
		matchingTx,
		extUtxo.Unblinded(),
		maxWeightToSatisfy,
		isSegwit,
	)
	if err != nil {
		log.Fatal(err)
	}
	// ANCHOR_END: external_utxo_create

	nodeAddr := node.GetNewAddress()

	// Create speding tx sending all to the node
	// ANCHOR: external_utxo_add
	builder := network.TxBuilder()

	// Add external UTXO (LBTC)
	if err := builder.AddExternalUtxos([]*lwk.ExternalUtxo{externalUtxo}); err != nil {
		log.Fatal(err)
	}

	// Send asset to the node (funded by wollet's UTXOs)
	if err := builder.AddRecipient(nodeAddr, 1, asset); err != nil {
		log.Fatal(err)
	}

	// Send LBTC back to external wollet
	if err := builder.DrainLbtcWallet(); err != nil {
		log.Fatal(err)
	}
	if err := builder.DrainLbtcTo(externalWolletAddr); err != nil {
		log.Fatal(err)
	}

	pset, err := builder.Finish(wollet)
	if err != nil {
		log.Fatal(err)
	}
	// ANCHOR_END: external_utxo_add

	pset, err = signer.Sign(pset)
	if err != nil {
		log.Fatal(err)
	}

	// Add the details for Wallet 2 so that Signer 2 is able to sign
	// ANCHOR: external_utxo_sign
	pset, err = externalWollet.AddDetails(pset)
	if err != nil {
		log.Fatal(err)
	}
	pset, err = externalSigner.Sign(pset)
	if err != nil {
		log.Fatal(err)
	}
	// ANCHOR_END: external_utxo_sign

	tx, err := pset.Finalize()
	if err != nil {
		log.Fatal(err)
	}

	txid, err := client.Broadcast(tx)
	if err != nil {
		log.Fatal(err)
	}

	_, err = wollet.WaitForTx(txid, client)
	if err != nil {
		log.Fatal(err)
	}
	_, err = externalWollet.WaitForTx(txid, client)
	if err != nil {
		log.Fatal(err)
	}

	bal1, _ := wollet.Balance()
	bal2, _ := externalWollet.Balance()

	if bal1[policyAsset] != 0 {
		panic("expected 0 lbtc in wollet")
	}
	if bal1[asset] != uint64(satsAsset-1) {
		panic("expected satsAsset - 1 in wollet")
	}

	if bal2[policyAsset] <= 0 {
		panic("expected > 0 lbtc in external wollet")
	}
	if bal2[asset] != 0 {
		panic("expected 0 custom asset in external wollet")
	}

	txs1, _ := wollet.Transactions()
	if len(txs1) != 2 {
		panic("expected 2 txs in wollet")
	}

	txs2, _ := externalWollet.Transactions()
	if len(txs2) != 2 {
		panic("expected 2 txs in external wollet")
	}
}
