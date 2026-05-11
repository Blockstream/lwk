use std::{num::NonZeroU8, path::PathBuf};

use tiny_http::Header;

#[derive(Debug, Clone)]
pub struct Config {
    /// Additional headers to add to GET and OPTIONS requests.
    pub headers: Vec<Header>,
    /// The number of threads to use for serving requests.
    pub num_threads: NonZeroU8,
    /// The path to serve HTTP GET requests from.
    pub serve_dir: Option<PathBuf>,
}

impl Config {
    pub fn builder() -> ConfigBuilder {
        ConfigBuilder::default()
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            headers: Vec::new(),
            num_threads: NonZeroU8::new(4).expect("non-zero"),
            serve_dir: None,
        }
    }
}

pub struct ConfigBuilder {
    headers: Vec<Header>,
    num_threads: NonZeroU8,
    serve_dir: Option<PathBuf>,
}

impl ConfigBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_headers(mut self, headers: Vec<Header>) -> Self {
        self.headers = headers;
        self
    }

    pub fn with_num_threads(mut self, num: NonZeroU8) -> Self {
        self.num_threads = num;
        self
    }

    pub fn with_serve_dir(mut self, dir: Option<PathBuf>) -> Self {
        self.serve_dir = dir;
        self
    }

    pub fn build(self) -> Config {
        Config {
            headers: self.headers,
            num_threads: self.num_threads,
            serve_dir: self.serve_dir,
        }
    }
}

impl Default for ConfigBuilder {
    fn default() -> Self {
        Self {
            headers: Vec::new(),
            num_threads: NonZeroU8::new(4).expect("non-zero"),
            serve_dir: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Config, ConfigBuilder};
    use std::{num::NonZeroU8, path::PathBuf};
    use tiny_http::Header;

    #[test]
    fn config_builder_new_uses_default_values() {
        let config = ConfigBuilder::new().build();

        assert!(config.headers.is_empty());
        assert_eq!(config.num_threads, NonZeroU8::new(4).expect("non-zero"));
        assert_eq!(config.serve_dir, None);

        let default_config = Config::default();
        assert_eq!(config.num_threads, default_config.num_threads);
        assert_eq!(config.serve_dir, default_config.serve_dir);
    }

    #[test]
    fn config_builder_applies_headers_and_serve_dir() {
        let headers = vec![
            Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap(),
            Header::from_bytes(&b"X-Test-Header"[..], &b"lwk"[..]).unwrap(),
        ];
        let serve_dir = Some(PathBuf::from("public"));
        let num_threads = NonZeroU8::new(2).expect("non-zero");

        let config = ConfigBuilder::new()
            .with_headers(headers)
            .with_num_threads(num_threads)
            .with_serve_dir(serve_dir.clone())
            .build();

        assert_eq!(config.headers.len(), 2);
        assert_eq!(
            config.headers[0].to_string(),
            "Access-Control-Allow-Origin: *"
        );
        assert_eq!(config.headers[1].to_string(), "X-Test-Header: lwk");
        assert_eq!(config.num_threads, num_threads);
        assert_eq!(config.serve_dir, serve_dir);
    }
}
