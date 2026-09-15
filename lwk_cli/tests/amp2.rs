use lwk_test_util::TestEnvBuilder;

use crate::common::*;

#[test]
fn test_amp2() {
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let server_url = format!("--server-url {}", env.electrum_url());
    let addr = get_available_addr().unwrap();

    let tmp = tempfile::tempdir().unwrap();
    let datadir = tmp.path().display().to_string();
    let params = server_url;

    // regtest: AMP2 requires --amp2-url for all methods
    let cli = format!("cli --addr {addr} -n regtest --datadir {datadir}");
    let t = {
        let cli = cli.clone();
        let params = params.clone();
        std::thread::spawn(move || {
            sh(&format!("{cli} server start {params}"));
        })
    };
    std::thread::sleep(std::time::Duration::from_millis(500));

    sw_signer(&cli, "sw");

    let dbk = "--descriptor-blinding-key \"slip77(0684e43749a3a3eb0362dcef8c66994bd51d33f8ce6b055126a800a626fc0d67)\"";
    for cmd in [
        format!("{cli} amp2 descriptor -s sw {dbk}"),
        format!("{cli} amp2 register -s sw {dbk}"),
        format!("{cli} amp2 cosign -p fake_pset"),
    ] {
        let err = sh_err(&cmd);
        assert!(err.contains("on regtest you have to specify --amp2-url"));
    }

    sh(&format!("{cli} server stop"));
    t.join().unwrap();

    // regtest: AMP2 requires --amp2-keyorigin-xpub once --amp2-url is set
    let t = {
        let cli = cli.clone();
        let params = params.clone();
        std::thread::spawn(move || {
            sh(&format!("{cli} server start {params} --amp2-url fake_url"));
        })
    };
    std::thread::sleep(std::time::Duration::from_millis(500));

    for cmd in [
        format!("{cli} amp2 descriptor -s sw {dbk}"),
        format!("{cli} amp2 register -s sw {dbk}"),
        format!("{cli} amp2 cosign -p fake_pset"),
    ] {
        let err = sh_err(&cmd);
        assert!(err.contains("on regtest you have to specify --amp2-keyorigin-xpub"));
    }

    sh(&format!("{cli} server stop"));
    t.join().unwrap();

    // mainnet: AMP2 is not available
    let cli = format!("cli --addr {addr} -n mainnet --datadir {datadir}");
    let t = {
        let cli = cli.clone();
        let params = params.clone();
        std::thread::spawn(move || {
            sh(&format!("{cli} server start {params} --amp2-url fake_url"));
        })
    };
    std::thread::sleep(std::time::Duration::from_millis(500));

    sw_signer(&cli, "sw");

    for cmd in [
        format!("{cli} amp2 descriptor -s sw {dbk}"),
        format!("{cli} amp2 register -s sw {dbk}"),
        format!("{cli} amp2 cosign -p fake_pset"),
    ] {
        let err = sh_err(&cmd);
        assert!(err.contains("AMP2 methods are not available for mainnet"));
    }

    sh(&format!("{cli} server stop"));
    t.join().unwrap();

    // TODO: proper e2e tests with regtest AMP2
}
