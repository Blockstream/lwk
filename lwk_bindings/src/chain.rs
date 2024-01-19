/// Wallet chain
#[derive(uniffi::Enum, Debug, PartialEq, Eq)]
pub enum Chain {
    /// External address, shown when asked for a payment.
    /// Wallet having a single descriptor are considered External
    External,

    /// Internal address, used for the change
    Internal,
}

impl From<wollet::Chain> for Chain {
    fn from(value: wollet::Chain) -> Self {
        match value {
            wollet::Chain::External => Chain::External,
            wollet::Chain::Internal => Chain::Internal,
        }
    }
}
