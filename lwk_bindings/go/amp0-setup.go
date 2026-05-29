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
			return // Exits gracefully like quit()
		}
	}

	// ANCHOR: amp0-setup
	// Create signer and watch only credentials
	username := "<username>"
	password := "<password>"
	mnemonic, err := lwk.NewMnemonic("<mnemonic>")
	mnemonic, err = lwk.MnemonicFromRandom(12) // ANCHOR: ignore
	if err != nil {
		log.Fatal(err)
	}

	network := lwk.NetworkTestnet()
	signer, err := lwk.NewSigner(mnemonic, network)
	if err != nil {
		log.Fatal(err)
	}

	fingerprint, err := signer.Fingerprint()
	if err != nil {
		log.Fatal(err)
	}

	username = fmt.Sprintf("user%s", fingerprint) // ANCHOR: ignore
	password = fmt.Sprintf("pass%s", fingerprint) // ANCHOR: ignore

	// Collect signer data
	signerData, err := signer.Amp0SignerData()
	if err != nil {
		log.Fatal(err)
	}

	// Connect to AMP0
	amp0, err := lwk.NewAmp0Connected(network, signerData)
	if err != nil {
		log.Fatal(err)
	}

	// Obtain and sign the authentication challenge
	challenge, err := amp0.GetChallenge()
	if err != nil {
		log.Fatal(err)
	}
	sig, err := signer.Amp0SignChallenge(challenge)
	if err != nil {
		log.Fatal(err)
	}

	// Login
	amp0Connected, err := amp0.Login(sig)
	if err != nil {
		log.Fatal(err)
	}

	// Create a new AMP0 account
	pointer, err := amp0Connected.NextAccount()
	if err != nil {
		log.Fatal(err)
	}
	accountXpub, err := signer.Amp0AccountXpub(pointer)
	if err != nil {
		log.Fatal(err)
	}
	ampId, err := amp0Connected.CreateAmp0Account(pointer, accountXpub)
	if err != nil {
		log.Fatal(err)
	}

	// Create watch only entries
	if err := amp0Connected.CreateWatchOnly(username, password); err != nil {
		log.Fatal(err)
	}

	// Use watch only credentials to interact with AMP0
	amp0New, err := lwk.NewAmp0(network, username, password, ampId)
	if err != nil {
		log.Fatal(err)
	}
	// ANCHOR_END: amp0-setup
	_ = amp0New

}
