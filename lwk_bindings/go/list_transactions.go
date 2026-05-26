package main

import (
	"fmt"
	"lwk-go/lwk"
)

func main() {
	network := lwk.NetworkTestnet()
	fmt.Println("Network:", network)

	mnemonic, err := lwk.NewMnemonic("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about")
	if err != nil {
		panic(err)
	}
	fmt.Println("Mnemonic:", mnemonic)

	client, err := network.DefaultElectrumClient()
	if err != nil {
		panic(err)
	}

	if err := client.Ping(); err != nil {
		panic(err)
	}

	signer, err := lwk.NewSigner(mnemonic, network)
	if err != nil {
		panic(err)
	}

	desc, err := signer.WpkhSlip77Descriptor()
	if err != nil {
		panic(err)
	}
	fmt.Println("Descriptor:", desc)

	wollet, err := lwk.NewWollet(network, desc, nil)
	if err != nil {
		panic(err)
	}

	update, err := client.FullScan(wollet)
	if err != nil {
		panic(err)
	}
	if update != nil {
		if err := wollet.ApplyUpdate(*update); err != nil {
			panic(err)
		}
	}

	txs, err := wollet.Transactions()
	if err != nil {
		panic(err)
	}

	fmt.Printf("Number of transactions: %d\n", len(txs))
}
