use crate::test_wollet::*;
use clients::blocking::BlockchainBackend;
use lwk_test_util::*;
use lwk_wollet::*;

/// Authenticated Explorer API: one call through the real auth path (Keycloak-issued
/// `client_credentials` token, validated by the APISIX `openid-connect` plugin, proxied to the
/// regtest Esplora). Also proves the gateway rejects unauthenticated calls.
#[cfg(feature = "esplora")]
#[tokio::test]
#[ignore = "requires docker and the blockstream/apisix image"]
async fn test_esplora_authenticated() {
    let env = TestEnvBuilder::from_env()
        .with_esplora()
        .with_auth()
        .build();
    let gateway_url = env.esplora_url();
    let network = Network::default_regtest();

    // without a token the gateway rejects the call
    let mut client = clients::asyncr::EsploraClientBuilder::new(&gateway_url, network)
        .build()
        .unwrap();
    assert!(client.tip().await.is_err());

    // with the token provider the token is fetched from keycloak and the call is served
    let token_provider = clients::TokenProvider::Blockstream {
        url: env.oidc_token_url(),
        client_id: lwk_test_util::AUTH_CLIENT_ID.to_string(),
        client_secret: lwk_test_util::AUTH_CLIENT_SECRET.to_string(),
    };
    let mut client = clients::asyncr::EsploraClientBuilder::new(&gateway_url, network)
        .token_provider(token_provider)
        .build()
        .unwrap();
    let tip = client.tip().await.unwrap();
    assert_eq!(tip.height, 101);
}

/// Authenticated Waterfalls API through the gateway with the production plugin chain
/// (openid-connect → oidc-identity-extractor → credit-checker): a funded wallet syncs via
/// the waterfalls endpoint with a Keycloak token, and exhausted credits deny the calls
/// (402 Payment Required).
#[cfg(feature = "esplora")]
#[test]
#[ignore = "requires docker and the blockstream/apisix image"]
fn test_waterfalls_authenticated() {
    let env = TestEnvBuilder::from_env()
        .with_waterfalls()
        .with_auth()
        .build();
    let network = Network::default_regtest();

    let token_provider = || clients::TokenProvider::Blockstream {
        url: env.oidc_token_url(),
        client_id: lwk_test_util::AUTH_CLIENT_ID.to_string(),
        client_secret: lwk_test_util::AUTH_CLIENT_SECRET.to_string(),
    };
    let client = clients::WaterfallsClientBuilder::new(&env.waterfalls_url(), network)
        .token_provider(token_provider())
        .build_blocking()
        .unwrap();

    let signer = generate_signer();
    let view_key = generate_view_key();
    let desc = format!("ct({},elwpkh({}/<0;1>/*))", view_key, signer.xpub());
    let mut wallet = TestWollet::new(client, &desc);

    // funding syncs the wallet through the authenticated gateway
    wallet.fund_btc(&env);

    // valid token but exhausted credits -> denied with 402 Payment Required
    env.set_credits(0);
    let mut denied_client = clients::WaterfallsClientBuilder::new(&env.waterfalls_url(), network)
        .token_provider(token_provider())
        .build_blocking()
        .unwrap();
    let err = denied_client.full_scan(&wallet.wollet).unwrap_err();
    assert!(
        matches!(err, Error::EsploraHttpError { status: 402, .. }),
        "expected an EsploraHttpError with status 402, got: {err:?}"
    );
}

