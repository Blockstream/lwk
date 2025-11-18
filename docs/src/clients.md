# Blockchain Clients

LWK supports different ways to retrieve wallet data from the Liquid blockchain:

- **Electrum** - TCP-based protocol, widely supported
- **Esplora** - HTTP-based REST API, browser-compatible
- **Waterfalls** - Optimized HTTP-based protocol with reduced roundtrips

Some clients also come in different flavors: blocking or async.
It's also possible to connect to authenticated backends for enterprise deployments.

## Quick Comparison

| Feature | Electrum | Esplora | Waterfalls |
|---------|----------|---------|------------|
| **Protocol** | TCP | HTTP/HTTPS | HTTP/HTTPS |
| **Browser Support** | ❌ No | ✅ Yes | ✅ Yes |
| **Mobile Support** | ✅ Yes | ✅ Yes | ✅ Yes |
| **Sync Speed** | 🏃 Average | 🐢 Slower | 🚀 Fastest |
| **Roundtrips** | Many but batched | Many | Few |
| **Async Support** | ❌ No | ✅ Yes | ✅ Yes |
| **Authentication** | ❌ No | ✅ OAuth2 | 🔜 Planned |
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
</custom-tabs>

### Authenticated Esplora

Some Esplora servers, particularly enterprise deployments like Blockstream Enterprise, require authentication for access. LWK supports OAuth2-based authentication with automatic token refresh.

Use authenticated clients when:
- Connecting to private or enterprise Esplora instances
- Requiring guaranteed rate limits and service quality
- Needing additional privacy and dedicated infrastructure

<custom-tabs category="lang">
<div slot="title">Rust</div>
<section>

```rust,ignore
{{#include ../../lwk_wollet/tests/e2e.rs:authenticated_esplora_client}}
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
</custom-tabs>

### Authenticated Waterfalls

Waterfalls clients also support OAuth2-based authentication for enterprise deployments, similar to authenticated Esplora clients.

<custom-tabs category="lang">
<div slot="title">Rust</div>
<section>

```rust,ignore
{{#include ../../lwk_wollet/tests/e2e.rs:authenticated_waterfalls_client}}
```
</section>
</custom-tabs>



