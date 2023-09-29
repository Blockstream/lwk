use testcontainers::{core::WaitFor, Image, ImageArgs};

pub const EMULATOR_PORT: u16 = 30_121;

#[derive(Debug, Default)]
pub struct JadeEmulator;

#[derive(Clone, Debug, Default)]
struct Args;

impl ImageArgs for Args {
    fn into_iterator(self) -> Box<dyn Iterator<Item = String>> {
        let args = ["bash".to_string()];
        Box::new(args.into_iter())
    }
}

impl Image for JadeEmulator {
    type Args = ();

    fn name(&self) -> String {
        "xenoky/local-jade-emulator".into() // TODO Change with blockstream official jade emulator
    }

    fn tag(&self) -> String {
        "latest".into()
    }

    fn ready_conditions(&self) -> Vec<WaitFor> {
        vec![WaitFor::StdOutMessage {
            message: "char device redirected".into(),
        }]
    }

    fn expose_ports(&self) -> Vec<u16> {
        [EMULATOR_PORT].into()
    }
}
