/// Wallet chain
#[derive(uniffi::Enum, Debug, PartialEq, Eq)]
pub enum Chain {
    /// External address, shown when asked for a payment.
    /// Wallet having a single descriptor are considered External
    External,

    /// Internal address, used for the change
    Internal,
}

impl From<lwk_wollet::Chain> for Chain {
    fn from(value: lwk_wollet::Chain) -> Self {
        match value {
            lwk_wollet::Chain::External => Chain::External,
            lwk_wollet::Chain::Internal => Chain::Internal,
        }
    }
}

impl From<Chain> for lwk_wollet::Chain {
    fn from(value: Chain) -> Self {
        match value {
            Chain::External => lwk_wollet::Chain::External,
            Chain::Internal => lwk_wollet::Chain::Internal,
        }
    }
}
