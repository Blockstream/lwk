use std::{net::IpAddr, str::FromStr};

/// An electrum url parsable from string in the following form: `tcp://example.com:50001` or `ssl://example.com:50002`
///
/// If you need to use tls without validating the domain, use the constructor [`ElectrumUrl`]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ElectrumUrl {
    /// The TLS scheme with the domain name and the flag indicating if the domain name should be validated
    Tls(String, bool),
    /// The plaintext scheme with the domain name
    Plaintext(String),
}

impl FromStr for ElectrumUrl {
    type Err = UrlError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let url: url::Url = s.parse()?;
        let ssl = url.scheme() == "ssl";
        if !(ssl || url.scheme() == "tcp") {
            return Err(UrlError::Schema(url.scheme().to_string()));
        }
        if url.port().is_none() {
            return Err(UrlError::MissingPort);
        }
        match url.domain() {
            Some(domain) => match domain.parse::<IpAddr>() {
                Ok(_) => {
                    if ssl {
                        Err(UrlError::SslWithoutDomain)
                    } else {
                        ElectrumUrl::new(&s[6..], false, false)
                    }
                }
                Err(_) => ElectrumUrl::new(&s[6..], ssl, ssl),
            },
            None => Err(UrlError::MissingDomain),
        }
    }
}

impl std::fmt::Display for ElectrumUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ElectrumUrl::Tls(s, _) => write!(f, "ssl://{}", s),
            ElectrumUrl::Plaintext(s) => write!(f, "tcp://{}", s),
        }
    }
}

impl ElectrumUrl {
    /// Create an electrum url to create an [`crate::ElectrumClient`]
    ///
    /// The given `host_port` is a domain name or an ip with the port and without the scheme,
    /// eg. `example.com:50001` or `127.0.0.1:50001`
    ///
    /// Note: you cannot validate domain without TLS, an error is thrown in this case.
    pub fn new(host_port: &str, tls: bool, validate_domain: bool) -> Result<Self, UrlError> {
        // We are not checking all possible scheme, however, these two seems to be the most common
        // since they are used in the electrum protocol
        if host_port.starts_with("tcp://") || host_port.starts_with("ssl://") {
            return Err(UrlError::NoScheme);
        }

        if tls {
            Ok(ElectrumUrl::Tls(host_port.into(), validate_domain))
        } else if validate_domain {
            Err(UrlError::ValidateWithoutTls)
        } else {
            Ok(ElectrumUrl::Plaintext(host_port.into()))
        }
    }
}

/// Error type when parsing a string to the [`ElectrumUrl`] type.
#[derive(thiserror::Error, Debug)]
#[allow(missing_docs)]
pub enum UrlError {
    #[error(transparent)]
    Url(#[from] url::ParseError),

    #[error("Invalid schema `{0}` supported ones are `ssl` or `tcp`")]
    Schema(String),

    #[error("Port is missing")]
    MissingPort,

    #[error("Domain is missing")]
    MissingDomain,

    #[error("Cannot specify `ssl` scheme without a domain")]
    SslWithoutDomain,

    #[error("Cannot validate the domain without tls")]
    ValidateWithoutTls,

    #[error("Don't specify the scheme in the url")]
    NoScheme,
}
