# Watch-Only Wallets
In LWK, the core functions of a wallet are split between two components for enhanced security: **Signers** manage private keys, while the **Wollet** handles everything else.

The term "Wollet" is not a typo; it stands for "Watch-Only wallet." A wollet provides view-only access, allowing you to generate addresses and see your balance without ever handling private keys. This design is crucial for security, as it keeps your private keys isolated.

A LWK wollet can perform the following operations:
* Generate addresses
* List transactions
* Get balance
* Create transactions (but not sign them)


