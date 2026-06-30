package main

import (
	"fmt"
	"log"
	"os"

	"lwk-go/lwk"
)

func main() {
	gitlabCI := os.Getenv("GITLAB_CI")
	ciJobName := os.Getenv("CI_JOB_NAME")
	ciBranchName := os.Getenv("CI_COMMIT_BRANCH")

	if gitlabCI == "true" {
		// We are in a CI job
		if ciBranchName != "master" || ciJobName != "amp0" {
			fmt.Println("Skipping test")
			return
		}
	}

	// ANCHOR: amp0-daily-ops
	// Signer
	mnemonic, err := lwk.NewMnemonic("<mnemonic>")
	mnemonic, err = lwk.NewMnemonic("thrive metal cactus come oval candy medal bounce captain shock permit joke") // ANCHOR: ignore
	if err != nil {
		log.Fatal(err)
	}

	// AMP0 Watch-Only credentials
	username := "<username>"
	password := "<password>"
	username = "userlwk001" // ANCHOR: ignore
	password = "userlwk001" // ANCHOR: ignore
	// AMP ID (optional)
	ampId := ""

	// Create AMP0 context
	network := lwk.NetworkTestnet()

	amp0, err := lwk.NewAmp0(network, username, password, ampId)
	if err != nil {
		log.Fatal(err)
	}

	// Create AMP0 Wollet
	wolletDescriptor, err := amp0.WolletDescriptor()
	if err != nil {
		log.Fatal(err)
	}
	wollet, err := lwk.NewWollet(network, wolletDescriptor, nil)
	if err != nil {
		log.Fatal(err)
	}

	// Get a new address
	addrResult, err := amp0.Address(nil)
	if err != nil {
		log.Fatal(err)
	}
	addr := addrResult.Address()

	// Update the wallet with (new) blockchain data
	url := "https://waterfalls.liquidwebwallet.org/liquidtestnet/api"
	client, err := lwk.NewWaterfallsClient(url, network)
	if err != nil {
		log.Fatal(err)
	}

	lastIndex, err := amp0.LastIndex()
	if err != nil {
		log.Fatal(err)
	}

	update, err := client.FullScanToIndex(wollet, lastIndex)
	if err != nil {
		log.Fatal(err)
	}
	if update != nil {
		if err := wollet.ApplyUpdate(*update); err != nil {
			log.Fatal(err)
		}
	}

	// Get balance
	balance, err := wollet.Balance()
	if err != nil {
		log.Fatal(err)
	}

	lbtc := network.PolicyAsset() // ANCHOR: ignore
	lbtcBalance := balance[lbtc]  // ANCHOR: ignore
	if lbtcBalance < 500 {        // ANCHOR: ignore
		fmt.Printf("Balance is insufficient to make a transaction, send some tLBTC to %s\n", addr) // ANCHOR: ignore
		return                                                                                     // ANCHOR: ignore
	} // ANCHOR: ignore

	// Construct a PSET sending LBTC back to the wallet
	b := network.TxBuilder()
	if err := b.DrainLbtcWallet(); err != nil { // send all to self
		log.Fatal(err)
	}

	amp0pset, err := b.FinishForAmp0(wollet)
	if err != nil {
		log.Fatal(err)
	}

	// User signs the PSET
	signer, err := lwk.NewSigner(mnemonic, network)
	if err != nil {
		log.Fatal(err)
	}

	pset, err := amp0pset.Pset()
	if err != nil {
		log.Fatal(err)
	}

	pset, err = signer.Sign(pset)
	if err != nil {
		log.Fatal(err)
	}

	blindingNonces, err := amp0pset.BlindingNonces()
	if err != nil {
		log.Fatal(err)
	}

	// Reconstruct the Amp0 PSET with the PSET signed by the user
	amp0pset, err = lwk.NewAmp0Pset(pset, blindingNonces)
	if err != nil {
		log.Fatal(err)
	}

	// AMP0 signs
	tx, err := amp0.Sign(amp0pset)
	if err != nil {
		log.Fatal(err)
	}

	// Broadcast the transaction
	txid, err := client.Broadcast(tx)
	if err != nil {
		log.Fatal(err)
	}
	// ANCHOR_END: amp0-daily-ops

	fmt.Println(txid)
}
