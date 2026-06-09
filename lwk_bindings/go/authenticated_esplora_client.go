package main

import (
	"fmt"
	"log"
	"os"

	"lwk-go/lwk"
)

func offlineChecks(clientId, clientSecret string, network *lwk.Network) {
	// ANCHOR: authenticated_esplora_client
	baseUrl := "https://enterprise.blockstream.info/liquid/api"
	loginUrl := "https://login.blockstream.com/realms/blockstream-public/protocol/openid-connect/token"

	var tpBlockstream lwk.TokenProvider = lwk.TokenProviderBlockstream{
		Url:          loginUrl,
		ClientId:     clientId,
		ClientSecret: clientSecret,
	}

	builder := lwk.EsploraClientBuilder{
		BaseUrl:       baseUrl,
		Network:       network,
		TokenProvider: &tpBlockstream,
	}
	client, err := lwk.EsploraClientFromBuilder(builder)
	// ANCHOR_END: authenticated_esplora_client
	if err != nil {
		log.Fatal(err)
	}

	// The OAuth2 provider is recorded on the builder and the client is built.
	if _, ok := (*builder.TokenProvider).(lwk.TokenProviderBlockstream); !ok {
		panic("expected builder.TokenProvider to be of type TokenProviderBlockstream")
	}
	if client == nil {
		panic("client is nil")
	}
	log.Printf("built OAuth2 (Blockstream) client for %s", baseUrl)

	// A static token (when you already have one) and/or custom headers also work.
	var tpStatic lwk.TokenProvider = lwk.TokenProviderStatic{Token: "my-token"}
	headers := map[string]string{"X-Custom-Header": "value"}
	builderStatic := lwk.EsploraClientBuilder{
		BaseUrl:       baseUrl,
		Network:       network,
		TokenProvider: &tpStatic,
		Headers:       &headers,
	}
	clientStatic, err := lwk.EsploraClientFromBuilder(builderStatic)
	if err != nil {
		log.Fatal(err)
	}

	staticProvider, ok := (*builderStatic.TokenProvider).(lwk.TokenProviderStatic)
	if !ok {
		panic("expected builder.TokenProvider to be of type TokenProviderStatic")
	}
	if staticProvider.Token != "my-token" {
		panic("expected token to be 'my-token'")
	}
	if builderStatic.Headers == nil || (*builderStatic.Headers)["X-Custom-Header"] != "value" {
		panic("expected custom headers to be set")
	}
	if clientStatic == nil {
		panic("client is nil")
	}
	log.Println("built static-token client with custom headers")

	// No-auth builders must keep working (auth fields default to nil).
	builderNoAuth := lwk.EsploraClientBuilder{
		BaseUrl: baseUrl,
		Network: network,
	}

	if builderNoAuth.TokenProvider != nil {
		panic("expected TokenProvider to be nil")
	}
	if builderNoAuth.Headers != nil {
		panic("expected Headers to be nil")
	}

	clientNoAuth, err := lwk.EsploraClientFromBuilder(builderNoAuth)
	if err != nil {
		log.Fatal(err)
	}
	if clientNoAuth == nil {
		panic("client is nil")
	}
	log.Println("offline checks passed")
}

func liveCheck(clientId, clientSecret string, network *lwk.Network) error {
	if clientId == "your_client_id" || clientSecret == "your_client_secret" {
		log.Println("skipping live check: set CLIENT_ID and CLIENT_SECRET to enable it")
		return nil
	}

	baseUrl := os.Getenv("ESPLORA_BASE_URL")
	if baseUrl == "" {
		baseUrl = "https://enterprise.uat.blockstream.info/liquid/api"
	}

	loginUrl := os.Getenv("ESPLORA_LOGIN_URL")
	if loginUrl == "" {
		loginUrl = "https://login.uat.blockstream.com/realms/blockstream-public/protocol/openid-connect/token"
	}

	timeout := uint8(30)

	var liveTp lwk.TokenProvider = lwk.TokenProviderBlockstream{
		Url:          loginUrl,
		ClientId:     clientId,
		ClientSecret: clientSecret,
	}

	builder := lwk.EsploraClientBuilder{
		BaseUrl:       baseUrl,
		Network:       network,
		TokenProvider: &liveTp,
		Timeout:       &timeout,
	}

	client, err := lwk.EsploraClientFromBuilder(builder)
	if err != nil {
		return err
	}

	log.Printf("authenticating against %s ...", baseUrl)

	// triggers the OAuth token fetch + an authenticated request
	tip, err := client.Tip()
	if err != nil {
		return err
	}

	log.Printf("authenticated OK: chain tip height=%d hash=%s", tip.Height(), tip.BlockHash())
	if tip.Height() <= 100 {
		return fmt.Errorf("unexpected tip height %d", tip.Height())
	}

	return nil
}

func main() {
	network := lwk.NetworkMainnet()

	clientId := os.Getenv("CLIENT_ID")
	if clientId == "" {
		clientId = "your_client_id"
	}

	clientSecret := os.Getenv("CLIENT_SECRET")
	if clientSecret == "" {
		clientSecret = "your_client_secret"
	}

	offlineChecks(clientId, clientSecret, network)

	if err := liveCheck(clientId, clientSecret, network); err != nil {
		// A network/credential failure here shouldn't look like a wiring bug.
		log.Printf("live check failed (network or credentials): %v\n", err)
		os.Exit(1)
	}

	log.Println("done")
}