/// Authenticated Electrum RPC through the electrs-electrum-proxy (in-band JWT validated
/// against Keycloak's JWKS + credit checking): a valid token is served by the regtest
/// electrs, bogus/missing tokens and exhausted credits are denied.
///
/// Uses `TokenProvider::Static`; the automatic fetch/refresh (`TokenProvider::Blockstream`,
/// feature `electrum_oidc`) is exercised in `test_electrum_token_refresh_authenticated`.
#[cfg(feature = "electrum")]
#[test]
#[ignore = "requires docker, the blockstream/apisix image and the rpcproxy image"]
fn test_electrum_authenticated() {
    let env = TestEnvBuilder::from_env()
        .with_electrum()
        .with_auth()
        .build();
    let gateway_url = env.electrum_url();

    // The gateway is a localhost proxy, the documented case for allowing a token on tcp://
    let tip_with = |token_provider: clients::TokenProvider| {
        ElectrumClientBuilder::new(&gateway_url)
            .token_provider(token_provider)
            .allow_plaintext_with_token(true)
            .build()
            .and_then(|mut client| client.tip())
    };

    // valid token + credits -> served by the regtest electrs
    let token = env.fetch_oidc_token();
    let tip = tip_with(clients::TokenProvider::Static(token.clone())).unwrap();
    assert_eq!(tip.height, 101);

    // The proxy denies with well-defined JSON-RPC error codes, surfaced today inside the
    // wrapped electrum_client protocol error.
    // TODO(#398): map these (and the esplora 401/402/429) to common lwk error variants so
    // callers don't need transport-specific handling.
    let assert_denied_with = |result: Result<elements::BlockHeader, Error>, code: i64| {
        let err = result.unwrap_err();
        match &err {
            Error::ClientError(electrum_client::Error::Protocol(value)) => assert_eq!(
                value.get("code").and_then(|c| c.as_i64()),
                Some(code),
                "expected a denial with JSON-RPC code {code}, got: {value}"
            ),
            other => panic!("expected a protocol error with code {code}, got: {other:?}"),
        }
    };

    // bogus token -> denied with AUTHENTICATION_REQUIRED
    assert_denied_with(
        tip_with(clients::TokenProvider::Static("not-a-jwt".to_string())),
        -32004,
    );

    // missing token -> denied with AUTHENTICATION_REQUIRED
    assert_denied_with(tip_with(clients::TokenProvider::None), -32004);

    // valid token but exhausted credits -> denied with INSUFFICIENT_CREDITS
    env.set_credits(0);
    assert_denied_with(tip_with(clients::TokenProvider::Static(token)), -32000);
}

/// Automatic OAuth2 token refetch for the esplora client against real token expiry: the
/// short-lifespan realm client issues five-second tokens, so the gateway starts denying
/// them with 401 and the client transparently discards the cached token and mints a new
/// one on the denied request's retry.
///
/// Transport difference with `test_electrum_token_refresh_authenticated`: the esplora
/// client sends the token on every HTTP request, so the gateway re-validates it each time
/// and the 401 arrives on the very request made with the expired token, without any
/// reconnection involved.
#[cfg(feature = "esplora")]
#[tokio::test]
#[ignore = "requires docker and the blockstream/apisix image"]
async fn test_esplora_token_expiry_authenticated() {
    let env = TestEnvBuilder::from_env()
        .with_esplora()
        .with_auth()
        .build();
    let gateway_url = env.esplora_url();
    let network = Network::default_regtest();

    let mut client = clients::asyncr::EsploraClientBuilder::new(&gateway_url, network)
        .token_provider(clients::TokenProvider::Blockstream {
            url: env.oidc_token_url(),
            client_id: AUTH_SHORT_CLIENT_ID.to_string(),
            client_secret: AUTH_SHORT_CLIENT_SECRET.to_string(),
        })
        .build()
        .unwrap();
    assert_eq!(client.tip().await.unwrap().height, 101);

    // Expiry oracle: this static token is minted after the client's cached one, so once
    // the gateway denies it, the client's token is certainly expired too. Polling instead
    // of a fixed sleep keeps the test honest about the gateway's `exp` leeway: APISIX
    // (lua-resty-openidc) validates `exp` with a 120s default leeway, so the denial
    // arrives ~125s after the mint, not after 5s.
    let static_token = env.fetch_oidc_token_for(AUTH_SHORT_CLIENT_ID, AUTH_SHORT_CLIENT_SECRET);
    let mut denied_client = clients::asyncr::EsploraClientBuilder::new(&gateway_url, network)
        .token_provider(clients::TokenProvider::Static(static_token))
        .build()
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(240);
    loop {
        match denied_client.tip().await {
            Ok(_) => {} // the token is still accepted, keep waiting for the expiry
            Err(e) => {
                // The esplora client retries the 401 to exhaustion (a static token can't
                // refresh) and collapses it into a Generic "too many retries" error, so
                // there is no structured HTTP status to match on here (unlike the 402 case,
                // which returns a structured error). Surfacing a structured error through
                // retry-exhaustion is tracked in #398.
                assert!(
                    matches!(&e, Error::Generic(m) if m.contains("401")),
                    "expected a 401 denial once the token expires, got: {e:?}"
                );
                break;
            }
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the five-second token was not denied within 240s"
        );
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }

    // The client with the (now expired) Blockstream-provided token is served
    // transparently: the 401 makes it drop the cached token and mint a fresh one.
    assert_eq!(client.tip().await.unwrap().height, 101);
}

