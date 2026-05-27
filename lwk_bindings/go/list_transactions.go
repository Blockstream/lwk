package main

import (
	"fmt"
	"lwk-go/lwk"
)

func main() {
	node := lwk.NewLwkTestEnv()
	fmt.Printf("Regtest height: %d\n", node.Height())

	network := lwk.NetworkRegtestDefault()
	fmt.Println("Network:", network)

	mnemonic, err := lwk.NewMnemonic("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about")
	if err != nil {
		panic(err)
	}

	client, err := lwk.ElectrumClientFromUrl(node.ElectrumUrl())
	if err != nil {
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

	idx := uint32(0)
	addrResult, err := wollet.Address(&idx)
	if err != nil {
		panic(err)
	}
	fmt.Println("Wollet address:", addrResult.Address())

	fundedSatoshi := uint64(100000)
	txid := node.SendToAddress(addrResult.Address(), fundedSatoshi, nil)
	fmt.Println("Funded txid:", txid)

	if _, err := wollet.WaitForTx(txid, client); err != nil {
		panic(err)
	}

	balance, err := wollet.Balance()
	if err != nil {
		panic(err)
	}
	policyAsset := network.PolicyAsset()
	fmt.Printf("Balance (policy asset): %d sats\n", balance[policyAsset])

	txs, err := wollet.Transactions()
	if err != nil {
		panic(err)
	}
	fmt.Printf("Number of transactions: %d\n", len(txs))
}
