use std::string::FromUtf8Error;

use base64::engine::general_purpose;
use elements::{Address, AddressParams};

// In case of blech32 addresses, the address is uppercased so that use less QR code space
fn address_to_qr_text(address: &Address) -> String {
    // TODO gdk use also `liquidtestnet` as schema, I don't think it's right but it may be already adopted.
    // verify it and consider to add that or to remove this comment
    let address_string = match address.payload {
        elements::address::Payload::WitnessProgram { .. } => {
            address.to_string().to_ascii_uppercase()
        }
        _ => address.to_string(),
    };
    if address.params == &AddressParams::LIQUID_TESTNET {
        format!("liquidtestnet:{address_string}")
    } else {
        format!("liquidnetwork:{address_string}")
    }
}

/// Convert the given address in a string representing a QR code to be consumed from a terminal
pub fn address_to_text_qr(address: &Address) -> Result<String, QrError> {
    let address = address_to_qr_text(address);
    let qr_code = qr_code::QrCode::new(address)?;
    Ok(qr_code.to_string(true, 3))
}

#[derive(thiserror::Error, Debug)]
#[allow(missing_docs)]
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

/// Convert the given elements address to a QR code image uri
///
/// The image format is monocromatic bitmap, encoded in base64 in the uri.
///
/// The text content of the QR doesn't contain a schema
///
/// Without `pixel_per_module` the default is no border, and 1 pixel per module, to be used
/// for example in html: `style="image-rendering: pixelated; border: 20px solid white;"`
pub fn address_to_qr(address: &Address, pixel_per_module: Option<u8>) -> Result<String, QrError> {
    let address = address_to_qr_text(address);
    string_to_qr(&address, pixel_per_module)
}

