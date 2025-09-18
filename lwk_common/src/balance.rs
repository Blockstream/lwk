use std::collections::{BTreeMap, HashSet};

use elements::AssetId;
use serde::{Deserialize, Serialize};

/// Wallet balance wrapper
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Balance(BTreeMap<AssetId, u64>);

impl From<BTreeMap<AssetId, u64>> for Balance {
    fn from(map: BTreeMap<AssetId, u64>) -> Self {
        Self(map)
    }
}

impl AsRef<BTreeMap<AssetId, u64>> for Balance {
    fn as_ref(&self) -> &BTreeMap<AssetId, u64> {
        &self.0
    }
}

impl std::ops::Deref for Balance {
    type Target = BTreeMap<AssetId, u64>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::Sub<Balance> for Balance {
    type Output = SignedBalance;

    fn sub(self, other: Balance) -> SignedBalance {
        let all_keys: HashSet<_> = other.0.keys().chain(self.0.keys()).collect();
        let mut result = BTreeMap::new();
        for key in all_keys {
            result.insert(
                *key,
                self.0.get(key).cloned().unwrap_or(0) as i64
                    - other.0.get(key).cloned().unwrap_or(0) as i64,
            );
        }
        result.retain(|_, v| *v != 0);
        SignedBalance(result)
    }
}

/// A signed balance of assets, to represent a balance with negative values such
/// as the results of a transactions from the perspective of a wallet.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(transparent)]
pub struct SignedBalance(BTreeMap<AssetId, i64>);

impl From<BTreeMap<AssetId, i64>> for SignedBalance {
    fn from(map: BTreeMap<AssetId, i64>) -> Self {
        Self(map)
    }
}

impl AsRef<BTreeMap<AssetId, i64>> for SignedBalance {
    fn as_ref(&self) -> &BTreeMap<AssetId, i64> {
        &self.0
    }
}

impl std::ops::Deref for SignedBalance {
    type Target = BTreeMap<AssetId, i64>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn test_balance_subtraction() {
        use std::collections::BTreeMap;

        // Helper function to create AssetId from hex string
        let a = |a| AssetId::from_str(a).unwrap();

        // Use the policy asset from lwk_test_util as a base and create variations
        let policy_asset =
            AssetId::from_str("5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225")
                .unwrap();
        let asset1 = policy_asset;
        let asset2 = a("6f0279e9ed041c3d710a9f57d0c02928416460c4b722d5a6d002dcab7c5b5b5b");
        let asset3 = a("7f0279e9ed041c3d710a9f57d0c02928416460c4b722d5a6d002dcab7c5b5b5b");

        // Test case 1: Basic subtraction with same assets
        let mut balance1 = BTreeMap::new();
        balance1.insert(asset1, 1000);
        balance1.insert(asset2, 500);

        let mut balance2 = BTreeMap::new();
        balance2.insert(asset1, 300);
        balance2.insert(asset2, 200);

        let result = Balance(balance1.clone()) - Balance(balance2.clone());
        assert_eq!(result.get(&asset1), Some(&700));
        assert_eq!(result.get(&asset2), Some(&300));

        // Test case 2: Subtraction with different assets (some assets only in one balance)
        let mut balance3 = BTreeMap::new();
        balance3.insert(asset1, 1000);
        balance3.insert(asset2, 500);

        let mut balance4 = BTreeMap::new();
        balance4.insert(asset1, 300);
        balance4.insert(asset3, 100); // Different asset

        let result2 = Balance(balance3) - Balance(balance4);
        assert_eq!(result2.get(&asset1), Some(&700));
        assert_eq!(result2.get(&asset2), Some(&500)); // Unchanged
        assert_eq!(result2.get(&asset3), Some(&-100)); // 0 - 100 = -100

        // Test case 3: Subtraction resulting in zero
        let mut balance5 = BTreeMap::new();
        balance5.insert(asset1, 500);

        let mut balance6 = BTreeMap::new();
        balance6.insert(asset1, 500);

        let result3 = Balance::from(balance5) - Balance::from(balance6);
        assert_eq!(result3.get(&asset1), None);

        // Test case 4: Empty balance subtraction
        let empty_balance = Balance::from(BTreeMap::new());
        let result4 = Balance::from(balance1) - empty_balance.clone();
        assert_eq!(result4.get(&asset1), Some(&1000));
        assert_eq!(result4.get(&asset2), Some(&500));

        // Test case 5: Subtracting from empty balance
        let result5 = empty_balance - Balance::from(balance2);
        assert_eq!(result5.get(&asset1), Some(&-300));
        assert_eq!(result5.get(&asset2), Some(&-200));

        // Test case 6: Subtraction that would result in negative (testing the logic, though this shouldn't happen in practice)
        let mut balance7 = BTreeMap::new();
        balance7.insert(asset1, 100);

        let mut balance8 = BTreeMap::new();
        balance8.insert(asset1, 300);

        let result6 = Balance::from(balance7) - Balance::from(balance8);
        // This will result in 100 - 300 = -200
        assert_eq!(result6.get(&asset1), Some(&-200));
    }
}
