#![doc = include_str!("../README.md")]

// from:
// https://github.com/bitcoin-core/HWI/blob/70ffb2be827e5b3d304203b4ded1f79c07b04a5f/hwilib/hwwclient.py
// https://github.com/bitcoin-core/HWI/blob/70ffb2be827e5b3d304203b4ded1f79c07b04a5f/test/test_jade.py

// enumerate

// supported by jade

// get_master_xpub(self, addrtype: AddressType = AddressType.WIT, account: int = 0) -> ExtendedKey:
// get_master_fingerprint(self) -> bytes
// get_pubkey_at_path(self, bip32_path: str) -> ExtendedKey
// sign_tx(self, psbt: PSBT) -> PSBT
// sign_message(self, message: Union[str, bytes], bip32_path: str) -> str
// display_singlesig_address(self, bip32_path: str, addr_type: AddressType) -> str
// display_multisig_address(self, addr_type: AddressType, multisig: MultisigDescriptor,) -> str
// close(self) -> None
// can_sign_taproot(self) -> bool (false)

// unsupported by jade

// wipe_device(self) -> bool
// setup_device(self, label: str = "", passphrase: str = "") -> bool
// restore_device(self, label: str = "", word_count: int = 24) -> bool
// backup_device(self, label: str = "", passphrase: str = "") -> bool
// prompt_pin(self) -> bool
// send_pin(self, pin: str) -> bool
// toggle_passphrase(self) -> bool
