package main

import (
	"log"

	"lwk-go/lwk"
)

func main() {
	// Start nodes
	node := lwk.NewLwkTestEnv()

	// Create wallet
	mnemonicStr := "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
	mnemonic, err := lwk.NewMnemonic(mnemonicStr)
	if err != nil {
		log.Fatal(err)
	}

	network := node.Network()
	client, err := lwk.ElectrumClientFromUrl(node.ElectrumUrl())
	if err != nil {
		log.Fatal(err)
	}

	signer, err := lwk.NewSigner(mnemonic, network)
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

	// Fund wallet with 2 utxos
	fundedSatoshi := uint64(100000)

	idx0 := uint32(0)
	addr0, err := wollet.Address(&idx0)
	if err != nil {
		log.Fatal(err)
	}
	txid1 := node.SendToAddress(addr0.Address(), fundedSatoshi, nil)
	_, err = wollet.WaitForTx(txid1, client)
	if err != nil {
		log.Fatal(err)
	}

	idx1 := uint32(1)
	addr1, err := wollet.Address(&idx1)
	if err != nil {
		log.Fatal(err)
	}
	txid2 := node.SendToAddress(addr1.Address(), fundedSatoshi, nil)
	_, err = wollet.WaitForTx(txid2, client)
	if err != nil {
		log.Fatal(err)
	}

	// Create and sign a transaction selecting only 1 utxo, without manual coin selection they would be 2
	sentSatoshi := uint64(1000)
	nodeAddress := node.GetNewAddress()

	// ANCHOR: get_utxos
	utxos, err := wollet.Utxos()
	if err != nil {
		log.Fatal(err)
	}
	// ANCHOR_END: get_utxos

	if len(utxos) == 0 {
		panic("expected utxos to be present after funding")
	}

	// ANCHOR: manual_coin_selection
	builder := network.TxBuilder()
	if err := builder.AddLbtcRecipient(nodeAddress, sentSatoshi); err != nil {
		log.Fatal(err)
	}

	if err := builder.SetWalletUtxos([]*lwk.OutPoint{utxos[0].Outpoint()}); err != nil {
		log.Fatal(err)
	}

	unsignedPset, err := builder.Finish(wollet)
	if err != nil {
		log.Fatal(err)
	}

	if len(unsignedPset.Inputs()) != 1 { // ANCHOR: Ignore
		panic("expected exactly 1 input in pset") // ANCHOR: Ignore
	} // ANCHOR: Ignore

	signedPset, err := signer.Sign(unsignedPset)
	if err != nil {
		log.Fatal(err)
	}

	finalizedPset, err := wollet.Finalize(signedPset)
	if err != nil {
		log.Fatal(err)
	}

	tx, err := finalizedPset.ExtractTx()
	if err != nil {
		log.Fatal(err)
	}
	// ANCHOR_END: manual_coin_selection

	// Broadcast the transaction
	txid3, err := client.Broadcast(tx)
	if err != nil {
		log.Fatal(err)
	}
	_, err = wollet.WaitForTx(txid3, client)
	if err != nil {
		log.Fatal(err)
	}
}
