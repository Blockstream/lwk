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

use crate::prices::currency_code::CurrencyCode;

const ALL: [CurrencyCode; 155] = [
    // Sorted by num.
    CurrencyCode {
        alpha3: "ALL",
        countries: &["AL"],
        exp: 2,
        name: "Albanian lek",
        num: "008",
    },
    CurrencyCode {
        alpha3: "DZD",
        countries: &["DZ"],
        exp: 2,
        name: "Algerian dinar",
        num: "012",
    },
    CurrencyCode {
        alpha3: "ARS",
        countries: &["AR"],
        exp: 2,
        name: "Argentine peso",
        num: "032",
    },
    CurrencyCode {
        alpha3: "AUD",
        countries: &["AU", "CC", "CX", "HM", "KI", "NF", "NR", "TV"],
        exp: 2,
        name: "Australian dollar",
        num: "036",
    },
    CurrencyCode {
        alpha3: "BSD",
        countries: &["BS"],
        exp: 2,
        name: "Bahamian dollar",
        num: "044",
    },
    CurrencyCode {
        alpha3: "BHD",
        countries: &["BH"],
        exp: 2,
        name: "Bahraini dinar",
        num: "048",
    },
    CurrencyCode {
        alpha3: "BDT",
        countries: &["BD"],
        exp: 2,
        name: "Bangladeshi taka",
        num: "050",
    },
    CurrencyCode {
        alpha3: "AMD",
        countries: &["AM"],
        exp: 2,
        name: "Armenian dram",
        num: "051",
    },
    CurrencyCode {
        alpha3: "BBD",
        countries: &["BB"],
        exp: 2,
        name: "Barbados dollar",
        num: "052",
    },
    CurrencyCode {
        alpha3: "BMD",
        countries: &["BM"],
        exp: 2,
        name: "Bermudian dollar",
        num: "060",
    },
    CurrencyCode {
        alpha3: "BTN",
        countries: &["BT"],
        exp: 2,
        name: "Bhutanese ngultrum",
        num: "064",
    },
    CurrencyCode {
        alpha3: "BOB",
        countries: &["BO"],
        exp: 2,
        name: "Boliviano",
        num: "068",
    },
    CurrencyCode {
        alpha3: "BWP",
        countries: &["BW"],
        exp: 2,
        name: "Botswana pula",
        num: "072",
    },
    CurrencyCode {
        alpha3: "BZD",
        countries: &["BZ"],
        exp: 2,
        name: "Belize dollar",
        num: "084",
    },
    CurrencyCode {
        alpha3: "SBD",
        countries: &["SB"],
        exp: 2,
        name: "Soloman Islands dollar",
        num: "090",
    },
    CurrencyCode {
        alpha3: "BND",
        countries: &["BN", "SG"],
        exp: 2,
        name: "Brunei dollar",
        num: "096",
    },
    CurrencyCode {
        alpha3: "MMK",
        countries: &["MM"],
        exp: 2,
        name: "Myanmar kyat",
        num: "104",
    },
    CurrencyCode {
        alpha3: "BIF",
        countries: &["BI"],
        exp: 0,
        name: "Burundian franc",
        num: "108",
    },
    CurrencyCode {
        alpha3: "KHR",
        countries: &["KH"],
        exp: 2,
        name: "Cambodian riel",
        num: "116",
    },
    CurrencyCode {
        alpha3: "CAD",
        countries: &["CA"],
        exp: 2,
        name: "Canadian dollar",
        num: "124",
    },
    CurrencyCode {
        alpha3: "CVE",
        countries: &["CV"],
        exp: 0,
        name: "Cape Verde escudo",
        num: "132",
    },
    CurrencyCode {
        alpha3: "KYD",
        countries: &["KY"],
        exp: 2,
        name: "Cayman Islands dollar",
        num: "136",
    },
    CurrencyCode {
        alpha3: "LKR",
        countries: &["LK"],
        exp: 2,
        name: "Sri Lankan rupee",
        num: "144",
    },
    CurrencyCode {
        alpha3: "CLP",
        countries: &["CL"],
        exp: 0,
        name: "Chilean peso",
        num: "152",
    },
    CurrencyCode {
        alpha3: "CNY",
        countries: &["CN"],
        exp: 2,
        name: "Chinese yuan",
        num: "156",
    },
    CurrencyCode {
        alpha3: "COP",
        countries: &["CO"],
        exp: 2,
        name: "Colombian peso",
        num: "170",
    },
    CurrencyCode {
        alpha3: "KMF",
        countries: &["KM"],
        exp: 0,
        name: "Comoro franc",
        num: "174",
    },
    CurrencyCode {
        alpha3: "CRC",
        countries: &["CR"],
        exp: 2,
        name: "Costa Rican colon",
        num: "188",
    },
    CurrencyCode {
        alpha3: "HRK",
        countries: &["HR"],
        exp: 2,
        name: "Croatian kuna",
        num: "191",
    },
    CurrencyCode {
        alpha3: "CUP",
        countries: &["CU"],
        exp: 2,
        name: "Cuban peso",
        num: "192",
    },
    CurrencyCode {
        alpha3: "CZK",
        countries: &["CZ"],
        exp: 2,
        name: "Czech koruna",
        num: "203",
    },
    CurrencyCode {
        alpha3: "DKK",
        countries: &["DK", "FO", "GL"],
        exp: 2,
        name: "Danish krone",
        num: "208",
    },
    CurrencyCode {
        alpha3: "DOP",
        countries: &["DO"],
        exp: 2,
        name: "Dominican peso",
        num: "214",
    },
    CurrencyCode {
        alpha3: "ETB",
        countries: &["ET"],
        exp: 2,
        name: "Ethiopian birr",
        num: "230",
    },
    CurrencyCode {
        alpha3: "ERN",
        countries: &["ER"],
        exp: 2,
        name: "Eritrean nakfa",
        num: "232",
    },
    CurrencyCode {
        alpha3: "FKP",
        countries: &["FK"],
        exp: 2,
        name: "Falkland Islands pound",
        num: "238",
    },
    CurrencyCode {
        alpha3: "FJD",
        countries: &["FJ"],
        exp: 2,
        name: "Fiji dollar",
        num: "242",
    },
    CurrencyCode {
        alpha3: "DJF",
        countries: &["DJ"],
        exp: 0,
        name: "Djiboutian franc",
        num: "262",
    },
    CurrencyCode {
        alpha3: "GMD",
        countries: &["GM"],
        exp: 2,
        name: "Gambian dalasi",
        num: "270",
    },
    CurrencyCode {
        alpha3: "GIP",
        countries: &["GI"],
        exp: 2,
        name: "Gibraltar pound",
        num: "292",
    },
    CurrencyCode {
        alpha3: "GTQ",
        countries: &["GT"],
        exp: 2,
        name: "Guatemalan quetzal",
        num: "320",
    },
    CurrencyCode {
        alpha3: "GNF",
        countries: &["GN"],
        exp: 0,
        name: "Guinean franc",
        num: "324",
    },
    CurrencyCode {
        alpha3: "GYD",
        countries: &["GY"],
        exp: 2,
        name: "Guyanese dollar",
        num: "328",
    },
    CurrencyCode {
        alpha3: "HTG",
        countries: &["HT"],
        exp: 2,
        name: "Haitian gourde",
        num: "332",
    },
    CurrencyCode {
        alpha3: "HNL",
        countries: &["HN"],
        exp: 2,
        name: "Honduran lempira",
        num: "340",
    },
    CurrencyCode {
        alpha3: "HKD",
        countries: &["HK", "MO"],
        exp: 2,
        name: "Hong Kong dollar",
        num: "344",
    },
    CurrencyCode {
        alpha3: "HUF",
        countries: &["HU"],
        exp: 2,
        name: "Hungarian forint",
        num: "348",
    },
    CurrencyCode {
        alpha3: "ISK",
        countries: &["IS"],
        exp: 0,
        name: "Icelandic króna",
        num: "352",
    },
    CurrencyCode {
        alpha3: "INR",
        countries: &["BT", "IN", "NP", "ZW"],
        exp: 2,
        name: "Indian rupee",
        num: "356",
    },
    CurrencyCode {
        alpha3: "IDR",
        countries: &["ID"],
        exp: 2,
        name: "Indonesian rupiah",
        num: "360",
    },
    CurrencyCode {
        alpha3: "IRR",
        countries: &["IR"],
        exp: 2,
        name: "Iranian rial",
        num: "364",
    },
    CurrencyCode {
        alpha3: "IQD",
        countries: &["IQ"],
        exp: 3,
        name: "Iraqi dinar",
        num: "368",
    },
    CurrencyCode {
        alpha3: "ILS",
        countries: &["IL", "PS"],
        exp: 2,
        name: "Israeli new shekel",
        num: "376",
    },
    CurrencyCode {
        alpha3: "KMD",
        countries: &["JM"],
        exp: 2,
        name: "Jamaican dollar",
        num: "388",
    },
    CurrencyCode {
        alpha3: "JPY",
        countries: &["JP"],
        exp: 0,
        name: "Japanese yen",
        num: "392",
    },
    CurrencyCode {
        alpha3: "KZT",
        countries: &["KZ"],
        exp: 2,
        name: "Kazakhstani tenge",
        num: "398",
    },
    CurrencyCode {
        alpha3: "JOD",
        countries: &["JO"],
        exp: 3,
        name: "Jordanian dinar",
        num: "400",
    },
    CurrencyCode {
        alpha3: "KES",
        countries: &["KE"],
        exp: 2,
        name: "Kenyan shilling",
        num: "404",
    },
    CurrencyCode {
        alpha3: "KPW",
        countries: &["KP"],
        exp: 2,
        name: "North Korean won",
        num: "408",
    },
    CurrencyCode {
        alpha3: "KRW",
        countries: &["KR"],
        exp: 0,
        name: "South Korean won",
        num: "410",
    },
    CurrencyCode {
        alpha3: "KWD",
        countries: &["KW"],
        exp: 3,
        name: "Kuwaiti dinar",
        num: "414",
    },
    CurrencyCode {
        alpha3: "KGS",
        countries: &["KG"],
        exp: 2,
        name: "Kyrgyzstani som",
        num: "417",
    },
    CurrencyCode {
        alpha3: "LAK",
        countries: &["LA"],
        exp: 2,
        name: "Lao kip",
        num: "418",
    },
    CurrencyCode {
        alpha3: "LBP",
        countries: &["LB"],
        exp: 2,
        name: "Lebanese pound",
        num: "422",
    },
    CurrencyCode {
        alpha3: "LSL",
        countries: &["LS"],
        exp: 2,
        name: "Lesotho loti",
        num: "426",
    },
    CurrencyCode {
        alpha3: "LRD",
        countries: &["LR"],
        exp: 2,
        name: "Liberian dollar",
        num: "430",
    },
    CurrencyCode {
        alpha3: "LYD",
        countries: &["LY"],
        exp: 3,
        name: "Libyan dinar",
        num: "434",
    },
    CurrencyCode {
        alpha3: "MOP",
        countries: &["MO"],
        exp: 2,
        name: "Macanese pataca",
        num: "446",
    },
    CurrencyCode {
        alpha3: "MWK",
        countries: &["MW"],
        exp: 2,
        name: "Malawian kwacha",
        num: "454",
    },
    CurrencyCode {
        alpha3: "MYR",
        countries: &["MY"],
        exp: 2,
        name: "Malaysian ringgit",
        num: "458",
    },
    CurrencyCode {
        alpha3: "MVR",
        countries: &["MV"],
        exp: 2,
        name: "Maldivian rufiyaa",
        num: "462",
    },
    CurrencyCode {
        alpha3: "MRO",
        countries: &["MR"],
        exp: 1,
        name: "Mauritanian ouguiya",
        num: "478",
    },
    CurrencyCode {
        alpha3: "MUR",
        countries: &["MU"],
        exp: 2,
        name: "Mauritian rupee",
        num: "480",
    },
    CurrencyCode {
        alpha3: "MXN",
        countries: &["MX"],
        exp: 2,
        name: "Mexican peso",
        num: "484",
    },
    CurrencyCode {
        alpha3: "MNT",
        countries: &["MN"],
        exp: 2,
        name: "Mongolian tögrög",
        num: "496",
    },
    CurrencyCode {
        alpha3: "MDL",
        countries: &["MD"],
        exp: 2,
        name: "Moldovan leu",
        num: "498",
    },
    CurrencyCode {
        alpha3: "MAD",
        countries: &["MA"],
        exp: 2,
        name: "Moroccan dirham",
        num: "504",
    },
    CurrencyCode {
        alpha3: "OMR",
        countries: &["OM"],
        exp: 3,
        name: "Omani rial",
        num: "512",
    },
    CurrencyCode {
        alpha3: "NAD",
        countries: &["NA"],
        exp: 2,
        name: "Namibian dollar",
        num: "516",
    },
    CurrencyCode {
        alpha3: "NPR",
        countries: &["NP"],
        exp: 2,
        name: "Nepalese rupee",
        num: "524",
    },
    CurrencyCode {
        alpha3: "ANG",
        countries: &["CW", "SX"],
        exp: 2,
        name: "Netherlands Antillean guilder",
        num: "532",
    },
    CurrencyCode {
        alpha3: "AWG",
        countries: &["AW"],
        exp: 2,
        name: "Aruban florin",
        num: "533",
    },
    CurrencyCode {
        alpha3: "VUV",
        countries: &["VU"],
        exp: 0,
        name: "Vanuatu vatu",
        num: "548",
    },
    CurrencyCode {
        alpha3: "NZD",
        countries: &["AQ", "CK", "NU", "NZ", "PN", "TK"],
        exp: 2,
        name: "New Zealand dollar",
        num: "554",
    },
    CurrencyCode {
        alpha3: "NIO",
        countries: &["NI"],
        exp: 2,
        name: "Nicaraguan córdoba",
        num: "558",
    },
    CurrencyCode {
        alpha3: "NGN",
        countries: &["NG"],
        exp: 2,
        name: "Nigerian naira",
        num: "566",
    },
    CurrencyCode {
        alpha3: "NOK",
        countries: &["AQ", "BV", "NO", "SJ"],
        exp: 2,
        name: "Norwegian krone",
        num: "578",
    },
    CurrencyCode {
        alpha3: "PKR",
        countries: &["PK"],
        exp: 2,
        name: "Pakistani rupee",
        num: "586",
    },
    CurrencyCode {
        alpha3: "PAB",
        countries: &["PA"],
        exp: 2,
        name: "Panamanian balboa",
        num: "590",
    },
    CurrencyCode {
        alpha3: "PGK",
        countries: &["PG"],
        exp: 2,
        name: "Papua New Guinean kina",
        num: "598",
    },
    CurrencyCode {
        alpha3: "PYG",
        countries: &["PY"],
        exp: 0,
        name: "Paraguayan guaraní",
        num: "600",
    },
    CurrencyCode {
        alpha3: "PEN",
        countries: &["PE"],
        exp: 2,
        name: "Peruvian Sol",
        num: "604",
    },
    CurrencyCode {
        alpha3: "PHP",
        countries: &["PH"],
        exp: 2,
        name: "Philippine peso",
        num: "608",
    },
    CurrencyCode {
        alpha3: "QAR",
        countries: &["QA"],
        exp: 2,
        name: "Qatari riyal",
        num: "634",
    },
    CurrencyCode {
        alpha3: "RUB",
        countries: &["GE-AB", "RU", "UA-43"],
        exp: 2,
        name: "Russian ruble",
        num: "643",
    },
    CurrencyCode {
        alpha3: "RWF",
        countries: &["RW"],
        exp: 0,
        name: "Rwandan franc",
        num: "646",
    },
    CurrencyCode {
        alpha3: "SHP",
        countries: &["SH-AC", "SH-SH"],
        exp: 2,
        name: "Saint Helena pound",
        num: "654",
    },
    CurrencyCode {
        alpha3: "STD",
        countries: &["ST"],
        exp: 2,
        name: "São Tomé and Príncipe dobra",
        num: "678",
    },
    CurrencyCode {
        alpha3: "SAR",
        countries: &["SA"],
        exp: 2,
        name: "Saudi riyal",
        num: "682",
    },
    CurrencyCode {
        alpha3: "SCR",
        countries: &["SC"],
        exp: 2,
        name: "Seychelles rupee",
        num: "690",
    },
    CurrencyCode {
        alpha3: "SLL",
        countries: &["SL"],
        exp: 2,
        name: "Sierra Leonean leone",
        num: "694",
    },
    CurrencyCode {
        alpha3: "SGD",
        countries: &["BN", "SG"],
        exp: 2,
        name: "Singapore dollar",
        num: "702",
    },
    CurrencyCode {
        alpha3: "VND",
        countries: &["VN"],
        exp: 0,
        name: "Vietnamese dong",
        num: "704",
    },
    CurrencyCode {
        alpha3: "SOS",
        countries: &["SO"],
        exp: 2,
        name: "Somali shilling",
        num: "706",
    },
    CurrencyCode {
        alpha3: "ZAR",
        countries: &["ZA"],
        exp: 2,
        name: "South African rand",
        num: "710",
    },
    CurrencyCode {
        alpha3: "SSP",
        countries: &["SS"],
        exp: 2,
        name: "South Sudeanese pound",
        num: "728",
    },
    CurrencyCode {
        alpha3: "SZL",
        countries: &["SZ"],
        exp: 2,
        name: "Swazi lilangeni",
        num: "748",
    },
    CurrencyCode {
        alpha3: "SEK",
        countries: &["SE"],
        exp: 2,
        name: "Swedish krona/kronor",
        num: "752",
    },
    CurrencyCode {
        alpha3: "CHF",
        countries: &["CH", "LI"],
        exp: 2,
        name: "Swiss franc",
        num: "756",
    },
    CurrencyCode {
        alpha3: "SYP",
        countries: &["SY"],
        exp: 2,
        name: "Syrian pound",
        num: "760",
    },
    CurrencyCode {
        alpha3: "THB",
        countries: &["KH", "LA", "MM", "TH"],
        exp: 2,
        name: "Thai baht",
        num: "764",
    },
    CurrencyCode {
        alpha3: "TOP",
        countries: &["TO"],
        exp: 2,
        name: "Tongan pa'anga",
        num: "776",
    },
    CurrencyCode {
        alpha3: "TTD",
        countries: &["TT"],
        exp: 2,
        name: "Trinidad and Tobago dollar",
        num: "780",
    },
    CurrencyCode {
        alpha3: "AED",
        countries: &["AE"],
        exp: 2,
        name: "United Arab Emirates dirham",
        num: "784",
    },
    CurrencyCode {
        alpha3: "TND",
        countries: &["TN"],
        exp: 3,
        name: "Tunisian dinar",
        num: "788",
    },
    CurrencyCode {
        alpha3: "UGX",
        countries: &["UG"],
        exp: 0,
        name: "Ugandan shilling",
        num: "800",
    },
    CurrencyCode {
        alpha3: "MKD",
        countries: &["MK"],
        exp: 2,
        name: "Macedonian denar",
        num: "807",
    },
    CurrencyCode {
        alpha3: "EGP",
        countries: &["EG"],
        exp: 2,
        name: "Egyptian pound",
        num: "818",
    },
    CurrencyCode {
        alpha3: "GBP",
        countries: &["GG", "GS", "IM", "IO", "JE", "SH-TA", "UK"],
        exp: 2,
        name: "Pound sterling",
        num: "826",
    },
    CurrencyCode {
        alpha3: "TZS",
        countries: &["TZ"],
        exp: 2,
        name: "Tanzanian shilling",
        num: "834",
    },
    CurrencyCode {
        alpha3: "USD",
        countries: &[
            "AS", "BB", "BM", "BQ", "EC", "FM", "GU", "HT", "IO", "MH", "MP", "PA", "PR", "PW",
            "SV", "TC", "TL", "US", "VG", "VI", "ZW",
        ],
        exp: 2,
        name: "United States dollar",
        num: "840",
    },
    CurrencyCode {
        alpha3: "UYU",
        countries: &["UY"],
        exp: 2,
        name: "Uruguayan peso",
        num: "858",
    },
    CurrencyCode {
        alpha3: "UZS",
        countries: &["UZ"],
        exp: 2,
        name: "Uzbekistan som",
        num: "860",
    },
    CurrencyCode {
        alpha3: "WST",
        countries: &["WS"],
        exp: 2,
        name: "Samoan tala",
        num: "882",
    },
    CurrencyCode {
        alpha3: "YER",
        countries: &["YE"],
        exp: 2,
        name: "Yemeni rial",
        num: "886",
    },
    CurrencyCode {
        alpha3: "TWD",
        countries: &["TW"],
        exp: 2,
        name: "New Taiwan dollar",
        num: "901",
    },
    CurrencyCode {
        alpha3: "CUC",
        countries: &["CU"],
        exp: 2,
        name: "Cuban convertible peso",
        num: "931",
    },
    CurrencyCode {
        alpha3: "TMT",
        countries: &["TM"],
        exp: 2,
        name: "Turkmenistani manat",
        num: "934",
    },
    CurrencyCode {
        alpha3: "GHS",
        countries: &["GH"],
        exp: 2,
        name: "Ghanaian cedi",
        num: "936",
    },
    CurrencyCode {
        alpha3: "VEF",
        countries: &["VE"],
        exp: 2,
        name: "Venezuelan bolivar",
        num: "937",
    },
    CurrencyCode {
        alpha3: "SDG",
        countries: &["SD"],
        exp: 2,
        name: "Sudanese pound",
        num: "938",
    },
    CurrencyCode {
        alpha3: "RSD",
        countries: &["RS"],
        exp: 2,
        name: "Serbian dinar",
        num: "941",
    },
    CurrencyCode {
        alpha3: "MZN",
        countries: &["MZ"],
        exp: 2,
        name: "Mozambican metical",
        num: "943",
    },
    CurrencyCode {
        alpha3: "AZN",
        countries: &["AZ"],
        exp: 2,
        name: "Azerbaijani manat",
        num: "944",
    },
    CurrencyCode {
        alpha3: "RON",
        countries: &["RO"],
        exp: 2,
        name: "Romanian leu",
        num: "946",
    },
    CurrencyCode {
        alpha3: "TRY",
        countries: &["TR"],
        exp: 2,
        name: "Turkish lira",
        num: "949",
    },
    CurrencyCode {
        alpha3: "XAF",
        countries: &["CM", "CF", "CG", "GA", "GQ", "TD"],
        exp: 0,
        name: "CFA franc BEAC",
        num: "950",
    },
    CurrencyCode {
        alpha3: "XCD",
        countries: &["AI", "AG", "DM", "GD", "KN", "LC", "MS", "VC"],
        exp: 2,
        name: "East Caribbean dollar",
        num: "951",
    },
    CurrencyCode {
        alpha3: "XOF",
        countries: &["BF", "BJ", "CI", "GW", "ML", "NE", "SN", "TG"],
        exp: 0,
        name: "CFA franc BCEAO",
        num: "952",
    },
    CurrencyCode {
        alpha3: "XPF",
        countries: &["NC", "PF", "WF"],
        exp: 0,
        name: "CFP franc",
        num: "953",
    },
    CurrencyCode {
        alpha3: "ZMW",
        countries: &["ZM"],
        exp: 2,
        name: "Zambian kwacha",
        num: "967",
    },
    CurrencyCode {
        alpha3: "SRD",
        countries: &["SR"],
        exp: 2,
        name: "Surinamese dollar",
        num: "968",
    },
    CurrencyCode {
        alpha3: "MGA",
        countries: &["MG"],
        exp: 1,
        name: "Malagasy ariary",
        num: "969",
    },
    CurrencyCode {
        alpha3: "AFN",
        countries: &["AF"],
        exp: 2,
        name: "Afghan afghani",
        num: "971",
    },
    CurrencyCode {
        alpha3: "TJS",
        countries: &["TJ"],
        exp: 2,
        name: "Tajikstani somoni",
        num: "972",
    },
    CurrencyCode {
        alpha3: "AOA",
        countries: &["AO"],
        exp: 2,
        name: "Angolan kwanza",
        num: "973",
    },
    CurrencyCode {
        alpha3: "BYR",
        countries: &["BY"],
        exp: 0,
        name: "Belarusian ruble",
        num: "974",
    },
    CurrencyCode {
        alpha3: "BGN",
        countries: &["BG"],
        exp: 2,
        name: "Bulgarian lev",
        num: "975",
    },
    CurrencyCode {
        alpha3: "CDF",
        countries: &["CD"],
        exp: 2,
        name: "Congolese franc",
        num: "976",
    },
    CurrencyCode {
        alpha3: "BAM",
        countries: &["BA"],
        exp: 2,
        name: "Bosnia and Herzegovina convertible mark",
        num: "977",
    },
    CurrencyCode {
        alpha3: "EUR",
        countries: &[
            "AD", "AT", "BE", "BL", "CY", "DE", "EE", "ES", "FI", "FR", "GP", "GR", "IE", "IT",
            "LT", "LU", "LV", "MC", "ME", "MQ", "MT", "NL", "PM", "PT", "RE", "SI", "SK", "VA",
            "XK", "YT",
        ],
        exp: 2,
        name: "Euro",
        num: "978",
    },
    CurrencyCode {
        alpha3: "UAH",
        countries: &["UA"],
        exp: 2,
        name: "Ukrainian hryvnia",
        num: "980",
    },
    CurrencyCode {
        alpha3: "GEL",
        countries: &["GE"],
        exp: 2,
        name: "Georgian lari",
        num: "981",
    },
    CurrencyCode {
        alpha3: "PLN",
        countries: &["PL"],
        exp: 2,
        name: "Polish złoty",
        num: "985",
    },
    CurrencyCode {
        alpha3: "BRL",
        countries: &["BR"],
        exp: 2,
        name: "Brazilian real",
        num: "986",
    },
];

