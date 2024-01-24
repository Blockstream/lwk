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
