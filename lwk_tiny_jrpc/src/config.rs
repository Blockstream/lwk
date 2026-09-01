use std::num::NonZeroU8;

#[derive(Debug, Clone)]
pub struct Config {
    /// The number of threads to use for serving requests.
    pub num_threads: NonZeroU8,
    /// If set, POST requests must carry an `Authorization` header equal to this value.
    pub expected_auth_header: Option<String>,
}

impl Config {
    pub fn builder() -> ConfigBuilder {
        ConfigBuilder::default()
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            num_threads: NonZeroU8::new(4).expect("non-zero"),
            expected_auth_header: None,
        }
    }
}

pub struct ConfigBuilder {
    num_threads: NonZeroU8,
    expected_auth_header: Option<String>,
}

impl ConfigBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_num_threads(mut self, num: NonZeroU8) -> Self {
        self.num_threads = num;
        self
    }

    pub fn with_expected_auth_header(mut self, expected_auth_header: Option<String>) -> Self {
        self.expected_auth_header = expected_auth_header;
        self
    }

    pub fn build(self) -> Config {
        Config {
            num_threads: self.num_threads,
            expected_auth_header: self.expected_auth_header,
        }
    }
}

impl Default for ConfigBuilder {
    fn default() -> Self {
        Self {
            num_threads: NonZeroU8::new(4).expect("non-zero"),
            expected_auth_header: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Config, ConfigBuilder};
    use std::num::NonZeroU8;

    #[test]
    fn config_builder_new_uses_default_values() {
        let config = ConfigBuilder::new().build();

        assert_eq!(config.num_threads, NonZeroU8::new(4).expect("non-zero"));

        let default_config = Config::default();
        assert_eq!(config.num_threads, default_config.num_threads);
    }
}