/// Convert the given string to a QR code image uri
///
/// The image format is monocromatic bitmap, encoded in base64 in the uri.
///
/// Without `pixel_per_module` the default is no border, and 1 pixel per module, to be used
/// for example in html: `style="image-rendering: pixelated; border: 20px solid white;"`
pub fn string_to_qr(str: &str, pixel_per_module: Option<u8>) -> Result<String, QrError> {
    let qr_code = qr_code::QrCode::new(str)?;
    let mut bmp = qr_code.to_bmp();
    if let Some(pixel_per_module) = pixel_per_module {
        bmp = bmp.add_white_border(2)?;
        if pixel_per_module > 1 {
            bmp = bmp.mul(pixel_per_module)?;
        }
    }
    let mut enc = base64::write::EncoderWriter::new(Vec::new(), &general_purpose::STANDARD);

    bmp.write(&mut enc)?;
    let delegate = enc.finish()?;

    let base64 = String::from_utf8(delegate)?;
    Ok(format!("data:image/bmp;base64,{base64}"))
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
███████████████████████████████████████████████
███████████████████████████████████████████████
███ ▄▄▄▄▄ █▀█▀▄▀█▀█▄▄ ▄ ▀█ ▀▄▀▀█▀▄ ▀█ ▄▄▄▄▄ ███
███ █   █ ██▀▄█ ▄ ▄█  █▄▀█▄   ▀▀▀█ ▀█ █   █ ███
███ █▄▄▄█ █▄ ▀█ ▀  ▄  ▀█▀█ ▀█▄▀█▀▀█▀█ █▄▄▄█ ███
███▄▄▄▄▄▄▄█▄▀▄▀ █ ▀▄█ ▀▄▀ ▀▄▀ ▀▄▀▄█ █▄▄▄▄▄▄▄███
███▄▀ ▀ █▄▀▀███ ▀ ███▄ ▀▀▀██  █▄ ▀▄▀▀▀█▄█▀ ▀███
███ ▀   ▀▄ ▄▀▀▀▄▄█ ▄▄██▄██▄▄█▄██▄▄▀▀ ▄▄▄█▄ ████
███▄█▄▄ ▀▄▀ ▄▄▄▀▀▄▀ █▄▀█▀█▀  █ █▄▄▀ ▀▄ ▄█▀█ ███
███▀▄▄█▀ ▄▀█▀█ ▄▄███▀ ▀█ ▄▄   ▀█▀▄▄ ██ ██ ▄▄███
███ █▀   ▄▀▀ ▀▄ ██▄▀▀▀█▀█ ▄█ ▄█▄█▀▄█  ██   ▀███
███▄▄▄ ▀▄▄█▀▀█  ▀ ▀▄▀ ▄▀▄█ ▄▀   ▄█ ▄▀▀▄▄▀▀█████
███▄ █  ▀▄▄▀▄██ █  ██ ▄▀▀█▀▄▀ █▀▀█▄ █   ▄ ▀▀███
███▀ ▀▀▀ ▄▀ ▀▀▄██▀ █▀ █▀█▀  ▄▀▄█ ▀▀█▀ ████▄▄███
████▄ ▄▄ ▄▄ ▄▀▄▄█  █ ▄   ▄▄▀▀▀▄ ▄█▄ █  ▄▀ ▄▀███
███ ▄▄ ▄█▄ ▀▄█▄▀ ███▀█ ▄ ▀█▄▀▀█▄▀██  ▀▄█▄▄▀████
█████ ▄ ▀▄▀▀▄ ▀▄█▀▀▄▀ ▀   ▀██ ▀ ▀  █ █▄▄▄█▄▄███
███▄▀  █ ▄██▀▀ █▄ █▄  ▄ ██▀█ ▄▄ ▀ ▀█ ▄▄▄▀ ▀▄███
███▄█▄▄██▄▄▀▀▀▀▀▀▀ █▄█▄▄ ▄▀▄▄▀▄▀ ██ ▄▄▄ ▀██▄███
███ ▄▄▄▄▄ ██▄▀▄█  ▀▄▄█▄█▀ ██▄█ ▄▀ ▄ █▄█ ▀▀▀ ███
███ █   █ █▄█ ▄ ▄▀█▄▀█▄▀█ ▀█▀██▄█ ▀▄▄▄▄ ▄▄ ████
███ █▄▄▄█ █▄█▀▄█ ▄▀▀▄█ ▀▀ ▄▄█ ▄  ▀█ ▀█▄▄▄▄▀▄███
███▄▄▄▄▄▄▄█▄▄█▄▄▄▄▄█▄▄███▄███▄▄█▄██▄███▄▄▄▄▄███
███████████████████████████████████████████████";
        assert!(text_qr.contains(expected.trim()));
    }

    #[test]
    fn test_address_to_uri_qr() {
        let address = Address::from_str(ADDR).unwrap();
        let uri_qr = address_to_qr(&address, None).unwrap();
        assert_eq!(uri_qr, "data:image/bmp;base64,Qk2GAQAAAAAAAD4AAAAoAAAAKQAAACkAAAABAAEAAAAAAEgBAAAAAgAAAAIAAAIAAAACAAAA////AAAAAAD+32I0j4AAAIIlni7BAAAAupZTvJ6AAAC6KktDCQAAALq8kgr/AAAAgicGFo+AAAD+VtJbqIAAAAD/BSyMAAAAswFe1PiAAAB0cmlPRwAAALYW+Hp6gAAALbN/P0AAAAA6aK4rXYAAAJGMVmThAAAA+1QciNYAAAAko1zotoAAAH/bfx27AAAA/eNrp2AAAABGkSHUIYAAAFyLLWy7gAAA20swob4AAAAY32l5ZgAAAPYatb2YAAAAvejqQmeAAACeWQNpZwAAAI1QdPSSAAAAZhwn45OAAAANjZXR0oAAALryoNa4gAAA/XEAAcEAAAC7jcmmewAAAHmOHmbjgAAAqgowbQkAAAAAWzd0gAAAAP6qqqq/gAAAgm91lqCAAAC6y+EgLoAAALpKZP2ugAAAui9p4S6AAACCqi21oIAAAP4Q+UM/gAAA");

        let uri_qr = address_to_qr(&address, Some(4)).unwrap();
        assert_eq!(uri_qr, "data:image/bmp;base64,Qk0eEQAAAAAAAD4AAAAoAAAAtAAAALQAAAABAAEAAAAAAOAQAAAAAgAAAAIAAAIAAAACAAAA////AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA////8P8P//8P8ADwAP8PAPAA///wAAAA////8P8P//8P8ADwAP8PAPAA///wAAAA////8P8P//8P8ADwAP8PAPAA///wAAAA////8P8P//8P8ADwAP8PAPAA///wAAAA8AAA8ADwDw/wD//wAPD/8P8AAA8AAAAA8AAA8ADwDw/wD//wAPD/8P8AAA8AAAAA8AAA8ADwDw/wD//wAPD/8P8AAA8AAAAA8AAA8ADwDw/wD//wAPD/8P8AAA8AAAAA8P/w8PAPD/APDwD/8P//APAP//DwAAAA8P/w8PAPD/APDwD/8P//APAP//DwAAAA8P/w8PAPD/APDwD/8P//APAP//DwAAAA8P/w8PAPD/APDwD/8P//APAP//DwAAAA8P/w8ADw8PAPAPD/DwAA/wAA8A8AAAAA8P/w8ADw8PAPAPD/DwAA/wAA8A8AAAAA8P/w8ADw8PAPAPD/DwAA/wAA8A8AAAAA8P/w8ADw8PAPAPD/DwAA/wAA8A8AAAAA8P/w8PD//wDwDwDwAADw8P////8AAAAA8P/w8PD//wDwDwDwAADw8P////8AAAAA8P/w8PD//wDwDwDwAADw8P////8AAAAA8P/w8PD//wDwDwDwAADw8P////8AAAAA8AAA8ADwD/8AAA/wAA8P8PAA///wAAAA8AAA8ADwD/8AAA/wAA8P8PAA///wAAAA8AAA8ADwD/8AAA/wAA8P8PAA///wAAAA8AAA8ADwD/8AAA/wAA8P8PAA///wAAAA////8A8PD/D/DwDwDw/w//Dw8ADwAAAA////8A8PD/D/DwDwDw/w//Dw8ADwAAAA////8A8PD/D/DwDwDw/w//Dw8ADwAAAA////8A8PD/D/DwDwDw/w//Dw8ADwAAAAAAAAAP////8AAA8PAPD/APAA/wAAAAAAAAAAAP////8AAA8PAPD/APAA/wAAAAAAAAAAAP////8AAA8PAPD/APAA/wAAAAAAAAAAAP////8AAA8PAPD/APAA/wAAAAAA8P8A/wAAAA8PD//w/w8PAP//8ADwAAAA8P8A/wAAAA8PD//w/w8PAP//8ADwAAAA8P8A/wAAAA8PD//w/w8PAP//8ADwAAAA8P8A/wAAAA8PD//w/w8PAP//8ADwAAAAD/8PAA//APAP8PAPDwD//w8AD/8AAAAAD/8PAA//APAP8PAPDwD//w8AD/8AAAAAD/8PAA//APAP8PAPDwD//w8AD/8AAAAAD/8PAA//APAP8PAPDwD//w8AD/8AAAAA8P8P8AAPD/D///AAD//w8A//8PDwAAAA8P8P8AAPD/D///AAD//w8A//8PDwAAAA8P8P8AAPD/D///AAD//w8A//8PDwAAAA8P8P8AAPD/D///AAD//w8A//8PDwAAAAAPD/D/D/AP8P////AP///w8AAAAAAAAAAPD/D/D/AP8P////AP///w8AAAAAAAAAAPD/D/D/AP8P////AP///w8AAAAAAAAAAPD/D/D/AP8P////AP///w8AAAAAAAAAAP/w8A/w8ADw8P/wAPDw/w8P/w/wAAAAAP/w8A/w8ADw8P/wAPDw/w8P/w/wAAAAAP/w8A/w8ADw8P/wAPDw/w8P/w/wAAAAAP/w8A/w8ADw8P/wAPDw/w8P/w/wAAAA8A8AD/AA/wAPDw/wD/APAP/wAA8AAAAA8A8AD/AA/wAPDw/wD/APAP/wAA8AAAAA8A8AD/AA/wAPDw/wD/APAP/wAA8AAAAA8A8AD/AA/wAPDw/wD/APAP/wAA8AAAAA///w/w8PDwAAD/8A8ADwAP8PD/AAAAAA///w/w8PDwAAD/8A8ADwAP8PD/AAAAAA///w/w8PDwAAD/8A8ADwAP8PD/AAAAAA///w/w8PDwAAD/8A8ADwAP8PD/AAAAAAAPAPAPDwAP8PD/8A//DwAPD/D/DwAAAAAPAPAPDwAP8PD/8A//DwAPD/D/DwAAAAAPAPAPDwAP8PD/8A//DwAPD/D/DwAAAAAPAPAPDwAP8PD/8A//DwAPD/D/DwAAAAD/////8P8P8P////AA//D/D/8P8AAAAAD/////8P8P8P////AA//D/D/8P8AAAAAD/////8P8P8P////AA//D/D/8P8AAAAAD/////8P8P8P////AA//D/D/8P8AAAAA////D//wAP8P8PD/8PAP/w/wAAAAAAAA////D//wAP8P8PD/8PAP/w/wAAAAAAAA////D//wAP8P8PD/8PAP/w/wAAAAAAAA////D//wAP8P8PD/8PAP/w/wAAAAAAAADwAP8PAPAA8A8AAP/w8PAADwAA/wAAAADwAP8PAPAA8A8AAP/w8PAADwAA/wAAAADwAP8PAPAA8A8AAP/w8PAADwAA/wAAAADwAP8PAPAA8A8AAP/w8PAADwAA/wAAAADw//APAA8P8A8P8PD/D/APD/8P/wAAAADw//APAA8P8A8P8PD/D/APD/8P/wAAAADw//APAA8P8A8P8PD/D/APD/8P/wAAAADw//APAA8P8A8P8PD/D/APD/8P/wAAAA/w/w/w8A8P8A/wAA8PAAD/D///AAAAAA/w/w/w8A8P8A/wAA8PAAD/D///AAAAAA/w/w/w8A8P8A/wAA8PAAD/D///AAAAAA/w/w/w8A8P8A/wAA8PAAD/D///AAAAAAAA/wAP8P//8P8PAPD//wDw/wD/AAAAAAAA/wAP8P//8P8PAPD//wDw/wD/AAAAAAAA/wAP8P//8P8PAPD//wDw/wD/AAAAAAAA/wAP8P//8P8PAPD//wDw/wD/AAAAAA//8P8AAP8PDw/w8P8P//D/AP8AAAAAAA//8P8AAP8PDw/w8P8P//D/AP8AAAAAAA//8P8AAP8PDw/w8P8P//D/AP8AAAAAAA//8P8AAP8PDw/w8P8P//D/AP8AAAAAAA8P//D//w8AD/8PDwDwAA8A/wD//wAAAA8P//D//w8AD/8PDwDwAA8A/wD//wAAAA8P//D//w8AD/8PDwDwAA8A/wD//wAAAA8P//D//w8AD/8PDwDwAA8A/wD//wAAAA8A//8A8P8A8AAAD/D/DwDw/wD/8AAAAA8A//8A8P8A8AAAD/D/DwDw/wD/8AAAAA8A//8A8P8A8AAAD/D/DwDw/wD/8AAAAA8A//8A8P8A8AAAD/D/DwDw/wD/8AAAAA8AD/Dw8PAAAP/w8A//8PAPAPAPAAAAAA8AD/Dw8PAAAP/w8A//8PAPAPAPAAAAAA8AD/Dw8PAAAP/w8A//8PAPAPAPAAAAAA8AD/Dw8PAAAP/w8A//8PAPAPAPAAAAAAD/AP8AAP/wAA8A////AA//APAP/wAAAAD/AP8AAP/wAA8A////AA//APAP/wAAAAD/AP8AAP/wAA8A////AA//APAP/wAAAAD/AP8AAP/wAA8A////AA//APAP/wAAAAAAD/D/AA/w/wDw8P/w8AD/8PAPDwAAAAAAD/D/AA/w/wDw8P/w8AD/8PAPDwAAAAAAD/D/AA/w/wDw8P/w8AD/8PAPDwAAAAAAD/D/AA/w/wDw8P/w8AD/8PAPDwAAAA8P/w8P//APDw8AAA/w8P8PD/8ADwAAAA8P/w8P//APDw8AAA/w8P8PD/8ADwAAAA8P/w8P//APDw8AAA/w8P8PD/8ADwAAAA8P/w8P//APDw8AAA/w8P8PD/8ADwAAAA////Dw//AA8AAAAAAAAAD/8AAA8AAAAA////Dw//AA8AAAAAAAAAD/8AAA8AAAAA////Dw//AA8AAAAAAAAAD/8AAA8AAAAA////Dw//AA8AAAAAAAAAD/8AAA8AAAAA8P/w//AA/w//APAP8PAP8A//8P8AAAAA8P/w//AA/w//APAP8PAP8A//8P8AAAAA8P/w//AA/w//APAP8PAP8A//8P8AAAAA8P/w//AA/w//APAP8PAP8A//8P8AAAAAD//wD/AA//AAD//wD/AP8P/wAP/wAAAAD//wD/AA//AAD//wD/AP8P/wAP/wAAAAD//wD/AA//AAD//wD/AP8P/wAP/wAAAAD//wD/AA//AAD//wD/AP8P/wAP/wAAAA8PDw8AAA8PAA/wAAD/D/DwAA8A8AAAAA8PDw8AAA8PAA/wAAD/D/DwAA8A8AAAAA8PDw8AAA8PAA/wAAD/D/DwAA8A8AAAAA8PDw8AAA8PAA/wAAD/D/DwAA8A8AAAAAAAAAAA8P8P8A/w//D/8PAPAAAAAAAAAAAAAAAA8P8P8A/w//D/8PAPAAAAAAAAAAAAAAAA8P8P8A/w//D/8PAPAAAAAAAAAAAAAAAA8P8P8A/w//D/8PAPAAAAAAAAAA////8PDw8PDw8PDw8PDw8PD////wAAAA////8PDw8PDw8PDw8PDw8PD////wAAAA////8PDw8PDw8PDw8PDw8PD////wAAAA////8PDw8PDw8PDw8PDw8PD////wAAAA8AAA8A/w//8P/w8P8A8P8PDwAADwAAAA8AAA8A/w//8P/w8P8A8P8PDwAADwAAAA8AAA8A/w//8P/w8P8A8P8PDwAADwAAAA8AAA8A/w//8P/w8P8A8P8PDwAADwAAAA8P/w8P8A8P//8AAPAPAAAADw//DwAAAA8P/w8P8A8P//8AAPAPAAAADw//DwAAAA8P/w8P8A8P//8AAPAPAAAADw//DwAAAA8P/w8P8A8P//8AAPAPAAAADw//DwAAAA8P/w8A8A8PAP8A8A////D/Dw//DwAAAA8P/w8A8A8PAP8A8A////D/Dw//DwAAAA8P/w8A8A8PAP8A8A////D/Dw//DwAAAA8P/w8A8A8PAP8A8A////D/Dw//DwAAAA8P/w8ADw//8P8PAP//AADwDw//DwAAAA8P/w8ADw//8P8PAP//AADwDw//DwAAAA8P/w8ADw//8P8PAP//AADwDw//DwAAAA8P/w8ADw//8P8PAP//AADwDw//DwAAAA8AAA8PDw8PAA8P8P8P8PD/DwAADwAAAA8AAA8PDw8PAA8P8P8P8PD/DwAADwAAAA8AAA8PDw8PAA8P8P8P8PD/DwAADwAAAA8AAA8PDw8PAA8P8P8P8PD/DwAADwAAAA////8AAPAAD///APDwAA/wD////wAAAA////8AAPAAD///APDwAA/wD////wAAAA////8AAPAAD///APDwAA/wD////wAAAA////8AAPAAD///APDwAA/wD////wAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=");
    }

    #[test]
    fn test_address_to_qr_text() {
        // Test mainnet address (bech32, should be uppercased)
        let mainnet_addr = Address::from_str("lq1qqvp9g33gw9y05xava3dvcpq8pnkv82yj3tdnzp547eyp9yrztz2lkyxrhscd55ev4p7lj2n72jtkn5u4xnj4v577c42jhf3ww").unwrap();
        let mainnet_qr_text = address_to_qr_text(&mainnet_addr);
        assert_eq!(mainnet_qr_text, "liquidnetwork:LQ1QQVP9G33GW9Y05XAVA3DVCPQ8PNKV82YJ3TDNZP547EYP9YRZTZ2LKYXRHSCD55EV4P7LJ2N72JTKN5U4XNJ4V577C42JHF3WW");

        // Test testnet address (bech32, should be uppercased)
        let testnet_addr = Address::from_str("tlq1qq02egjncr8g4qn890mrw3jhgupwqymekv383lwpmsfghn36hac5ptpmeewtnftluqyaraa56ung7wf47crkn5fjuhk422d68m").unwrap();
        let testnet_qr_text = address_to_qr_text(&testnet_addr);
        assert_eq!(testnet_qr_text, "liquidtestnet:TLQ1QQ02EGJNCR8G4QN890MRW3JHGUPWQYMEKV383LWPMSFGHN36HAC5PTPMEEWTNFTLUQYARAA56UNG7WF47CRKN5FJUHK422D68M");
    }
}
