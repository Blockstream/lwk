# Merge Test Updates

This directory contains 3 consecutive `Update` structs for a regtest wallet with descriptor:

ct(slip77(e12440f9ab46536bee060598e3b6064c0c1fa46049731d614b5c973ed054b363),elwpkh(tpubD6NzVbkrYhZ4Y2AL6uijCXAKHRNLMt7rxWnZt3bjPuYv8ibEQvGmEhRPDqbLUX7a7yCwEyEgEV37gu2CUa3a6CCpjLfrv3Xks9pGitNtd7z/*))

## Files

- `update_merge_1.bin` - Initial wallet sync (tip only update)
- `update_merge_2.bin` - Wallet funding transaction  
- `update_merge_3.bin` - Spending transaction
- `descriptor.txt` - The wallet descriptor used for these updates

## Expected Wallet States

When these updates are applied sequentially to a fresh wallet, the expected states are:

### After Update 1
- **Balance**: 0 L-BTC
- **Transactions**: 0
- **UTXOs**: 0
- **Description**: Initial sync of an empty wallet, only updates the blockchain tip

### After Update 2
- **Balance**: 1,000,000
- **Transactions**: 1
- **UTXOs**: 1
- **Description**: Wallet received funding transaction of 1,000,000 sats

### After Update 3
- **Balance**: 899,974 sats
- **Transactions**: 2
- **UTXOs**: 1 (change output)
- **Description**: Wallet sent 100,000 sats to an external address with a fee of 26 sats
