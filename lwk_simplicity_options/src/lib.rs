use simplicityhl::num::U256;
use simplicityhl::str::WitnessName;
use simplicityhl::value::ValueConstructible;
use simplicityhl::{Arguments, Value};
use std::collections::HashMap;

fn xonly_public_key(_secret_key: u32) -> U256 {
    let bytes = [2; 32];
    U256::from_byte_array(bytes)
}

/// Test function that calls some SimplicityHL functions
pub fn test_args() -> String {
    let args = Arguments::from(HashMap::from([(
        WitnessName::from_str_unchecked("PUBLIC_KEY"),
        Value::u256(xonly_public_key(1)),
    )]));
    args.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simplicityhl() {
        let args = test_args();
        println!("{args}");
        assert!(args.contains("PUBLIC_KEY: u256 = 0x0202020202020202020202020202020202020202020202020202020202020202;"));
    }
}
