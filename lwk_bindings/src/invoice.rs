/// Represents a syntactically and semantically correct lightning BOLT11 invoice.
pub struct Bolt11Invoice {
    inner: lwk_boltz::Bolt11Invoice,
}
