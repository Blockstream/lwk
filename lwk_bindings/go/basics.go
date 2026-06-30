package main

import (
	"fmt"
	"log"
	"lwk-go/lwk"
)

func main() {
	node := lwk.NewLwkTestEnv()

	// ANCHOR: generate-signer
	mnemonic, err := lwk.MnemonicFromRandom(12)
	if err != nil {
		log.Fatal(err)
	}
	network := lwk.NetworkTestnet()
	network = lwk.NetworkRegtestDefault() // ANCHOR: ignore
	signer, err := lwk.NewSigner(mnemonic, network)
	if err != nil {
		log.Fatal(err)
	}
	// ANCHOR_END: generate-signer

	// ANCHOR: get-xpub
	xpub, err := signer.KeyoriginXpub(lwk.BipNewBip84())
	if err != nil {
		log.Fatal(err)
	}
	// ANCHOR_END: get-xpub
	fmt.Println("Xpub:", xpub)

	// ANCHOR: wollet
	desc, err := signer.WpkhSlip77Descriptor()
	if err != nil {
		log.Fatal(err)
	}
	wollet, err := lwk.NewWollet(network, desc, nil)
	if err != nil {
		log.Fatal(err)
	}
	// ANCHOR_END: wollet

	// ANCHOR: address
	addr, err := wollet.Address(nil)
	if err != nil {
		log.Fatal(err)
	}
	// ANCHOR_END: address

	elClient, err := lwk.ElectrumClientFromUrl(node.ElectrumUrl())
	if err != nil {
		panic(err)
	}
	sats := uint64(100000)
	txid := node.SendToAddress(addr.Address(), sats, nil)
	walletTx, err := wollet.WaitForTx(txid, elClient)
	if err != nil {
		panic(err)
	}
	policyAsset := network.PolicyAsset()
	if walletTx.Balance()[policyAsset] != int64(sats) {
		panic("unexpected balance")
	}

	// ANCHOR: txs
	txs, err := wollet.Transactions()
	if err != nil {
		log.Fatal(err)
	}
	balance, err := wollet.Balance()
	if err != nil {
		log.Fatal(err)
	}
	// ANCHOR_END: txs
	fmt.Printf("Transactions: %d, balance: %d sats\n", len(txs), balance[policyAsset])

	// ANCHOR: electrum_client
	// Create electrum client with custom URL
	electrumClient, err := lwk.NewElectrumClient("blockstream.info:995", true, true)
	if err != nil {
		log.Fatal(err)
	}

	// Or use the default electrum client for the network
	defaultClient, err := lwk.NetworkMainnet().DefaultElectrumClient()
	if err != nil {
		log.Fatal(err)
	}
	// ANCHOR_END: electrum_client
	_ = electrumClient
	_ = defaultClient

	// ANCHOR: esplora_client
	esploraClient, err := lwk.NewEsploraClient("https://blockstream.info/liquid/api", lwk.NetworkMainnet())
	if err != nil {
		log.Fatal(err)
	}
	// ANCHOR_END: esplora_client
	_ = esploraClient

	// ANCHOR: waterfalls_client
	waterfallsClient, err := lwk.NewWaterfallsClient("https://waterfalls.liquidwebwallet.org/liquid/api", lwk.NetworkMainnet())
	if err != nil {
		log.Fatal(err)
	}
	// ANCHOR_END: waterfalls_client
	_ = waterfallsClient

	// ANCHOR: client
	url := "https://blockstream.info/liquidtestnet/api"
	client, err := lwk.NewEsploraClient(url, network)
	if err != nil {
		log.Fatal(err)
	}
	client, err = lwk.NewEsploraClient(node.EsploraUrl(), network) // ANCHOR: ignore
	if err != nil {                                                // ANCHOR: ignore
		panic(err) // ANCHOR: ignore
	} // ANCHOR: ignore

	update, err := client.FullScan(wollet)
	if err != nil {
		log.Fatal(err)
	}
	if update != nil {
		if err := wollet.ApplyUpdate(*update); err != nil {
			log.Fatal(err)
		}
	}
	// ANCHOR_END: client

	nodeAddress := node.GetNewAddress()
	sendSats := uint64(1000)
	lbtc := policyAsset

	// ANCHOR: tx
	b := network.TxBuilder()
	if err := b.AddRecipient(nodeAddress, sendSats, lbtc); err != nil {
		log.Fatal(err)
	}
	pset, err := b.Finish(wollet)
	if err != nil {
		log.Fatal(err)
	}
	// ANCHOR_END: tx

	// ANCHOR: pset-details
	details, err := wollet.PsetDetails(pset)
	if err != nil {
		log.Fatal(err)
	}
	// ANCHOR_END: pset-details
	_ = details

	// ANCHOR: sign
	pset, err = signer.Sign(pset)
	if err != nil {
		log.Fatal(err)
	}
	// ANCHOR_END: sign

	// ANCHOR: broadcast
	tx, err := pset.Finalize()
	if err != nil {
		log.Fatal(err)
	}
	txid, err = client.Broadcast(tx)
	if err != nil {
		log.Fatal(err)
	}

	// optional
	if err := wollet.ApplyTransaction(tx); err != nil {
		log.Fatal(err)
	}
	// ANCHOR_END: broadcast
	fmt.Println("Broadcast txid:", txid)
}
