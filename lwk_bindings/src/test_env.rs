#[derive(uniffi::Object)]
pub struct TestEnv {
    inner: lwk_test_util::TestElectrumServer,
}

#[uniffi::export]
impl TestEnv {
    #[allow(clippy::new_without_default)]
    #[uniffi::constructor]
    pub fn new() -> TestEnv {
        TestEnv {
            inner: lwk_test_util::setup(true),
        }
    }

    pub fn generate(&self, blocks: u32) {
        self.inner.generate(blocks);
    }

    pub fn height(&self) -> u64 {
        self.inner.node_height()
    }
}
