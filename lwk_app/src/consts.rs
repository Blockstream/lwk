use std::net::{Ipv4Addr, SocketAddrV4};
use std::time::Duration;

pub const DEFAULT_ADDR: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 32_111);

pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

pub const SCANNING_INTERVAL: Duration = Duration::from_secs(10);

/// The keyorigin xpub of the AMP2 server key for the testnet network.
pub const AMP_KEYORIGIN_XPUB_TESTNET: &str = "[3d970d04/87h/1h/0h]tpubDC347GyKEGtyd4swZDaEmBTcNuqseyX7E3Yw58FoeV1njuBcUmBMr5vBeBh6eRsxKYHeCAEkKj8J2p2dBQQJwB8n33uyAPrdgwFxLFTCXRd";

/// The URL of the AMP2 server for the testnet network.
pub const AMP_URL_TESTNET: &str = "https://amp2.testnet.blockstream.com/";
