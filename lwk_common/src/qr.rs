use base64::engine::general_purpose;
use elements::Address;

/// Convert the given address in a string representing a QR code to be consumed from a terminal
pub fn address_to_text_qr(address: &Address) -> String {
    let address = address.to_string();
    let qr_code = qr_code::QrCode::new(&address).unwrap();
    qr_code.to_string(true, 3)
}

/// Convert the given elements address to an image uri
///
/// The image format is monocromatic bitmap, encoded in base64 in the uri.
///
/// The text content of the QR doesn't contain a schema
///
/// The text content of the QR is not uppercased when possible to optimize
///
/// Without `pixel_per_module` the default is no border, and 1 pixel per module, to be used
/// for example in html: `style="image-rendering: pixelated; border: 20px solid white;"`
pub fn address_to_uri_qr(address: &Address, pixel_per_module: Option<u8>) -> String {
    let address = address.to_string();
    let qr_code = qr_code::QrCode::new(&address).unwrap();
    let mut bmp = qr_code.to_bmp();
    if let Some(pixel_per_module) = pixel_per_module {
        bmp = bmp
            .add_white_border(1)
            .unwrap()
            .mul(pixel_per_module)
            .unwrap();
    }
    let mut enc = base64::write::EncoderWriter::new(Vec::new(), &general_purpose::STANDARD);

    bmp.write(&mut enc).unwrap();
    let delegate = enc.finish().unwrap();

    let base64 = String::from_utf8(delegate).unwrap();
    format!("data:image/bmp;base64,{}", base64)
}

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use elements::Address;

    use super::*;

    const ADDR: &str = "lq1qqf8er278e6nyvuwtgf39e6ewvdcnjupn9a86rzpx655y5lhkt0walu3djf9cklkxd3ryld97hu8h3xepw7sh2rlu7q45dcew5";

    #[test]
    fn test_address_to_text_qr() {
        let address = Address::from_str(ADDR).unwrap();
        let text_qr = address_to_text_qr(&address);
        let expected = "
███████████████████████████████████████████████
███████████████████████████████████████████████
███ ▄▄▄▄▄ ██ █▀▀▀█ ▄ █▄▄ ▀▄██▄▀ ▀▀▄██ ▄▄▄▄▄ ███
███ █   █ █ ▄ ▀▀    ▄▄▀█▄ ▀▄▀▄█▀▀▄▄▄█ █   █ ███
███ █▄▄▄█ █ █  █▄██ ▄ ▄▄▄ ▄ ▀ ▀▄▀▀▄▄█ █▄▄▄█ ███
███▄▄▄▄▄▄▄█ ▀ █▄▀▄█ █▄█ ▀ ▀▄█▄█ █ █▄█▄▄▄▄▄▄▄███
███▄▀▄ ▄ ▄▀▀█▄█▀▀▄██ ▀ ▄▀ ▀█▄▄▄▄ ▄▀ █ ▄   █▀███
███ █▄ █ ▄█▀█▄   ▄▄▄█ ▀▄  ██▄█  ▄▀█ █▀  ▀██▀███
███ █▄▄▄ ▄▀▄ █▄▀▀ ▄█▄▀█▄█▀▀▄  ▄  ▄▀▄▄▀▀▀ ▄▄▀███
███ ▄██  ▄█▀█▀▀▄▀▀█▄███▄ ▄▀▀██▄ █▄██▄█▄▀ ▄▀████
████  ▄  ▄█ ▄  █ ▄ ▄▄▄▀▀▀▄▀█▄▄▄▀▄▀ ▀▀▀ ▀ ▄▄▀███
█████▄█  ▄▄▀▄█▀▄█▀█▀█▄▀ ▀ ▀ █▄ ▀▄ ▀█▀ ▄  ▄█████
████ ▀▄  ▄ ▀ ▀█▀▀  ██▄▀▀▀▀▀▀ ▀█▄▄▀  ▀▀ ▀█ ▀▀███
███ ▀ █ ▄▄▄▄█▄█ ▀▀▀▀███  ▄█ ▄▄ █▄ █ ▄▀█ ▀██████
███▀▀ █ ▄▄██▄██ █ ▄▄ ▄▀▄▀▄ █  █▀ ▄▀▀▀▀ ▀█▄█▀███
███▄██▀ ▄▄▄▀▀▀▀▀ ▄▄█▄▄▀▀  ▀▄▄█ ██▄███▀▀▄▄█ ████
███▀▄  ▀▀▄ ▄▄█▄▀▄    ██▀▀ ▀▄▄ █▄▄▀ ▀█▀▀▀ ▄▄ ███
███ █   ▄▄▀ ▀▀▀▀██▄ █  ▀ ▄█▄█▄  ▄▄▀▄▀▀▀ ▀▄ ████
███▄█▄█▄█▄▄ █▄▀▀▄█   ▀ ▄▀▀▀█▄▀▄▀ ▀█ ▄▄▄ █▄▀▀███
███ ▄▄▄▄▄ █▀▄▄▀  ▄▀▀ ▀█▄  ▀▀█ ▄▄█▀  █▄█  █▀▀███
███ █   █ █  ▀▄▄█▄█▄▀▀ ▄ █▀█ ▄█▄▄██▄ ▄ ▄▀▄▀▀███
███ █▄▄▄█ █▄█ ▄▄█ ▄ ▀██▄▀ █ █▀ ▄▄█ ▀▀    █▀████
███▄▄▄▄▄▄▄█▄▄▄███▄▄▄██▄██▄██▄▄▄▄▄▄▄█▄▄█▄▄▄█████
███████████████████████████████████████████████
";
        assert!(text_qr.contains(expected.trim()));
    }

    #[test]
    fn test_address_to_uri_qr() {
        let address = Address::from_str(ADDR).unwrap();
        let uri_qr = address_to_uri_qr(&address, None);
        assert_eq!("data:image/bmp;base64,Qk2GAQAAAAAAAD4AAAAoAAAAKQAAACkAAAABAAEAAAAAAEgBAAAAAgAAAAIAAAIAAAACAAAA////AAAAAAD+45J/bgAAAIIixrH9AAAAuruKnTwAAAC64HVAVYAAALranGz6AAAAgp3no42AAAD+bk45rAAAAACZ9y6JgAAAq6XYVPoAAAC5+LwZfQAAAL6Btr6LAAAAvQvPI7yAAABz18LtB4AAABj8HxAxAAAAjwdm0g0AAADoClVt+IAAAC5L62YSAAAA6A+MkqwAAACvqA72yAAAAG3vH+P7gAAAX0MgTZIAAAAMkp+bbAAAAC9IKrY+AAAAbLUdC/yAAAB+9+J1FwAAAIy2BYgNAAAAzgiOGlYAAACFTiNtPIAAAL7TSP7HAAAAlJw2GryAAAC2P65cmAAAAFWMdwWugAAAviJafr4AAAAA5I8KAAAAAP6qqqq/gAAAgrCi9iCAAAC6tP+proAAALq/k0wugAAAuufmo66AAACCXUYeIIAAAP5B3Sk/gAAA", uri_qr);
    }
}
