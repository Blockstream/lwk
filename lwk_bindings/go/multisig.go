package main

import (
	"fmt"
	"log"

	"lwk-go/lwk"
)

func main() {
	node := lwk.NewLwkTestEnv()

	// ANCHOR: multisig-setup
	network := lwk.NetworkTestnet()
	network = node.Network() // ANCHOR: ignore

	// Derivation for multisig
	bip := lwk.BipNewBip87()

	// Alice creates their signer and gets the xpub
	signerA, err := lwk.SignerRandom(network)
	if err != nil {
		log.Fatal(err)
	}
	xpubA, err := signerA.KeyoriginXpub(bip)
	if err != nil {
		log.Fatal(err)
	}

	// Bob creates their signer and gets the xpub
	signerB, err := lwk.SignerRandom(network)
	if err != nil {
		log.Fatal(err)
	}
	xpubB, err := signerB.KeyoriginXpub(bip)
	if err != nil {
		log.Fatal(err)
	}

	// Carol, who acts as a coordinator, creates their signer and gets the xpub
	signerC, err := lwk.SignerRandom(network)
	if err != nil {
		log.Fatal(err)
	}
	xpubC, err := signerC.KeyoriginXpub(bip)
	if err != nil {
		log.Fatal(err)
	}

	// Carol generates a random SLIP77 descriptor blinding key
	slip77RandKey := "<random-64-hex-chars>"
	slip77RandKey = "1111111111111111111111111111111111111111111111111111111111111111" // ANCHOR: ignore
	descBlindingKey := fmt.Sprintf("slip77(%s)", slip77RandKey)

	// Carol uses the collected xpubs and the descriptor blinding key to create
	// the 2of3 descriptor
	threshold := 2
	descStr := fmt.Sprintf("ct(%s,elwsh(multi(%d,%s/<0;1>/*,%s/<0;1>/*,%s/<0;1>/*)))", descBlindingKey, threshold, xpubA, xpubB, xpubC)

	// Validate the descriptor string
	wd, err := lwk.NewWolletDescriptor(descStr)
	if err != nil {
		log.Fatal(err)
	}
	// ANCHOR_END: multisig-setup

	// ANCHOR: multisig-receive
	// Carol creates the wollet
	wolletC, err := lwk.NewWollet(network, wd, nil)
	if err != nil {
		log.Fatal(err)
	}

	// With the wollet, Carol can obtain addresses, transactions and balance
	addr, err := wolletC.Address(nil)
	if err != nil {
		log.Fatal(err)
	}
	txs, err := wolletC.Transactions()
	if err != nil {
		log.Fatal(err)
	}
	balance, err := wolletC.Balance()
	if err != nil {
		log.Fatal(err)
	}

	_ = addr    // ANCHOR: ignore
	_ = txs     // ANCHOR: ignore
	_ = balance // ANCHOR: ignore

	// Update the wollet state
	url := "https://blockstream.info/liquidtestnet/api"
	client, err := lwk.NewEsploraClient(url, network)
	if err != nil {
		log.Fatal(err)
	}

	client, err = lwk.NewEsploraClient(node.EsploraUrl(), network) // ANCHOR: ignore
	if err != nil {                                                // ANCHOR: ignore
		log.Fatal(err) // ANCHOR: ignore
	} // ANCHOR: ignore

	update, err := client.FullScan(wolletC)
	if err != nil {
		log.Fatal(err)
	}
	if update != nil {
		if err := wolletC.ApplyUpdate(*update); err != nil {
			log.Fatal(err)
		}
	}
	// ANCHOR_END: multisig-receive

	// Receive some funds
	addr0, err := wolletC.Address(nil)
	if err != nil {
		log.Fatal(err)
	}

	elClient, err := lwk.ElectrumClientFromUrl(node.ElectrumUrl())
	if err != nil {
		panic(err)
	}

	txid := node.SendToAddress(addr0.Address(), 10000, nil)
	_, err = wolletC.WaitForTx(txid, elClient)
	if err != nil {
		log.Fatal(err)
	}

	// ANCHOR: multisig-send
	// Carol creates a transaction send few sats to a certain address
	address, err := lwk.NewAddress("<address>")
	address = node.GetNewAddress() // ANCHOR: ignore
	err = nil                      // ANCHOR: ignore
	if err != nil {
		log.Fatal(err)
	}
	sendSats := uint64(1000)
	lbtc := network.PolicyAsset()

	builder := network.TxBuilder()
	if err := builder.AddRecipient(address, sendSats, lbtc); err != nil {
		log.Fatal(err)
	}
	pset, err := builder.Finish(wolletC)
	if err != nil {
		log.Fatal(err)
	}

	pset, err = signerC.Sign(pset)
	if err != nil {
		log.Fatal(err)
	}

	// Carol sends the PSET to Bob
	// Bob wants to analyze the PSET before signing, thus he creates a wollet
	wdB, err := lwk.NewWolletDescriptor(descStr)
	if err != nil {
		log.Fatal(err)
	}
	wolletB, err := lwk.NewWollet(network, wdB, nil)
	if err != nil {
		log.Fatal(err)
	}

	updateB, err := client.FullScan(wolletB)
	if err != nil {
		log.Fatal(err)
	}
	if updateB != nil {
		if err := wolletB.ApplyUpdate(*updateB); err != nil {
			log.Fatal(err)
		}
	}

	// Then Bob uses the wollet to analyze the PSET
	details, err := wolletB.PsetDetails(pset)
	if err != nil {
		log.Fatal(err)
	}

	// PSET has a reasonable fee
	if details.Balance().FeesIn(lbtc) >= 100 {
		panic("fee too high")
	}

	// PSET has a signature from Carol
	fingerprintsHas := details.FingerprintsHas()
	if len(fingerprintsHas) != 1 {
		panic("unexpected fingerprints_has len")
	}

	fpC, err := signerC.Fingerprint()
	if err != nil {
		log.Fatal(err)
	}
	hasC := false
	for _, fp := range fingerprintsHas {
		if fp == fpC {
			hasC = true
			break
		}
	}
	if !hasC {
		panic("missing signer_c fingerprint")
	}

	// PSET needs a signature from either Bob or Carol
	fingerprintsMissing := details.FingerprintsMissing()
	if len(fingerprintsMissing) != 2 {
		panic("unexpected fingerprints_missing len")
	}
	fpA, err := signerA.Fingerprint()
	if err != nil {
		log.Fatal(err)
	}
	fpB, err := signerB.Fingerprint()
	if err != nil {
		log.Fatal(err)
	}

	hasMissingA := false
	hasMissingB := false
	for _, fp := range fingerprintsMissing {
		if fp == fpA {
			hasMissingA = true
		}
		if fp == fpB {
			hasMissingB = true
		}
	}
	if !hasMissingA || !hasMissingB {
		panic("missing signer_a or signer_b fingerprint")
	}

	// PSET has a single recipient, with data matching what was specified above
	recipients := details.Balance().Recipients()
	if len(recipients) != 1 {
		panic("unexpected recipients len")
	}
	recipient := recipients[0]
	if fmt.Sprintf("%v", *recipient.Address()) != fmt.Sprintf("%v", address) {
		panic("unexpected recipient address")
	}
	if *recipient.Asset() != lbtc {
		panic("unexpected recipient asset")
	}
	if *recipient.Value() != sendSats {
		panic("unexpected recipient value")
	}

	// Bob is satisified with the PSET and signs it
	pset, err = signerB.Sign(pset)
	if err != nil {
		log.Fatal(err)
	}

	// Bob sends the PSET back to Carol
	// Carol checks that the PSET has enough signatures
	detailsC, err := wolletC.PsetDetails(pset)
	if err != nil {
		log.Fatal(err)
	}
	if len(detailsC.FingerprintsHas()) != 2 {
		panic("unexpected fingerprints_has len")
	}

	// Carol finalizes the PSET and broadcast the transaction
	tx, err := pset.Finalize()
	if err != nil {
		log.Fatal(err)
	}
	txid, err = client.Broadcast(tx)
	if err != nil {
		log.Fatal(err)
	}
	// ANCHOR_END: multisig-send

	fmt.Println("Broadcast multisig txid:", txid)
}
