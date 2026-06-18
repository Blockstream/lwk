use lwk_common::Network;
use lwk_wollet::{
    elements::{pset::PartiallySignedTransaction, AssetId},
    Wollet, WolletDescriptor,
};

use crate::lending::error::LendingError;

pub struct LendingSession {
    network: Network,
    indexer_url: Option<String>,
    descriptor: WolletDescriptor,
}

impl LendingSession {
    pub fn builder(network: Network, descriptor: WolletDescriptor) -> LendingSessionBuilder {
        LendingSessionBuilder::new(network, descriptor)
    }

    pub fn network(&self) -> Network {
        self.network
    }

    pub fn indexer_url(&self) -> Option<&str> {
        self.indexer_url.as_deref()
    }

    pub fn descriptor(&self) -> &WolletDescriptor {
        &self.descriptor
    }

    /// One-time action from every user to prepare for creating an offer
    ///
    /// Returns fully assembled PSET for creation of borrower account
    pub fn borrower_prepare(
        &self,
        _wollet: &Wollet,
    ) -> Result<PrepareBorrowTransaction, LendingError> {
        todo!()
    }

    /// Create borrow offer
    ///
    /// # Errors
    /// Borrower account was not previously created
    pub fn borrower_create_offer(
        &self,
        _wollet: &Wollet,
        _details: OfferDetails,
    ) -> Result<CreateBorrowTransaction, LendingError> {
        todo!()
    }

    pub fn fully_repay_loan(&self, _details: RepaymentDetails) -> Result<(), LendingError> {
        todo!()
    }

    pub fn partially_repay_loan(&self, _details: RepaymentDetails) -> Result<(), LendingError> {
        todo!()
    }

    pub fn cancel_offer(&self) -> Result<(), LendingError> {
        todo!()
    }

    pub fn accept_offer(&self) -> Result<(), LendingError> {
        todo!()
    }

    pub fn claim_partial_repayment(&self) -> Result<(), LendingError> {
        todo!()
    }

    pub fn liquidate_offer(&self) -> Result<(), LendingError> {
        todo!()
    }
}

/// Builder for creating a [`LendingSession`].
pub struct LendingSessionBuilder {
    network: Network,
    indexer_url: Option<String>,
    descriptor: WolletDescriptor,
}

impl LendingSessionBuilder {
    /// Create a new [`LendingSessionBuilder`] with required parameters.
    pub fn new(network: Network, descriptor: WolletDescriptor) -> Self {
        Self {
            network,
            indexer_url: None,
            descriptor,
        }
    }
    /// Build the [`LendingSession`].
    pub fn build(self) -> Result<LendingSession, LendingError> {
        Ok(LendingSession {
            network: self.network,
            indexer_url: self.indexer_url,
            descriptor: self.descriptor,
        })
    }

    pub fn set_indexer(mut self, indexer_url: &str) -> Self {
        self.indexer_url = Some(indexer_url.to_string());
        self
    }
}

pub struct OfferDetails {
    pub principal_asset_id: AssetId,
    pub principal_amount: u64,
    pub collateral_asset_id: AssetId,
    pub collateral_amount: u64,
    pub loan_expiration_time: u32,
    pub principal_interest_rate: u16,
}

pub struct RepaymentDetails {
    pub amount_to_repay: u64,
}

pub struct PrepareBorrowTransaction {
    inner: PartiallySignedTransaction,
}

impl PrepareBorrowTransaction {
    pub fn inner(&self) -> &PartiallySignedTransaction {
        &self.inner
    }
}

pub struct CreateBorrowTransaction {
    inner: PartiallySignedTransaction,
}

impl CreateBorrowTransaction {
    pub fn inner(&self) -> &PartiallySignedTransaction {
        &self.inner
    }
}
