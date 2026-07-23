use crate::contract::Contract;
use crate::elements::{Address, AssetId, OutPoint, Transaction};
use crate::Error;
use std::collections::HashSet;

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

/// Accumulates the reissuance requests added via [`crate::TxBuilder::add_reissuance()`].
#[derive(Default)]
pub struct Reissuances {
    pub(crate) requests: Vec<ReissuanceRequest>,
    /// Cache to check for duplicated assets
    assets: HashSet<AssetId>,
}

impl std::fmt::Debug for Reissuances {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        // `assets` cache skipped to avoid random iteration order problems
        f.debug_struct("Reissuances")
            .field("requests", &self.requests)
            .finish()
    }
}

impl Reissuances {
    pub fn add(&mut self, request: ReissuanceRequest) -> Result<(), Error> {
        if !self.assets.insert(request.asset_to_reissue) {
            return Err(Error::DuplicatedReissuanceAsset(request.asset_to_reissue));
        }

        self.requests.push(request);

        Ok(())
    }
}

/// A request to reissue an existing asset, passed to [`crate::TxBuilder::add_reissuance()`]
#[derive(Debug, Clone)]
pub struct ReissuanceRequest {
    pub(crate) asset_to_reissue: AssetId,
    pub(crate) satoshi_to_reissue: u64,
    pub(crate) asset_receiver: Option<Address>,
    pub(crate) issuance_tx: Option<Transaction>,
}

impl ReissuanceRequest {
    /// Creates a request to reissue `satoshi_to_reissue` units of `asset_to_reissue`, provided
    /// the reissuance token is owned by the wallet generating the reissuance
    pub fn new(asset_to_reissue: AssetId, satoshi_to_reissue: u64) -> Self {
        Self {
            asset_to_reissue,
            satoshi_to_reissue,
            asset_receiver: None,
            issuance_tx: None,
        }
    }

    /// Sets the address receiving the reissued asset units; if not called, they are sent
    /// to an address of the wallet generating the reissuance
    pub fn asset_receiver(mut self, address: Address) -> Self {
        self.asset_receiver = Some(address);
        self
    }

    /// Sets the transaction containing the original issuance of `asset_to_reissue`; only
    /// needed if that issuance transaction does not involve this wallet
    pub fn issuance_tx(mut self, tx: Transaction) -> Self {
        self.issuance_tx = Some(tx);
        self
    }
}
