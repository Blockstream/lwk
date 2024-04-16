use std::{collections::HashMap, env, net};

use testcontainers::{
    core::{Port, WaitFor},
    Image, ImageArgs, RunnableImage,
};

pub const REGISTRY_PORT: u16 = 3000;

#[derive(Debug)]
pub struct Registry {
    vars: HashMap<String, String>,
}

/// Returns a non-used local port if available.
///
/// duplicated because may cause circular deps if put in test_util
fn get_available_addr() -> net::SocketAddr {
    // using 0 as port let the system assign a port available
    let t = net::TcpListener::bind(net::SocketAddrV4::new(net::Ipv4Addr::new(127, 0, 0, 1), 0))
        .expect("cannot bind");
    t.local_addr().expect("cannot get local addr")
}

impl Registry {
    /// Create a registry server
    ///
    /// Takes the port of an esplora instance running on the host to fetch transactions
    /// Skip domain verification, any domain will be succesfully verified
    pub fn new(esplora_port: u16) -> RunnableImage<Registry> {
        let mut vars = HashMap::new();
        vars.insert("ADDR".to_owned(), "127.0.0.1:3000".to_owned());
        vars.insert("DB_PATH".to_owned(), "/tmp".to_owned());
        vars.insert("ESPLORA_URL".to_owned(), "127.0.0.1:3001".to_owned());
        vars.insert("SKIP_VERIFY_DOMAIN_LINK".to_owned(), "1".to_owned());

        let image = Self { vars };

        let free_port = get_available_addr().port();

        RunnableImage::from(image)
            .with_mapped_port(Port {
                local: esplora_port,
                internal: 3001,
            })
            .with_mapped_port(Port {
                local: free_port,
                internal: 3000,
            })
    }
}

#[derive(Clone, Debug, Default)]
pub struct Args;

impl ImageArgs for Args {
    fn into_iterator(self) -> Box<dyn Iterator<Item = String>> {
        // let args = ["-p".to_string(), "3009:3010".to_string()];
        let args = [];
        Box::new(args.into_iter())
    }
}

impl Image for Registry {
    type Args = Args;

    fn name(&self) -> String {
        env::var("REGISTRY_IMAGE_NAME").unwrap_or("xenoky/liquid-asset-registry".into())
    }

    fn tag(&self) -> String {
        env::var("REGISTRY_IMAGE_VERSION").unwrap_or("02843eb2".into())
    }

    fn ready_conditions(&self) -> Vec<WaitFor> {
        vec![WaitFor::StdErrMessage {
            message: "INFO Starting web server on 127.0.0.1:3000".into(),
        }]
    }

    fn expose_ports(&self) -> Vec<u16> {
        [REGISTRY_PORT].into()
    }

    fn env_vars(&self) -> Box<dyn Iterator<Item = (&String, &String)> + '_> {
        Box::new(self.vars.iter())
    }
}
