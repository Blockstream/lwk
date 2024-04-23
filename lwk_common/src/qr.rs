use std::string::FromUtf8Error;

use base64::engine::general_purpose;
use elements::Address;

// In case of blech32 addresses, the address is uppercased so that use less QR code space
// we don't prepend uri schema because it seems Green doesn't interpret it and Elements Core uses `bitcoin`
fn address_to_qr_text(address: &Address) -> String {
    match address.payload {
        elements::address::Payload::WitnessProgram { .. } => {
            address.to_string().to_ascii_uppercase()
        }
        _ => address.to_string(),
    }
}

/// Convert the given address in a string representing a QR code to be consumed from a terminal
pub fn address_to_text_qr(address: &Address) -> Result<String, QrError> {
    let address = address_to_qr_text(address);
    let qr_code = qr_code::QrCode::new(address)?;
    Ok(qr_code.to_string(true, 3))
}

#[derive(thiserror::Error, Debug)]
pub enum QrError {
    #[error(transparent)]
    Qr(#[from] qr_code::types::QrError),

    #[error(transparent)]
    Bmp(#[from] qr_code::bmp_monochrome::BmpError),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Utf8(#[from] FromUtf8Error),
}

/// Convert the given elements address to an image uri
///
/// The image format is monocromatic bitmap, encoded in base64 in the uri.
///
/// The text content of the QR doesn't contain a schema
///
/// Without `pixel_per_module` the default is no border, and 1 pixel per module, to be used
/// for example in html: `style="image-rendering: pixelated; border: 20px solid white;"`
pub fn address_to_uri_qr(
    address: &Address,
    pixel_per_module: Option<u8>,
) -> Result<String, QrError> {
    let address = address_to_qr_text(address);
    let qr_code = qr_code::QrCode::new(address)?;
    let mut bmp = qr_code.to_bmp();
    if let Some(pixel_per_module) = pixel_per_module {
        bmp = bmp.add_white_border(2)?.mul(pixel_per_module)?;
    }
    let mut enc = base64::write::EncoderWriter::new(Vec::new(), &general_purpose::STANDARD);

    bmp.write(&mut enc)?;
    let delegate = enc.finish()?;

    let base64 = String::from_utf8(delegate)?;
    Ok(format!("data:image/bmp;base64,{}", base64))
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
        let text_qr = address_to_text_qr(&address).unwrap();
        let expected = "
███████████████████████████████████████████
███████████████████████████████████████████
███ ▄▄▄▄▄ █ ▄▄▀██▄▄▀▄▀███▄▀▀███ █ ▄▄▄▄▄ ███
███ █   █ █▀█▀▀▀▀ ▄▀▀▄▄██ █▄██▄██ █   █ ███
███ █▄▄▄█ ██▄▀  █ ▀▄▀▀█ ██▀▄ ▄▀▄█ █▄▄▄█ ███
███▄▄▄▄▄▄▄█ █ ▀▄█ █ █▄█ █ ▀▄▀▄▀ █▄▄▄▄▄▄▄███
███ ▀ ▄▀ ▄▄█▄▄██▄█▀██ ▄▀ ▄ ▀█▄▀█▀▄██ ▀▄ ███
███▄█▀▄▄█▄  ▀█  ▄██ █▄▄  ▄█▀ ▄█▄ ▄█▀▄█▄▄███
███▄█▄▄█▀▄▄ ▀█▀██ ▄█ ▄▄▀ ▄ ▀▄▄ ▄▄▀▀ ▀▀█ ███
███ ██ ▄█▄▀█ █▄█▀▄▄▄███▀▀█▀▄▀ ▄▄▀█▄▀ ██ ███
███▄█▀█▀▀▄▄ ▀█▀ █ ██▄▀ █ ▀▄ ▄█▄▀▄▄ ▀▀  ▀███
███▀ ▀ ▀ ▄▄▀▄▄▄▀ █▀▀  █ █▀▀ █ ▀█▀ ▄▄▀▄▀████
█████ ▀▀▄▄ █▀▀▄▀ ▀ █▀ ▄▀▄▀▀   ▄▀▀▀▄▄▀   ███
███▀▄▄█▀ ▄  █ ▄▄▄ ▀▄▀▄▀ █▀█▄▀▄▀ █▄ ▄▀▀█▄███
███▄ █ ▄█▄▄▄▄██  ▄█▄▀███▄█ ▀▄▄██  █▀▄▀█▀███
███▀▄ █▀▄▄██ ▄▀█  ▀█▄ ▀▀███▄▀█  ▄█▀▄▀▀█▄███
█████▄▄██▄▄ ▀  ██ ███▄    ▄▄▀▀  ▄▄▄ ▄█ ████
███ ▄▄▄▄▄ █  ▄█   ▀▄▀ ▄▀▀▄▀██▄▄ █▄█ █ █████
███ █   █ █▀▄▄  ▀▀▀ ▀█▄▄▀█▀██ █▄ ▄▄▄ █ ▄███
███ █▄▄▄█ █▄█▄  ██ █▄ ▀█ ▀█▄ █▀▄▀▀ █▄ ▄▀███
███▄▄▄▄▄▄▄█▄███▄█▄█▄███▄▄█▄▄██▄▄█▄▄█▄▄█▄███
███████████████████████████████████████████";
        assert!(text_qr.contains(expected.trim()));
    }

    #[test]
    fn test_address_to_uri_qr() {
        let address = Address::from_str(ADDR).unwrap();
        let uri_qr = address_to_uri_qr(&address, None).unwrap();
        assert_eq!(uri_qr, "data:image/bmp;base64,Qk1mAQAAAAAAAD4AAAAoAAAAJQAAACUAAAABAAEAAAAAACgBAAAAAgAAAAIAAAIAAAACAAAA////AAAAAAD+io2baAAAAIIZNlcoAAAAurlkyXAAAAC6n8UkUAAAALp4mC/YAAAAgs9tCKAAAAD+7rI6oAAAAADyHniQAAAAM7I/n9AAAACoVzhZYAAAAGZmYJyIAAAAUAxBhqgAAADbzoVmQAAAAI2jWllgAAAAZ76oq4gAAAA5b2vueAAAACcVNPG4AAAA/I3rtlAAAABXdGijoAAAACzaNon4AAAAg4pV1zAAAACRRA1kyAAAAJpTgLlIAAAABNJNk+gAAACzg3d8iAAAACHYjMSAAAAAm5y+blgAAADsAS2UaAAAALdkNyJYAAAAALKLWAAAAAD+qqqr+AAAAII7aVIIAAAAulqI6ugAAAC6vsIC6AAAALoDMpLoAAAAgpChiggAAAD+40IL+AAAAA==");

        let uri_qr = address_to_uri_qr(&address, Some(4)).unwrap();
        assert_eq!(uri_qr, "data:image/bmp;base64,Qk2eDwAAAAAAAD4AAAAoAAAApAAAAKQAAAABAAEAAAAAAGAPAAAAAgAAAAIAAAIAAAACAAAA////AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA////8PAA8PDwAP8P8A/w/w/w8AAAAAAA////8PAA8PDwAP8P8A/w/w/w8AAAAAAA////8PAA8PDwAP8P8A/w/w/w8AAAAAAA////8PAA8PDwAP8P8A/w/w/w8AAAAAAA8AAA8AAP8A8A/w/wDw8P/wDw8AAAAAAA8AAA8AAP8A8A/w/wDw8P/wDw8AAAAAAA8AAA8AAP8A8A/w/wDw8P/wDw8AAAAAAA8AAA8AAP8A8A/w/wDw8P/wDw8AAAAAAA8P/w8PD/8A8P8A8A/wDwDw//AAAAAAAA8P/w8PD/8A8P8A8A/wDwDw//AAAAAAAA8P/w8PD/8A8P8A8A/wDwDw//AAAAAAAA8P/w8PD/8A8P8A8A/wDwDw//AAAAAAAA8P/w8PAP////AA8PAPAPAA8PAAAAAAAA8P/w8PAP////AA8PAPAPAA8PAAAAAAAA8P/w8PAP////AA8PAPAPAA8PAAAAAAAA8P/w8PAP////AA8PAPAPAA8PAAAAAAAA8P/w8A//8ADwD/AAAPD///8P8AAAAAAA8P/w8A//8ADwD/AAAPD///8P8AAAAAAA8P/w8A//8ADwD/AAAPD///8P8AAAAAAA8P/w8A//8ADwD/AAAPD///8P8AAAAAAA8AAA8P8A//8P8P8PAADwAPDwAAAAAAAA8AAA8P8A//8P8P8PAADwAPDwAAAAAAAA8AAA8P8A//8P8P8PAADwAPDwAAAAAAAA8AAA8P8A//8P8P8PAADwAPDwAAAAAAAA////8P/w//Dw/wDwAP/w8PDwAAAAAAAA////8P/w//Dw/wDwAP/w8PDwAAAAAAAA////8P/w//Dw/wDwAP/w8PDwAAAAAAAA////8P/w//Dw/wDwAP/w8PDwAAAAAAAAAAAAAP//APAAD//wD//wAPAPAAAAAAAAAAAAAP//APAAD//wD//wAPAPAAAAAAAAAAAAAP//APAAD//wD//wAPAPAAAAAAAAAAAAAP//APAAD//wD//wAPAPAAAAAAAAAP8A//D/APAA////8A////8PAAAAAAAAAP8A//D/APAA////8A////8PAAAAAAAAAP8A//D/APAA////8A////8PAAAAAAAAAP8A//D/APAA////8A////8PAAAAAAAA8PDwAA8PD/8A//AADw/wDw/wAAAAAAAA8PDwAA8PD/8A//AADw/wDw/wAAAAAAAA8PDwAA8PD/8A//AADw/wDw/wAAAAAAAA8PDwAA8PD/8A//AADw/wDw/wAAAAAAAAD/AP8A/wD/AP8AAA8A//APAA8AAAAAAAD/AP8A/wD/AP8AAA8A//APAA8AAAAAAAD/AP8A/wD/AP8AAA8A//APAA8AAAAAAAD/AP8A/wD/AP8AAA8A//APAA8AAAAAAADw8AAAAA/wAPAAAP8AAP8PDw8AAAAAAADw8AAAAA/wAPAAAP8AAP8PDw8AAAAAAADw8AAAAA/wAPAAAP8AAP8PDw8AAAAAAADw8AAAAA/wAPAAAP8AAP8PDw8AAAAAAA/w/w//8A//DwAA8PD/AP8A8AAAAAAAAA/w/w//8A//DwAA8PD/AP8A8AAAAAAAAA/w/w//8A//DwAA8PD/AP8A8AAAAAAAAA/w/w//8A//DwAA8PD/AP8A8AAAAAAAAA8AD/D/DwAP8PD/DwDw/wDw/wAAAAAAAA8AD/D/DwAP8PD/DwDw/wDw/wAAAAAAAA8AD/D/DwAP8PD/DwDw/wDw/wAAAAAAAA8AD/D/DwAP8PD/DwDw/wDw/wAAAAAAAAD/AP//D///Dw8PAA8PDw//AA8AAAAAAAD/AP//D///Dw8PAA8PDw//AA8AAAAAAAD/AP//D///Dw8PAA8PDw//AA8AAAAAAAD/AP//D///Dw8PAA8PDw//AA8AAAAAAAAP/wDw/w//8P8PD///D/8A//8AAAAAAAAP/wDw/w//8P8PD///D/8A//8AAAAAAAAP/wDw/w//8P8PD///D/8A//8AAAAAAAAP/wDw/w//8P8PD///D/8A//8AAAAAAAAPAP/wAPDw8A/w8A//8AD/D/8AAAAAAAAPAP/wAPDw8A/w8A//8AD/D/8AAAAAAAAPAP/wAPDw8A/w8A//8AD/D/8AAAAAAAAPAP/wAPDw8A/w8A//8AD/D/8AAAAAAA////APAA/w//8PD/8P8P8A8PAAAAAAAA////APAA/w//8PD/8P8P8A8PAAAAAAAA////APAA/w//8PD/8P8P8A8PAAAAAAAA////APAA/w//8PD/8P8P8A8PAAAAAAAADw8P/w//DwAP8PAA8PAA//DwAAAAAAAADw8P/w//DwAP8PAA8PAA//DwAAAAAAAADw8P/w//DwAP8PAA8PAA//DwAAAAAAAADw8P/w//DwAP8PAA8PAA//DwAAAAAAAAAPD/AP8P8PAA/w/w8ADwD///8AAAAAAAAPD/AP8P8PAA/w/w8ADwD///8AAAAAAAAPD/AP8P8PAA/w/w8ADwD///8AAAAAAAAPD/AP8P8PAA/w/w8ADwD///8AAAAAAA8AAA//AA8PAPDw8P/w8P/wD/AAAAAAAA8AAA//AA8PAPDw8P/w8P/wD/AAAAAAAA8AAA//AA8PAPDw8P/w8P/wD/AAAAAAAA8AAA//AA8PAPDw8P/w8P/wD/AAAAAAAA8A8ADw8ADwAAAP8PD/APAP8A8AAAAAAA8A8ADw8ADwAAAP8PD/APAP8A8AAAAAAA8A8ADw8ADwAAAP8PD/APAP8A8AAAAAAA8A8ADw8ADwAAAP8PD/APAP8A8AAAAAAA8A/w8A8PAP/wAAAA8P/wDw8A8AAAAAAA8A/w8A8PAP/wAAAA8P/wDw8A8AAAAAAA8A/w8A8PAP/wAAAA8P/wDw8A8AAAAAAA8A/w8A8PAP/wAAAA8P/wDw8A8AAAAAAAAAAPAP8PAPAPAP8P8A8A///w8AAAAAAAAAAPAP8PAPAPAP8P8A8A///w8AAAAAAAAAAPAP8PAPAPAP8P8A8A///w8AAAAAAAAAAPAP8PAPAPAP8P8A8A///w8AAAAAAA8P8A//AAAP8P/w//D///APAA8AAAAAAA8P8A//AAAP8P/w//D///APAA8AAAAAAA8P8A//AAAP8P/w//D///APAA8AAAAAAA8P8A//AAAP8P/w//D///APAA8AAAAAAAAPAAD/8P8ADwAP8A/wAPAPAAAAAAAAAAAPAAD/8P8ADwAP8A/wAPAPAAAAAAAAAAAPAAD/8P8ADwAP8A/wAPAPAAAAAAAAAAAPAAD/8P8ADwAP8A/wAPAPAAAAAAAAAA8A/w//AP/wDw///wD/D/8A8P8AAAAAAA8A/w//AP/wDw///wD/D/8A8P8AAAAAAA8A/w//AP/wDw///wD/D/8A8P8AAAAAAA8A/w//AP/wDw///wD/D/8A8P8AAAAAAA//D/AAAAAA8A8P8P8A8PAA/w8AAAAAAA//D/AAAAAA8A8P8P8A8PAA/w8AAAAAAA//D/AAAAAA8A8P8P8A8PAA/w8AAAAAAA//D/AAAAAA8A8P8P8A8PAA/w8AAAAAAA8P8P/w/wDwAA/w//APAA8A8P8AAAAAAA8P8P/w/wDwAA/w//APAA8A8P8AAAAAAA8P8P/w/wDwAA/w//APAA8A8P8AAAAAAA8P8P/w/wDwAA/w//APAA8A8P8AAAAAAAAAAAAPD/APDwAPD/Dw/wAAAAAAAAAAAAAAAAAPD/APDwAPD/Dw/wAAAAAAAAAAAAAAAAAPD/APDwAPD/Dw/wAAAAAAAAAAAAAAAAAPD/APDwAPD/Dw/wAAAAAAAAAAAA////8PDw8PDw8PDw8PDw////8AAAAAAA////8PDw8PDw8PDw8PDw////8AAAAAAA////8PDw8PDw8PDw8PDw////8AAAAAAA////8PDw8PDw8PDw8PDw////8AAAAAAA8AAA8AD/8P8P8PAPDw8A8AAA8AAAAAAA8AAA8AD/8P8P8PAPDw8A8AAA8AAAAAAA8AAA8AD/8P8P8PAPDw8A8AAA8AAAAAAA8AAA8AD/8P8P8PAPDw8A8AAA8AAAAAAA8P/w8A8P8PDwAPAA//Dw8P/w8AAAAAAA8P/w8A8P8PDwAPAA//Dw8P/w8AAAAAAA8P/w8A8P8PDwAPAA//Dw8P/w8AAAAAAA8P/w8A8P8PDwAPAA//Dw8P/w8AAAAAAA8P/w8PD///D/AADwAAAA8P/w8AAAAAAA8P/w8PD///D/AADwAAAA8P/w8AAAAAAA8P/w8PD///D/AADwAAAA8P/w8AAAAAAA8P/w8PD///D/AADwAAAA8P/w8AAAAAAA8P/w8AAAAP8A/wDw8A8A8P/w8AAAAAAA8P/w8AAAAP8A/wDw8A8A8P/w8AAAAAAA8P/w8AAAAP8A/wDw8A8A8P/w8AAAAAAA8P/w8AAAAP8A/wDw8A8A8P/w8AAAAAAA8AAA8PAPAADw8AAP8ADw8AAA8AAAAAAA8AAA8PAPAADw8AAP8ADw8AAA8AAAAAAA8AAA8PAPAADw8AAP8ADw8AAA8AAAAAAA8AAA8PAPAADw8AAP8ADw8AAA8AAAAAAA////8P/wAP8PAADwAADw////8AAAAAAA////8P/wAP8PAADwAADw////8AAAAAAA////8P/wAP8PAADwAADw////8AAAAAAA////8P/wAP8PAADwAADw////8AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=");
    }
}
