use std::sync::Arc;

use crate::error;
pub use lwk_common::DynStore;

/// Store keys for Boltz swap persistence.
///
/// All keys are namespaced with a prefix derived from the mnemonic (e.g., `2e8d7a6ccb9bbae4:boltz:`)
/// to ensure data isolation between different wallets sharing the same store.
pub mod store_keys {
    use super::error;
    use super::DynStore;

    /// Generate the key for the list of pending swap IDs
    pub fn pending_swaps(prefix: &str) -> String {
        format!("{prefix}:boltz:pending_swaps")
    }

    /// Generate the key for the list of completed swap IDs
    pub fn completed_swaps(prefix: &str) -> String {
        format!("{prefix}:boltz:completed_swaps")
    }

    /// Generate the key for a specific swap's data
    pub fn swap_data(prefix: &str, swap_id: &str) -> String {
        format!("{prefix}:boltz:swap:{swap_id}")
    }

    /// Read the pending swaps list from the store
    ///
    /// Returns an empty Vec if the key doesn't exist, propagates errors on store
    /// access failure or deserialization failure.
    pub fn get_pending_swaps(
        store: &dyn DynStore,
        prefix: &str,
    ) -> Result<Vec<String>, error::Error> {
        store
            .get(&pending_swaps(prefix))
            .map_err(error::Error::Store)?
            .map(|data| serde_json::from_slice(&data))
            .transpose()?
            .map_or_else(|| Ok(Vec::new()), Ok)
    }

    /// Read the completed swaps list from the store
    ///
    /// Returns an empty Vec if the key doesn't exist, propagates errors on store
    /// access failure or deserialization failure.
    pub fn get_completed_swaps(
        store: &dyn DynStore,
        prefix: &str,
    ) -> Result<Vec<String>, error::Error> {
        store
            .get(&completed_swaps(prefix))
            .map_err(error::Error::Store)?
            .map(|data| serde_json::from_slice(&data))
            .transpose()?
            .map_or_else(|| Ok(Vec::new()), Ok)
    }

    /// Write the pending swaps list to the store
    pub fn set_pending_swaps(
        store: &dyn DynStore,
        prefix: &str,
        swaps: &[String],
    ) -> Result<(), error::Error> {
        let data = serde_json::to_vec(swaps)?;
        store
            .put(&pending_swaps(prefix), &data)
            .map_err(error::Error::Store)
    }

    /// Write the completed swaps list to the store
    pub fn set_completed_swaps(
        store: &dyn DynStore,
        prefix: &str,
        swaps: &[String],
    ) -> Result<(), error::Error> {
        let data = serde_json::to_vec(swaps)?;
        store
            .put(&completed_swaps(prefix), &data)
            .map_err(error::Error::Store)
    }
}

/// Trait for swap response types that support persistence.
///
/// This trait provides the interface needed for persisting swap data to a store.
/// Implementors must provide serialization, swap ID access, store access, and store prefix.
/// Default implementations are provided for persist operations.
pub trait SwapPersistence {
    /// Serialize the swap data to a JSON string
    fn serialize(&self) -> Result<String, error::Error>;

    /// Get the swap ID
    fn swap_id(&self) -> &str;

    /// Get a reference to the store, if configured
    fn store(&self) -> Option<&Arc<dyn DynStore>>;

    /// Get the store key prefix (derived from mnemonic identifier)
    fn store_prefix(&self) -> &str;

    /// Persist swap data to the store
    fn persist(&self) -> Result<(), error::Error> {
        if let Some(store) = self.store() {
            let data = self.serialize()?;
            let key = store_keys::swap_data(self.store_prefix(), self.swap_id());
            store
                .put(&key, data.as_bytes())
                .map_err(error::Error::Store)?;
            log::debug!("Persisted swap data for {}", self.swap_id());
        }
        Ok(())
    }

    /// Persist swap data and add to pending swaps list
    fn persist_and_add_to_pending(&self) -> Result<(), error::Error> {
        if let Some(store) = self.store() {
            let prefix = self.store_prefix();

            // Persist the swap data
            self.persist()?;

            // Add to pending list
            let mut pending = store_keys::get_pending_swaps(store.as_ref(), prefix)?;

            let swap_id = self.swap_id().to_string();
            if !pending.contains(&swap_id) {
                pending.push(swap_id.clone());
                store_keys::set_pending_swaps(store.as_ref(), prefix, &pending)?;
                log::debug!("Added swap {swap_id} to pending list");
            }
        }
        Ok(())
    }

    /// Move swap from pending to completed list
    fn move_to_completed(&self) -> Result<(), error::Error> {
        if let Some(store) = self.store() {
            let prefix = self.store_prefix();
            let swap_id = self.swap_id().to_string();

            // Remove from pending list
            let mut pending = store_keys::get_pending_swaps(store.as_ref(), prefix)?;
            pending.retain(|id| id != &swap_id);
            store_keys::set_pending_swaps(store.as_ref(), prefix, &pending)?;

            // Add to completed list
            let mut completed = store_keys::get_completed_swaps(store.as_ref(), prefix)?;
            if !completed.contains(&swap_id) {
                completed.push(swap_id.clone());
                store_keys::set_completed_swaps(store.as_ref(), prefix, &completed)?;
            }

            log::debug!("Moved swap {swap_id} to completed list");
        }
        Ok(())
    }
}
