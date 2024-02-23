use crate::error::Error;
use once_cell::sync::Lazy;
use regex_lite::Regex;

// Domain name validation code extracted from https://github.com/rushmorem/publicsuffix/blob/master/src/lib.rs,
// MIT, Copyright (c) 2016 Rushmore Mushambi

static DOMAIN_LABEL1: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[[:alnum:]]+$").expect("static"));

static DOMAIN_LABEL2: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[[:alnum:]]+[[:alnum:]-]*[[:alnum:]]+$").expect("static"));

pub fn verify_domain_name(domain: &str) -> Result<(), Error> {
    if domain.starts_with('.')
        || idna_to_ascii(domain)? != *domain
        || domain.to_lowercase() != domain
        || domain.len() > 255
    {
        return Err(Error::InvalidDomain);
    }

    let mut labels: Vec<&str> = domain.split('.').collect();
    // strip of the first dot from a domain to support fully qualified domain names
    if domain.ends_with('.') {
        labels.pop();
    }

    if labels.len() > 128 || labels.len() <= 1 {
        return Err(Error::InvalidDomain);
    }

    labels.reverse();
    for (i, label) in labels.iter().enumerate() {
        if i == 0 && label.parse::<f64>().is_ok() {
            // the tld must not be a number
            return Err(Error::InvalidDomain);
        }
        if !DOMAIN_LABEL1.is_match(label) && !DOMAIN_LABEL2.is_match(label) {
            return Err(Error::InvalidDomain);
        }
    }

    Ok(())
}

fn idna_to_ascii(domain: &str) -> Result<String, Error> {
    idna::domain_to_ascii(domain).map_err(|_| Error::InvalidDomain)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_domain() {
        assert!(verify_domain_name("foo.com").is_ok());
        assert!(verify_domain_name("foO.com").is_err());
        assert!(verify_domain_name(">foo.com").is_err());
        assert!(verify_domain_name("δοκιμή.com").is_err());
        assert!(verify_domain_name("xn--jxalpdlp.com").is_ok());
    }
}
