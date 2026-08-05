# Blockchain Clients

LWK supports different ways to retrieve wallet data from the Liquid blockchain:

- **Electrum** - TCP-based protocol, widely supported
- **Esplora** - HTTP-based REST API, browser-compatible
- **Waterfalls** - Optimized HTTP-based protocol with reduced roundtrips

Some clients also come in different flavors: blocking or async.

For production, all three clients can connect to Blockstream Enterprise authenticated, paid instances, see [Authenticated connections](#authenticated-connections).

## Quick Comparison

| Feature | Electrum | Esplora | Waterfalls |
|---------|----------|---------|------------|
| **Protocol** | TCP | HTTP/HTTPS | HTTP/HTTPS |
| **Browser Support** | ❌ No | ✅ Yes | ✅ Yes |
| **Mobile Support** | ✅ Yes | ✅ Yes | ✅ Yes |
| **Sync Speed** | 🏃 Average | 🐢 Slower | 🚀 Fastest |
| **Roundtrips** | Many but batched | Many | Few |
| **Async Support** | ❌ No | ✅ Yes | ✅ Yes |
| **Authentication** | ❌ No | ✅ OAuth2 | ✅ OAuth2 |
| **Maturity** | ⭐⭐⭐ Mature | ⭐⭐⭐ Mature | ⭐⭐ New |

## Electrum

The Electrum protocol is the most widely used light-client syncing mechanism for Bitcoin and Liquid wallets.

**Key characteristics:**
- **Protocol:** TCP-based
- **Performance:** Good
- **Availability:** Only blocking variant
- **Platform support:** Desktop, mobile, and server applications
- **Browser support:** ❌ No (TCP not available in browsers)
- **Default servers:** Blockstream public Electrum servers

This client is recommended for desktop, mobile, and server applications where interoperability is critical. By default, Blockstream public Electrum servers are used, but you can also specify custom URLs for private or local deployments.

<custom-tabs category="lang">
<div slot="title">Rust</div>
<section>

```rust,ignore
{{#include ../../lwk_wollet/tests/e2e.rs:electrum_client}}
```
</section>

<div slot="title">Python</div>
<section>

```python
{{#include ../../lwk_bindings/tests/bindings/basics.py:electrum_client}}
```
</section>

<div slot="title">Javascript</div>
<section>

```typescript
```
</section>

<div slot="title">Go</div>
<section>

```go
{{#include ../../lwk_bindings/go/basics.go:electrum_client}}
```
</section>
</custom-tabs>

## Esplora

The Esplora client is based on the [Esplora API](https://github.com/Blockstream/esplora/blob/master/API.md), a popular HTTP-based blockchain explorer API.

**Key characteristics:**
- **Protocol:** HTTP/HTTPS REST API
- **Performance:** Multiple roundtrips required for wallet sync
- **Availability:** Both blocking and async variants
- **Browser support:** ✅ Yes, works in web browsers
- **Authentication:** Supports OAuth2 for enterprise deployments

This client is ideal for web applications and scenarios where HTTP-based communication is required. While it requires more roundtrips than Electrum, it's the only option for browser-based applications and offers broad compatibility.

<custom-tabs category="lang">
<div slot="title">Rust</div>
<section>

```rust,ignore
{{#include ../../lwk_wollet/tests/e2e.rs:esplora_client}}
```
</section>

<div slot="title">Python</div>
<section>

```python
{{#include ../../lwk_bindings/tests/bindings/basics.py:esplora_client}}
```
</section>

<div slot="title">Javascript</div>
<section>

```typescript
{{#include ../../lwk_wasm/tests/node/basics.js:esplora_client}}
```
</section>

<div slot="title">Go</div>
<section>

```go
{{#include ../../lwk_bindings/go/basics.go:esplora_client}}
```
</section>
</custom-tabs>

## Waterfalls

[Waterfalls](https://github.com/RCasatta/waterfalls) is an optimized blockchain indexer designed to significantly reduce the number of roundtrips required for wallet synchronization compared to traditional Esplora.

**Key characteristics:**
- **Protocol:** HTTP/HTTPS REST API (Esplora-compatible with extensions)
- **Performance:** Fewer roundtrips than standard Esplora, faster sync times
- **Availability:** Both blocking and async variants
- **Browser support:** ✅ Yes, works in web browsers
- **Maturity:** Newer technology, still evolving

**Important:** The public Waterfalls instance shown in the examples (`waterfalls.liquidwebwallet.org`) is provided for testing and development only.

<custom-tabs category="lang">
<div slot="title">Rust</div>
<section>

```rust,ignore
{{#include ../../lwk_wollet/tests/e2e.rs:waterfalls_client}}
```
</section>

<div slot="title">Python</div>
<section>

```python
{{#include ../../lwk_bindings/tests/bindings/basics.py:waterfalls_client}}
```
</section>

<div slot="title">Javascript</div>
<section>

```typescript
{{#include ../../lwk_wasm/tests/node/basics.js:waterfalls_client}}
```
</section>

<div slot="title">Go</div>
<section>

```go
{{#include ../../lwk_bindings/go/basics.go:waterfalls_client}}
```
</section>
</custom-tabs>

## Fallback Client

For improved resilience, implement a fallback strategy to handle transient errors.
This pattern is useful when dealing with unreliable network conditions or temporary server issues.

When a primary request fails, manually evaluate the error to determine if a retry is appropriate with a different client.

<custom-tabs category="lang">
<div slot="title">Rust</div>
<section>

```rust,ignore
{{#include ../../lwk_wollet/tests/e2e.rs:fallback_client}}
```
</section>

<div slot="title">Python</div>
<section>

```python
{{#include ../../lwk_bindings/tests/bindings/fallback_client.py:fallback_client}}
```
</section>

<div slot="title">Javascript</div>
<section>

```typescript
{{#include ../../lwk_wasm/tests/node/fallback_client.js:fallback_client}}
```
</section>

<div slot="title">Go</div>
<section>

```go
{{#include ../../lwk_bindings/go/fallback_client.go:fallback_client}}
```
</section>
</custom-tabs>

## Authenticated connections

Blockstream runs paid, authenticated instances of these APIs for production use, [Blockstream Enterprise](https://blockstream.info/explorer-api): dedicated infrastructure with guaranteed rate limits, higher quotas, and greater privacy than the shared public servers. If you are shipping a product on Liquid, these are the endpoints to build against.

All three clients authenticate the same way. Point the client at your enterprise endpoint and add an OAuth2 **token provider**; the client fetches a token with your credentials and refreshes it automatically, so the rest of your code is unchanged from the public client.

### Endpoints

Mainnet Liquid enterprise endpoints:

| API | Endpoint | Transport |
|-----|----------|-----------|
| Esplora (REST) | `https://enterprise.blockstream.info/liquid/api` | HTTPS |
| Waterfalls | `https://enterprise.blockstream.info/liquid/api/waterfalls` | HTTPS |
| Electrum RPC | `ssl://elements-mainnet.enterprise.blockstream.info:50002` | TLS |
| OAuth2 token | `https://login.blockstream.com/realms/blockstream-public/protocol/openid-connect/token` | HTTPS |

The table lists mainnet Liquid. For Liquid testnet, swap the host prefix and path to `elements-testnet` and `liquidtestnet` (for example `ssl://elements-testnet.enterprise.blockstream.info:50002` and `https://enterprise.blockstream.info/liquidtestnet/api`); the OAuth2 token endpoint is unchanged.

### Token providers

- `TokenProvider::Blockstream { url, client_id, client_secret }` fetches a token from the OAuth2 endpoint and refreshes it automatically.
- `TokenProvider::Static(token)` uses a token you already hold (no refresh).

Notes:
- Electrum needs the `electrum_oidc` cargo feature (a default feature of `lwk_wollet`).
- The token is only sent over an encrypted connection. On a plaintext `tcp://` Electrum url it is refused unless explicitly allowed (for a localhost or already-tunneled proxy: `allow_plaintext_with_token` in the builder, or `--auth-allow-plaintext-with-token` in `lwk_cli`).
- Esplora and Waterfalls address the enterprise load balancer by path (`/liquid/api`, `/liquid/api/waterfalls`); Electrum uses a network-prefixed host.
- In the browser (wasm), authenticated Esplora/Waterfalls is not yet available, and Electrum has no browser path.

The snippets below show the client wiring; take the endpoint urls from the table above.

### Esplora

<custom-tabs category="lang">
<div slot="title">Rust</div>
<section>

```rust,ignore
{{#include ../../lwk_wollet/tests/auth.rs:authenticated_esplora_client}}
```
</section>

<div slot="title">Python</div>
<section>

```python
{{#include ../../lwk_bindings/tests/bindings/authenticated_esplora_client.py:authenticated_esplora_client}}
```
</section>

<div slot="title">Go</div>
<section>

```go
{{#include ../../lwk_bindings/go/authenticated_esplora_client.go:authenticated_esplora_client}}
```
</section>
</custom-tabs>

### Waterfalls

<custom-tabs category="lang">
<div slot="title">Rust</div>
<section>

```rust,ignore
{{#include ../../lwk_wollet/tests/auth.rs:authenticated_waterfalls_client}}
```
</section>
</custom-tabs>

### Electrum

<custom-tabs category="lang">
<div slot="title">Rust</div>
<section>

```rust,ignore
{{#include ../../lwk_wollet/tests/auth.rs:authenticated_electrum_client}}
```
</section>

<div slot="title">Python</div>
<section>

```python
{{#include ../../lwk_bindings/tests/bindings/authenticated_electrum_client.py:authenticated_electrum_client}}
```
</section>
</custom-tabs>

