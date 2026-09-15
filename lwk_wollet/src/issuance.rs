use crate::elements::{Address, AssetId, OutPoint, Transaction};
use crate::Contract;
use crate::Error;
use std::collections::HashSet;

#[derive(Debug)]
pub enum Issuances {
    None,
    Sequential(Vec<IssuanceRequest>),
    Pinned(Vec<(IssuanceRequest, OutPoint)>),
}

/// A request to issue a new asset, passed to [`crate::TxBuilder::add_issuance()`]
///
/// **Experimental**: this API might change without notice.
#[derive(Debug, Clone)]
pub struct IssuanceRequest {
    pub(crate) satoshi_asset: u64,
    pub(crate) asset_outputs: Vec<IssuanceOutput>,
    pub(crate) satoshi_token: u64,
    pub(crate) token_outputs: Vec<IssuanceOutput>,
    pub(crate) contract: Option<Contract>,
    pub(crate) pinned_input: Option<OutPoint>,
}

/// An output of a (re)issuance: `satoshi` units received by `address`, or by an address of the
/// wallet generating the (re)issuance if `address` is `None`
#[derive(Debug, Clone)]
pub(crate) struct IssuanceOutput {
    pub(crate) satoshi: u64,
    pub(crate) address: Option<Address>,
}

impl IssuanceRequest {
    /// Creates a builder for an issuance of `satoshi_asset` asset units and `satoshi_token`
    /// reissuance tokens
    ///
    /// **Experimental**: this API might change without notice.
    ///
    /// At least one of the two amounts must be greater than zero.
    pub fn new(satoshi_asset: u64, satoshi_token: u64) -> Self {
        Self {
            satoshi_asset,
            asset_outputs: vec![],
            satoshi_token,
            token_outputs: vec![],
            contract: None,
            pinned_input: None,
        }
    }

    /// Adds an output receiving `satoshi` of the issued asset units
    ///
    /// **Experimental**: this API might change without notice.
    ///
    /// The units are sent to `address` if some, or to an address of the wallet generating the
    /// issuance if none.
    ///
    /// Call this multiple times to split the issued units across several outputs: the amounts of
    /// all the added outputs must sum up to the issued amount, otherwise
    /// [`crate::TxBuilder::add_issuance()`] errors.
    ///
    /// If not called, the issued units are sent to a single address of the wallet generating the
    /// issuance.
    pub fn add_asset_output(mut self, satoshi: u64, address: Option<Address>) -> Self {
        self.asset_outputs.push(IssuanceOutput { satoshi, address });
        self
    }

    /// Adds an output receiving `satoshi` reissuance tokens
    ///
    /// **Experimental**: this API might change without notice.
    ///
    /// The tokens are sent to `address` if some, or to an address of the wallet generating the
    /// issuance if none.
    ///
    /// Call this multiple times to split the reissuance tokens across several outputs: the amounts
    /// of all the added outputs must sum up to the issued token amount, otherwise
    /// [`crate::TxBuilder::add_issuance()`] errors.
    ///
    /// If not called, the reissuance tokens are sent to a single address of the wallet generating
    /// the issuance.
    pub fn add_token_output(mut self, satoshi: u64, address: Option<Address>) -> Self {
        self.token_outputs.push(IssuanceOutput { satoshi, address });
        self
    }

    /// Sets the contract whose metadata will be committed in the generated asset id
    ///
    /// **Experimental**: this API might change without notice.
    pub fn contract(mut self, contract: Contract) -> Self {
        self.contract = Some(contract);
        self
    }

    /// Pin this issuance to a specific input
    ///
    /// **Experimental**: this API might change without notice.
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
///
/// **Experimental**: this API might change without notice.
#[derive(Debug, Clone)]
pub struct ReissuanceRequest {
    pub(crate) asset_to_reissue: AssetId,
    pub(crate) satoshi_to_reissue: u64,
    pub(crate) asset_outputs: Vec<IssuanceOutput>,
    pub(crate) issuance_tx: Option<Transaction>,
    pub(crate) pinned_input: Option<OutPoint>,
}

impl ReissuanceRequest {
    /// Creates a request to reissue `satoshi_to_reissue` units of `asset_to_reissue`
    ///
    /// **Experimental**: this API might change without notice.
    ///
    /// Requires the reissuance token to be owned by the wallet generating the reissuance.
    pub fn new(asset_to_reissue: AssetId, satoshi_to_reissue: u64) -> Self {
        Self {
            asset_to_reissue,
            satoshi_to_reissue,
            asset_outputs: vec![],
            issuance_tx: None,
            pinned_input: None,
        }
    }

    /// Adds an output receiving `satoshi` of the reissued asset units
    ///
    /// **Experimental**: this API might change without notice.
    ///
    /// The units are sent to `address` if some, or to an address of the wallet generating the
    /// reissuance if none.
    ///
    /// Call this multiple times to split the reissued units across several outputs: the amounts of
    /// all the added outputs must sum up to the reissued amount, otherwise
    /// [`crate::TxBuilder::add_reissuance()`] errors.
    ///
    /// If not called, the reissued units are sent to a single address of the wallet generating the
    /// reissuance.
    pub fn add_asset_output(mut self, satoshi: u64, address: Option<Address>) -> Self {
        self.asset_outputs.push(IssuanceOutput { satoshi, address });
        self
    }

    /// Sets the transaction containing the original issuance of `asset_to_reissue`
    ///
    /// **Experimental**: this API might change without notice.
    ///
    /// Only needed if that issuance transaction does not involve this wallet.
    pub fn issuance_tx(mut self, tx: Transaction) -> Self {
        self.issuance_tx = Some(tx);
        self
    }

    /// Pin this reissuance to the reissuance token utxo spent by `input`
    ///
    /// **Experimental**: this API might change without notice.
    ///
    /// `input` must hold the reissuance token of `asset_to_reissue`, otherwise
    /// [`crate::TxBuilder::finish()`] will error. If it is not already an input of the transaction
    /// it is added, unless a manual inputs order is set, in which case it must be one of the
    /// outpoints passed to [`crate::TxBuilder::set_inputs_order()`].
    ///
    /// If not called, the reissuance is assigned to the first input holding the token.
    pub fn pin_input(mut self, input: OutPoint) -> Self {
        self.pinned_input = Some(input);
        self
    }
}
