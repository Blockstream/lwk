//! Module to handle the formattings of amount of assets given its precision.

use std::num::{ParseIntError, TryFromIntError};

#[derive(thiserror::Error, Debug)]
#[allow(missing_docs)]
pub enum Error {
    #[error("The maximum precision is 8, given {0}")]
    TooPrecise(u8),

    #[error(transparent)]
    Parse(#[from] ParseIntError),

    #[error("There was an overflow in converting the string {0}")]
    Overflow(String),

    #[error(transparent)]
    From(#[from] TryFromIntError),

    #[error("Our precision is {our}, given a string with {given}")]
    StringTooPrecise { our: u8, given: u8 },
}

/// Helper to convert satoshi values of an asset to the value with the given precision and viceversa.
///
/// For example 100 satoshi with precision 2 is "1.00"
#[derive(Debug)]
pub struct Precision(u8);

impl Precision {
    /// Create a new Precision, erroring if the given precision is greater than the allowed maximum (8)
    pub fn new(precision: u8) -> Result<Precision, Error> {
        if precision > 8 {
            Err(Error::TooPrecise(precision))
        } else {
            Ok(Precision(precision))
        }
    }

    /// Convert the given `sats` to the formatted value according to our precision
    ///
    /// ```
    /// # use lwk_common::precision::Precision;
    /// let p = Precision::new(2).unwrap();
    /// assert_eq!(p.sats_to_string(100), "1.00");
    /// ```
    pub fn sats_to_string(&self, sats: i64) -> String {
        let precision = self.0 as usize;
        if precision == 0 {
            return sats.to_string();
        }

        let negative = if sats < 0 { "-" } else { "" };
        let sats = sats.abs().to_string();
        if sats.len() > precision {
            let over = sats.len() - precision;
            format!("{}{}.{}", negative, &sats[..over], &sats[over..])
        } else {
            let missing = precision - sats.len();
            format!("{}0.{}{}", negative, "0".repeat(missing), sats)
        }
    }

    /// Convert the given string with precision to satoshi units.
    ///
    /// ```
    /// # use lwk_common::precision::Precision;
    /// let p = Precision::new(2).unwrap();
    /// assert_eq!(p.string_to_sats("1.00").unwrap(), 100);
    /// assert_eq!(p.string_to_sats("1.0").unwrap(), 100);
    /// assert_eq!(p.string_to_sats("1").unwrap(), 100);
    /// ```
    pub fn string_to_sats(&self, val: &str) -> Result<i64, Error> {
        match val.find('.') {
            Some(idx) => {
                let right_idx: u8 = (val.len() - idx - 1).try_into()?;
                if right_idx > self.0 {
                    return Err(Error::StringTooPrecise {
                        our: self.0,
                        given: right_idx,
                    });
                }

                let without_dot = val.replacen('.', "", 1);

                // We want this function to roundtrip every value accepted by sats_to_string which are i64.
                // Thus, we use i128 because the conversion of this value with the multiplication of the precision
                // may momentarily overflow i64, but return in a valid range with the following division
                let parsed_without_dot = self.inner_convert(&without_dot)?;
                let pow = 10i128.pow(right_idx as u32);
                Ok((parsed_without_dot / pow).try_into()?)
            }
            None => Ok(self.inner_convert(val)?.try_into()?),
        }
    }

    fn inner_convert(&self, val: &str) -> Result<i128, Error> {
        let num: i128 = val.parse()?;
        let pow = 10i128.pow(self.0 as u32);
        num.checked_mul(pow)
            .ok_or_else(|| Error::Overflow(val.to_string()))
    }
}

#[cfg(test)]
mod test {
    use rand::{thread_rng, Rng};

    use super::*;

    fn check_sat_to_str(prec: u8, sats: i64, expected: &str) {
        let prec = Precision::new(prec).unwrap();
        assert_eq!(prec.sats_to_string(sats), expected);
    }

    fn check_str_to_sat(prec: u8, str: &str, expected: i64) {
        let prec = Precision::new(prec).unwrap();
        assert_eq!(prec.string_to_sats(str).unwrap(), expected);
    }

    #[test]
    fn test_fixed() {
        check_sat_to_str(2, 100, "1.00");
        check_sat_to_str(2, -100, "-1.00");
        check_sat_to_str(0, -100, "-100");
        check_sat_to_str(0, 100, "100");
        check_sat_to_str(8, 100, "0.00000100");
        check_sat_to_str(8, 100_000_000, "1.00000000");
        check_sat_to_str(8, -100_000_000, "-1.00000000");

        check_str_to_sat(8, ".1", 10_000_000);
        check_str_to_sat(8, "0.1", 10_000_000);
        check_str_to_sat(8, "0.0", 0);
        check_str_to_sat(8, "01", 100_000_000);
        check_str_to_sat(8, "1.00000000", 100_000_000);
        check_str_to_sat(8, "1.00000001", 100_000_001);
        check_str_to_sat(8, "-1.00000001", -100_000_001);
    }

    #[test]
    fn test_errors() {
        let exp = "The maximum precision is 8, given 9";
        assert_eq!(exp, Precision::new(9).unwrap_err().to_string());

        let p = Precision::new(0).unwrap();
        let over_u64 = (1i128 << 65).to_string();
        let exp = "out of range integral type conversion attempted";
        assert_eq!(exp, p.string_to_sats(&over_u64).unwrap_err().to_string());

        let p = Precision::new(8).unwrap();
        let max = i128::MAX.to_string();
        let exp = "There was an overflow in converting the string 170141183460469231731687303715884105727";
        assert_eq!(exp, p.string_to_sats(&max).unwrap_err().to_string());

        let exp = "invalid digit found in string";
        assert_eq!(exp, p.string_to_sats("0..1").unwrap_err().to_string());

        let exp = "invalid digit found in string";
        assert_eq!(exp, p.string_to_sats("0.1 ").unwrap_err().to_string());

        let p = Precision::new(1).unwrap();
        let exp = "Our precision is 1, given a string with 2";
        assert_eq!(exp, p.string_to_sats("0.01").unwrap_err().to_string());
    }

    #[test]
    fn test_precision_roundtrips() {
        let mut rng = thread_rng();

        for i in 0..8 {
            let p = Precision(i);
            for _ in 0..100 {
                let sats: i64 = rng.gen();
                let sats_string = p.sats_to_string(sats);
                assert_eq!(
                    sats,
                    p.string_to_sats(&sats_string).unwrap(),
                    "precision:{} sats_string:{}",
                    p.0,
                    sats_string
                );
            }
        }
    }
}
