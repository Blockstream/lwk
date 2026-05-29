package main

import (
	"log"

	"lwk-go/lwk"
)

func main() {
	// ANCHOR: bip85
	// Load mnemonic
	testMnemonic := "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
	mnemonic, err := lwk.NewMnemonic(testMnemonic)
	if err != nil {
		log.Fatal(err)
	}

	// Create signer
	network := lwk.NetworkTestnet()
	signer, err := lwk.NewSigner(mnemonic, network)
	if err != nil {
		log.Fatal(err)
	}

	// Derive mnemonics
	derived0_12, err := signer.DeriveBip85Mnemonic(uint32(0), uint32(12))
	if err != nil {
		log.Fatal(err)
	}

	derived0_24, err := signer.DeriveBip85Mnemonic(uint32(0), uint32(24))
	if err != nil {
		log.Fatal(err)
	}

	derived1_12, err := signer.DeriveBip85Mnemonic(uint32(1), uint32(12))
	if err != nil {
		log.Fatal(err)
	}
	// ANCHOR_END: bip85

	_ = derived0_12
	_ = derived0_24
	_ = derived1_12
}
