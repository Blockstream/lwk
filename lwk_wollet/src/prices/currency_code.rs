// ISC License (ISC)
//
// Copyright (c) 2016, Zeyla Hellyer <zeylahellyer@gmail.com>
//
// Permission to use, copy, modify, and/or distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY
// SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES WHATSOEVER
// RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN ACTION OF
// CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR IN
// CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
//
// What is ISO 4217?
//
// | ISO 4217 is a standard published by the International Organization for
// | Standardization, which delineates currency designators, country codes
// | (alpha and numeric), and references to minor units in three tables.
// |
// | - [Wikipedia](http://en.wikipedia.org/wiki/ISO_4217)
//
// Originally by zeyla on GitHub.

pub use super::codes::all;

/// Data for each Currency Code defined by ISO 4217.
#[derive(Clone, Debug)]
pub struct CurrencyCode {
    /// 3-letter code of the currency
    pub alpha3: &'static str,
    /// Vector of Alpha2 codes for the countries that use the currency
    pub countries: &'static [&'static str],
    /// Number of decimals
    pub exp: i8,
    /// Fully readable and used name
    pub name: &'static str,
    /// Assigned 3-digit numeric code
    pub num: &'static str,
}

/// Returns the CurrencyCode with the given Alpha3 code, if one exists.
pub fn alpha3(alpha3: &str) -> Option<&'static CurrencyCode> {
    all().iter().find(|c| c.alpha3 == alpha3)
}
