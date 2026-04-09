use ::lnurl::lnurl::LnUrl;

use crate::Error;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LnUrlIdentifier {
    /// A lnurl
    LnUrl(LnUrl),

    /// A [LUD16](https://github.com/lnurl/luds/blob/luds/16.md) identifier like `alice@example.com`
    Lud16(String),
}

impl std::fmt::Display for LnUrlIdentifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LnUrl(lnurl) => write!(f, "{lnurl}"),
            Self::Lud16(lud16) => write!(f, "{lud16}"),
        }
    }
}

impl LnUrlIdentifier {
    pub fn lnurl(&self) -> Option<&LnUrl> {
        match self {
            Self::LnUrl(lnurl) => Some(lnurl),
            Self::Lud16(_) => None,
        }
    }

    pub fn lud16(&self) -> Option<&str> {
        match self {
            Self::LnUrl(_) => None,
            Self::Lud16(lud16) => Some(lud16),
        }
    }

    pub fn resolve_url(&self) -> Result<String, Error> {
        match self {
            Self::LnUrl(lnurl) => Ok(lnurl.url.clone()),
            Self::Lud16(email) => {
                let (user, domain) = email.split_once('@').ok_or("Invalid email")?;

                let schema = lnurl_schema_for_domain(domain)?;
                Ok(format!("{schema}://{domain}/.well-known/lnurlp/{user}"))
            }
        }
    }
}

fn lnurl_schema_for_domain(domain: &str) -> Result<&'static str, Error> {
    // TODO: support onion domains
    if domain.starts_with("127.0.0.1") || domain.starts_with("localhost") {
        if cfg!(debug_assertions) {
            // allow insecure LNURL resolution over HTTP for local domains in debug mode
            Ok("http")
        } else {
            Err(
                format!("Refusing insecure LNURL resolution over HTTP for local domain: {domain}")
                    .into(),
            )
        }
    } else {
        Ok("https")
    }
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LnUrlPayResponse {
    pub callback: String,
    pub max_sendable: u64,
    pub min_sendable: u64,
    pub metadata: String,
    pub tag: String,
}

#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LnUrlInvoiceResponse {
    pub pr: String,
    pub status: Option<String>,
    pub reason: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lnurl_identifier_resolve_url() {
        let lnurl = LnUrl::from_url("https://example.com/.well-known/lnurlp/alice".to_string());
        assert_eq!(
            LnUrlIdentifier::LnUrl(lnurl).resolve_url().unwrap(),
            "https://example.com/.well-known/lnurlp/alice"
        );

        assert_eq!(
            LnUrlIdentifier::Lud16("alice@example.com".to_string())
                .resolve_url()
                .unwrap(),
            "https://example.com/.well-known/lnurlp/alice"
        );

        if cfg!(debug_assertions) {
            assert_eq!(
                LnUrlIdentifier::Lud16("alice@localhost:8080".to_string())
                    .resolve_url()
                    .unwrap(),
                "http://localhost:8080/.well-known/lnurlp/alice"
            );
            assert_eq!(
                LnUrlIdentifier::Lud16("alice@127.0.0.1:8080".to_string())
                    .resolve_url()
                    .unwrap(),
                "http://127.0.0.1:8080/.well-known/lnurlp/alice"
            );
        } else {
            assert!(LnUrlIdentifier::Lud16("alice@localhost:8080".to_string())
                .resolve_url()
                .is_err());
            assert!(LnUrlIdentifier::Lud16("alice@127.0.0.1:8080".to_string())
                .resolve_url()
                .is_err());
        }
    }
}
