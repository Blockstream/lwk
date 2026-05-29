package main

import (
	"log"
	"lwk-go/lwk"
)

func main() {
	node := lwk.NewLwkTestEnv()

	network := node.Network()

	mnemonic, err := lwk.MnemonicFromRandom(12)
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

	primaryUrl := node.ElectrumUrl()
	fallbackUrl := node.ElectrumUrl() // In practice, this would be a different server

	// ANCHOR: fallback_client
	client, err := lwk.ElectrumClientFromUrl(primaryUrl)
	if err != nil {
		log.Fatal(err)
	}

	update, err := client.FullScan(wollet)
	if err != nil {
		// Falling into a retryable error, making a request with the fallback client
		fallbackClient, err := lwk.ElectrumClientFromUrl(fallbackUrl)
		if err != nil {
			log.Fatal(err)
		}

		update, err = fallbackClient.FullScan(wollet)
		if err != nil {
			log.Fatal(err)
		}
	}

	if update != nil {
		if err := wollet.ApplyUpdate(*update); err != nil {
			log.Fatal(err)
		}
	}
	// ANCHOR_END: fallback_client
}