/// Automatic OAuth2 token fetch and refresh for the Electrum client
/// (`TokenProvider::Blockstream`, feature `electrum_oidc`) against real token expiry,
/// using the short-lifespan realm client issuing five-second tokens.
///
/// Transport difference with `test_esplora_token_expiry_authenticated`: the Electrum RPC
/// proxy validates the token only on each connection's first message and closes denied
/// connections. An established connection keeps being served after its token expires; the
/// expired token only surfaces (as JSON-RPC error -32004) when a new connection is made,
/// e.g. after a server restart or idle disconnect. The client then invalidates the cached
/// token and retries the call once, minting a fresh token for the new connection. The
/// reconnection is exercised here by restarting the proxy container.
#[cfg(feature = "electrum_oidc")]
#[test]
#[ignore = "requires docker, the blockstream/apisix image and the rpcproxy image"]
fn test_electrum_token_refresh_authenticated() {
    let env = TestEnvBuilder::from_env()
        .with_electrum()
        .with_auth()
        .build();
    let gateway_url = env.electrum_url();

    // A wrong secret fails at build() with the token endpoint's own error (401 with
    // `invalid_client`), not an opaque denial from the electrum gateway.
    let err = ElectrumClientBuilder::new(&gateway_url)
        .token_provider(clients::TokenProvider::Blockstream {
            url: env.oidc_token_url(),
            client_id: AUTH_SHORT_CLIENT_ID.to_string(),
            client_secret: "wrong-secret".to_string(),
        })
        .allow_plaintext_with_token(true)
        .build();
    assert!(
        matches!(err, Err(Error::EsploraHttpError { status: 401, .. })),
        "expected the token endpoint's 401 for a wrong secret, got: {err:?}"
    );

    // The token is minted when the client is built and the connection is served.
    let mut client = ElectrumClientBuilder::new(&gateway_url)
        .token_provider(clients::TokenProvider::Blockstream {
            url: env.oidc_token_url(),
            client_id: AUTH_SHORT_CLIENT_ID.to_string(),
            client_secret: AUTH_SHORT_CLIENT_SECRET.to_string(),
        })
        .allow_plaintext_with_token(true)
        .build()
        .unwrap();
    assert_eq!(client.tip().unwrap().height, 101);

    // Mine a block so a served tip() is distinguishable from the last known header
    // (`tip()` returns the cached header when the subscription errors).
    env.elementsd_generate(1);

    // Expiry oracle: this static token is minted after the client's cached one, so once
    // fresh connections with it are denied, the client's token is certainly expired too.
    // Polling instead of a fixed sleep keeps the test honest about the proxy's `exp`
    // leeway: the jsonwebtoken crate defaults to 60s, so the denial arrives ~65s after
    // the mint, not after 5s.
    let static_token = env.fetch_oidc_token_for(AUTH_SHORT_CLIENT_ID, AUTH_SHORT_CLIENT_SECRET);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
    loop {
        let denied = ElectrumClientBuilder::new(&gateway_url)
            .token_provider(clients::TokenProvider::Static(static_token.clone()))
            .allow_plaintext_with_token(true)
            .build();
        match denied {
            Ok(_) => {} // the token is still accepted on new connections, keep waiting
            Err(Error::ClientError(electrum_client::Error::Protocol(value)))
                if value.get("code").and_then(|c| c.as_i64()) == Some(-32004) =>
            {
                break;
            }
            Err(e) => panic!("expected a -32004 denial once the token expires, got: {e:?}"),
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the five-second token was not denied within 120s"
        );
        std::thread::sleep(std::time::Duration::from_secs(5));
    }

    // The established connection is still served with the expired token: the proxy
    // validates per connection, not per message.
    assert_eq!(client.tip().unwrap().height, 102);

    // Drop the authenticated connection (as a production restart or idle disconnect
    // would): the next call reconnects, gets denied with -32004 because the cached token
    // expired, and transparently retries with a freshly minted token.
    env.restart_electrum_gateway();
    env.elementsd_generate(1);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    loop {
        // The first call after the restart needs a few seconds: electrum-client backs off
        // before reconnecting, once for the dead connection and once for the -32004 one.
        let outcome = match client.tip() {
            Ok(header) if header.height == 103 => break,
            Ok(header) => format!("Ok(height={})", header.height),
            Err(e) => format!("Err({e})"),
        };
        assert!(
            std::time::Instant::now() < deadline,
            "the tip did not advance to 103 within 60s of the gateway restart (last tip(): {outcome})"
        );
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
}
