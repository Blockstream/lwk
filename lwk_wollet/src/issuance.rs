use crate::contract::Contract;
use crate::elements::{Address, AssetId, OutPoint, Transaction};

#[derive(Debug)]
// We make issuance and reissuance are mutually exclusive for simplicity
// TODO: restructurize in same way as issuances
pub enum ReissuanceRequest {
    None,
    /// `Box<Option<Address>>` is temporarily boxed to satisfy the linter.
    /// Will be removed once the same changes as for [`Issuances`] are implemented.
    Reissuance(AssetId, u64, Box<Option<Address>>, Option<Transaction>),
}

#[derive(Debug)]
pub enum Issuances {
    None,
    Sequential(Vec<IssuanceRequest>),
    Pinned(Vec<(IssuanceRequest, OutPoint)>),
}

/// A request to issue a new asset, passed to [`crate::TxBuilder::add_issuance()`]
#[derive(Debug, Clone)]
pub struct IssuanceRequest {
    pub(crate) satoshi_asset: u64,
    pub(crate) address_asset: Option<Address>,
    pub(crate) satoshi_token: u64,
    pub(crate) address_token: Option<Address>,
    pub(crate) contract: Option<Contract>,
    pub(crate) pinned_input: Option<OutPoint>,
}

impl IssuanceRequest {
    /// Creates a builder for an issuance of `satoshi_asset` asset units and `satoshi_token`
    /// reissuance tokens (at least one of the two must be greater than zero)
    pub fn new(satoshi_asset: u64, satoshi_token: u64) -> Self {
        Self {
            satoshi_asset,
            address_asset: None,
            satoshi_token,
            address_token: None,
            contract: None,
            pinned_input: None,
        }
    }

    /// Sets the address receiving the issued asset units; if not called, they are sent
    /// to an address of the wallet generating the issuance
    pub fn address_asset(mut self, address: Address) -> Self {
        self.address_asset = Some(address);
        self
    }

    /// Sets the address receiving the reissuance tokens; if not called, they are sent
    /// to an address of the wallet generating the issuance
    pub fn address_token(mut self, address: Address) -> Self {
        self.address_token = Some(address);
        self
    }

    /// Sets the contract whose metadata will be committed in the generated asset id
    pub fn contract(mut self, contract: Contract) -> Self {
        self.contract = Some(contract);
        self
    }

    /// Pin this issuance to a specific input
    ///
    /// Requires manual inputs order: `input` must be one of the outpoints passed to
    /// [`crate::TxBuilder::set_inputs_order()`], otherwise [`crate::TxBuilder::finish()`] will
    /// error.
    ///
    /// If multiple issuances in the same transaction are pinned, each must target a different
    /// input: pinning two issuances to the same outpoint errors at finish time.
    pub fn pin_input(mut self, input: OutPoint) -> Self {
        self.pinned_input = Some(input);
        self
    }
}