/// Returns all CurrencyCodes defined by ISO 4217.
// A function that returns a ref to a really big array of all the currency codes
// designated by ISO 4217, with some exceptions:
//
// - BOV Bolivian Mvdol (funds code)
// - CHE WIR Euro (complementary currency)
// - CHW WIR Franc (complementary currency)
// - CLF Unidad de Fomento (funds code)
// - CNH Chinese yuan when traded in Hong Kong
// - COU Unidad de Valor Real (UVR) (funds code)
// - MXV Mexican Unidad de Inversion (UDI) (funds code)
// - USN United States dollar (next day) (funds code)
// - USS United States dollar (same day) (funds code)
// - UYI Uruguay Peso en Unidades Indexadas (URUIURUI) (funds code)
// - XAG Silver (one troy ounce)
// - XAU Gold (one troy ounce)
// - XBA European Composite Unit (EURCO) (bond market unit)
// - XBB European Monetary Unit (E.M.U.-6) (bond market unit)
// - XBC European Unit of Account 9 (E.U.A.-9) (bond market unit)
// - XBD European Unit of Account 17 (E.U.A.-17) (bond market unit)
// - XDR Special drawing rights
// - XFU UIC franc (special settlement currency)
// - XPD Palladium (one troy ounce)
// - XPT Platinum (one troy ounce)
// - XSU Unified System for Regional Compensation (SUCRE)
// - XTS Code reserved for testing purposes
// - XUA ADB Unit of Account (African Development Bank)
// - XXX No currency
// - ZWD Zimbabwe dollar
//
// Country alpha2-codes are taken from the ISO's website:
// https://www.iso.org/obp/ui/
pub fn all() -> &'static [CurrencyCode] {
    &ALL
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_alpha3_unique() {
        let all_codes = all();
        let mut seen = HashSet::new();

        for code in all_codes {
            assert!(
                seen.insert(code.alpha3),
                "Duplicate alpha3 code found: {}",
                code.alpha3
            );
        }

        // Verify we have all 155 unique codes
        assert_eq!(seen.len(), 155);
    }
}
