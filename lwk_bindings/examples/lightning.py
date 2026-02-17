import hashlib
import json
import os
import threading
import time
from lwk import *


# Implementation of the ForeignStore trait for file-based persistence
class FileStore(ForeignStore):
    """A file-based store implementation that persists data to a directory."""
    
    def __init__(self, store_dir):
        self.store_dir = store_dir
        os.makedirs(store_dir, exist_ok=True)
    
    def _key_to_path(self, key):
        """Convert a key to a file path"""
        # key is an hex string, always valid as path
        return os.path.join(self.store_dir, f"{key}")
    
    def get(self, key):
        path = self._key_to_path(key)
        if os.path.exists(path):
            with open(path, "rb") as f:
                return f.read()
        return None
    
    def put(self, key, value):
        path = self._key_to_path(key)
        with open(path, "wb") as f:
            f.write(bytes(value))
    
    def remove(self, key):
        path = self._key_to_path(key)
        if os.path.exists(path):
            os.remove(path)

# Example implementation of the Logging trait
class MyLogger(Logging):
    def log(self, level: LogLevel, message: str):
        # Only print Info level and greater (Info, Warn, Error)
        if level in (LogLevel.INFO, LogLevel.WARN, LogLevel.ERROR):
            level_str = {
                LogLevel.INFO: "INFO",
                LogLevel.WARN: "WARN",
                LogLevel.ERROR: "ERROR",
            }.get(level, "UNKNOWN")
            print(f"[{level_str}] {message}")

def get_balance(wollet):
    """Get and display the wallet balance"""
    balance = wollet.balance()
    lbtc_asset = network.policy_asset()
    lbtc_balance = balance.get(lbtc_asset, 0)
    print(f"Balance: {lbtc_balance} L-BTC")

    # Show other assets if any
    for asset, amount in balance.items():
        if asset != lbtc_asset:
            print(f"Asset {asset}: {amount}")
    return balance

def update_balance(wollet, esplora_client, desc):
    """Update balance by performing full scan"""
    print("Updating balance...")
    update = esplora_client.full_scan(wollet)
    wollet.apply_update(update)
    print("Balance updated.")
    return get_balance(wollet)

def invoice_thread(invoice_response, claim_address):
    """Thread function to handle invoice completion"""
    print("Waiting for invoice payment...")
    swap_id = invoice_response.swap_id()

    while True:
        try:
            state = invoice_response.advance()
            print(f"Invoice state for swap {swap_id}: {state}")
            if state == PaymentState.SUCCESS:
                claim_txid = invoice_response.claim_txid()
                print(f"Invoice payment completed successfully, received from txid: {claim_txid}!")    
            break
        except LwkError.NoBoltzUpdate as e:
            print("No update available, continuing polling...")
            time.sleep(1)
            continue
        except Exception as e:
            print(f"Error in invoice thread - type: {type(e).__name__}, details: {e}")
            import traceback
            traceback.print_exc()
            break

def pay_invoice_thread(prepare_pay_response):
    """Thread function to handle payment completion"""
    print("Waiting for payment completion...")
    swap_id = prepare_pay_response.swap_id()
    while True:
        try:
            state = prepare_pay_response.advance()
            print(f"Payment state for swap {swap_id}: {state}")
            break
        except LwkError.NoBoltzUpdate as e:
            print("No update available, continuing polling...")
            time.sleep(1)
            continue
        except Exception as e:
            print(f"Error in payment thread - type: {type(e).__name__}, details: {e}")
            import traceback
            traceback.print_exc()
            break

def lockup_thread(lockup_response):
    """Thread function to handle chain swap completion"""
    print("Waiting for chain swap completion...")
    swap_id = lockup_response.swap_id()

    while True:
        try:
            state = lockup_response.advance()
            print(f"Chain swap state for swap {swap_id}: {state}")
            break
        except LwkError.NoBoltzUpdate as e:
            print("No update available, continuing polling...")
            time.sleep(1)
            continue
        except Exception as e:
            print(f"Error in chain swap thread - type: {type(e).__name__}, details: {e}")
            import traceback
            traceback.print_exc()
            break

def should_start_completion_thread(swap_data):
    """Ask whether to start monitoring for a restored swap."""
    swap_id = swap_data.get("id", "unknown")
    last_state = swap_data.get("last_state", "unknown")
    answer = input(
        f"Swap {swap_id} (state: {last_state}). "
        "Start completion thread? [y/N]: "
    ).strip().lower()
    return answer in ("y", "yes")

def show_invoice(boltz_session, wollet):
    """Create and show an invoice"""
    # Ask for the invoice amount
    while True:
        try:
            amount_str = input("Enter invoice amount in satoshis: ").strip()
            amount = int(amount_str)
            if amount <= 0:
                print("Amount must be positive. Please try again.")
                continue
            break
        except ValueError:
            print("Invalid amount. Please enter a valid number.")

    # Get the latest address for claiming
    claim_address = wollet.address(None).address()
    print(f"Claim address: {claim_address}")

    # Create invoice
    webhook_url = os.getenv('WEBHOOK')
    webhook = WebHook(webhook_url, status=[]) if webhook_url else None
    invoice_response = boltz_session.invoice(amount, "Lightning payment", claim_address, webhook) 

    fee = invoice_response.fee()
    print(f"Fee: {fee}")
    boltz_fee = invoice_response.boltz_fee()
    print(f"Boltz fee: {boltz_fee}")

    # Get and print the bolt11 invoice
    bolt11_invoice_obj = invoice_response.bolt11_invoice()
    bolt11_invoice = str(bolt11_invoice_obj)
    print(f"Invoice: {bolt11_invoice}")

    swap_id = invoice_response.swap_id()
    print(f"Swap ID: {swap_id}")

    # Start thread to wait for payment
    thread = threading.Thread(target=invoice_thread, args=(invoice_response, claim_address))
    thread.daemon = True
    thread.start()
    print("Started thread to monitor invoice payment.")

def pay_invoice(boltz_session, wollet, esplora_client, signer, skip_completion_thread=False):
    """Pay a bolt11 invoice"""
    # Read bolt11 invoice from user
    bolt11_str = input("Enter bolt11 invoice: ").strip()

    try:
        # Parse the invoice
        lightning_payment = LightningPayment(bolt11_str)

        # Get refund address
        refund_address = wollet.address(None).address()
        print(f"Refund address: {refund_address}")

        # Prepare payment
        webhook_url = os.getenv('WEBHOOK')
        webhook = WebHook(webhook_url, status=[]) if webhook_url else None
        prepare_pay_response = boltz_session.prepare_pay(lightning_payment, refund_address, webhook) 

        fee = prepare_pay_response.fee()
        print(f"Fee: {fee}")
        boltz_fee = prepare_pay_response.boltz_fee()
        print(f"Boltz fee: {boltz_fee}")


        # Get the URI for payment
        uri = prepare_pay_response.uri()
        uri_address = prepare_pay_response.uri_address()
        uri_amount = prepare_pay_response.uri_amount()
        print(f"Pay to URI: {uri} {uri_amount} {uri_address}")

        # Build and send transaction to the URI
        print(f"Sending {uri_amount} sats to {uri_address}...")
        builder = network.tx_builder()
        lbtc_asset = network.policy_asset()
        builder.add_recipient(uri_address, uri_amount, lbtc_asset)

        # Create the PSET
        pset = builder.finish(wollet)

        # Sign the PSET
        signed_pset = signer.sign(pset)

        # Finalize and extract transaction
        finalized_pset = wollet.finalize(signed_pset)
        tx = finalized_pset.extract_tx()

        # Broadcast the transaction
        txid = esplora_client.broadcast(tx)
        print(f"Transaction broadcasted! TXID: {txid}")


        # Return early if skip_completion_thread is True
        if skip_completion_thread:
            print("Skipping to start the completing thread as requested.")
            return

        # Start thread to wait for completion
        thread = threading.Thread(target=pay_invoice_thread, args=(prepare_pay_response,))
        thread.daemon = True
        thread.start()
        print("Started thread to monitor payment completion.")

    except LwkError.MagicRoutingHint as e:
        print(f"Magic routing hint detected! Pay directly to: {e.uri} {e.address} {e.amount}")

        address = Address(e.address)
        # Build and send direct transaction to the routing hint
        print(f"Sending {e.amount} sats to {address}...")
        builder = network.tx_builder()
        lbtc_asset = network.policy_asset()
        builder.add_recipient(address, e.amount, lbtc_asset)

        # Create the PSET
        pset = builder.finish(wollet)

        # Sign the PSET
        signed_pset = signer.sign(pset)

        # Finalize and extract transaction
        finalized_pset = wollet.finalize(signed_pset)
        tx = finalized_pset.extract_tx()

        # Broadcast the transaction
        txid = esplora_client.broadcast(tx)
        print(f"Direct payment transaction broadcasted! TXID: {txid}")
        return

    except Exception as e:
        print(f"Error preparing payment: {e}")

def restorable_reverse_swaps(boltz_session, wollet):
    """Fetch reverse swaps for the wallet"""
    try:
        # Get the claim address for this wallet
        claim_address = wollet.address(None).address()
        print(f"Fetching reverse swaps for claim address: {claim_address}")

        # Fetch reverse swaps from Boltz
        swap_list = boltz_session.swap_restore()
        swap_data_list = boltz_session.restorable_reverse_swaps(swap_list, claim_address)

        if not swap_data_list:
            print("No reverse swaps found")
            return

        print(f"Found {len(swap_data_list)} reverse swap(s)\n")

        # Print each swap's JSON data
        for i, data in enumerate(swap_data_list, 1):
            print(f"=== Swap {i} ===")
            # Pretty print the JSON
            swap_data = json.loads(data)
            print(json.dumps(swap_data, indent=2))
            print()

            if should_start_completion_thread(swap_data):
                invoice_response = boltz_session.restore_invoice(data)
                thread = threading.Thread(
                    target=invoice_thread,
                    args=(invoice_response, claim_address),
                )
                thread.daemon = True
                thread.start()
                print(f"Started completion thread for restored swap {invoice_response.swap_id()}")
            else:
                print("Skipped completion thread startup.")
    except Exception as e:
        print(f"Error fetching reverse swaps: {e}")

def restorable_submarine_swaps(boltz_session, wollet):
    """Fetch submarine swaps for the wallet"""
    try:
        # Get the refund address for this wallet
        refund_address = wollet.address(None).address()
        print(f"Fetching submarine swaps for refund address: {refund_address}")

        # Fetch submarine swaps from Boltz
        swap_list = boltz_session.swap_restore()
        swap_data_list = boltz_session.restorable_submarine_swaps(swap_list, refund_address)

        if not swap_data_list:
            print("No submarine swaps found")
            return

        print(f"Found {len(swap_data_list)} submarine swap(s)\n")

        # Print each swap's JSON data
        for i, data in enumerate(swap_data_list, 1):
            print(f"=== Swap {i} ===")
            # Pretty print the JSON
            swap_data = json.loads(data)
            print(json.dumps(swap_data, indent=2))
            print()

            if should_start_completion_thread(swap_data):
                prepare_pay_response = boltz_session.restore_prepare_pay(data)
                thread = threading.Thread(
                    target=pay_invoice_thread,
                    args=(prepare_pay_response,),
                )
                thread.daemon = True
                thread.start()
                print(f"Started completion thread for restored swap {prepare_pay_response.swap_id()}")
            else:
                print("Skipped completion thread startup.")
    except Exception as e:
        print(f"Error fetching submarine swaps: {e}")

def restorable_btc_to_lbtc_swaps(boltz_session, wollet):
    """Fetch BTC to LBTC chain swaps for the wallet"""
    try:
        # Get the claim address for this wallet (Liquid)
        claim_address = wollet.address(None).address()
        print(f"Using claim address (Liquid): {claim_address}")

        # Ask for the Bitcoin refund address
        refund_address_str = input("Enter Bitcoin refund address: ").strip()
        refund_address = BitcoinAddress(refund_address_str)

        # Fetch restorable BTC to LBTC swaps from Boltz
        swap_list = boltz_session.swap_restore()
        swap_data_list = boltz_session.restorable_btc_to_lbtc_swaps(
            swap_list, claim_address, refund_address
        )

        if not swap_data_list:
            print("No BTC to LBTC chain swaps found")
            return

        print(f"Found {len(swap_data_list)} BTC to LBTC chain swap(s)\n")

        for i, data in enumerate(swap_data_list, 1):
            print(f"=== Swap {i} ===")
            swap_data = json.loads(data)
            print(json.dumps(swap_data, indent=2))
            print()

            if should_start_completion_thread(swap_data):
                lockup_response = boltz_session.restore_lockup(data)
                thread = threading.Thread(target=lockup_thread, args=(lockup_response,))
                thread.daemon = True
                thread.start()
                print(f"Started completion thread for restored swap {lockup_response.swap_id()}")
            else:
                print("Skipped completion thread startup.")
    except Exception as e:
        print(f"Error fetching BTC to LBTC chain swaps: {e}")

def restorable_lbtc_to_btc_swaps(boltz_session, wollet):
    """Fetch LBTC to BTC chain swaps for the wallet"""
    try:
        # Get the refund address for this wallet (Liquid)
        refund_address = wollet.address(None).address()
        print(f"Using refund address (Liquid): {refund_address}")

        # Ask for the Bitcoin claim address
        claim_address_str = input("Enter Bitcoin claim address: ").strip()
        claim_address = BitcoinAddress(claim_address_str)

        # Fetch restorable LBTC to BTC swaps from Boltz
        swap_list = boltz_session.swap_restore()
        swap_data_list = boltz_session.restorable_lbtc_to_btc_swaps(
            swap_list, claim_address, refund_address
        )

        if not swap_data_list:
            print("No LBTC to BTC chain swaps found")
            return

        print(f"Found {len(swap_data_list)} LBTC to BTC chain swap(s)\n")

        for i, data in enumerate(swap_data_list, 1):
            print(f"=== Swap {i} ===")
            swap_data = json.loads(data)
            print(json.dumps(swap_data, indent=2))
            print()

            if should_start_completion_thread(swap_data):
                lockup_response = boltz_session.restore_lockup(data)
                thread = threading.Thread(target=lockup_thread, args=(lockup_response,))
                thread.daemon = True
                thread.start()
                print(f"Started completion thread for restored swap {lockup_response.swap_id()}")
            else:
                print("Skipped completion thread startup.")
    except Exception as e:
        print(f"Error fetching LBTC to BTC chain swaps: {e}")

def list_all_swaps(boltz_session):
    """List all swaps for the lightning session"""
    try:
        # Fetch all swaps from Boltz
        swap_list = boltz_session.swap_restore()

        # Convert to JSON string and parse
        swaps_json = str(swap_list)
        print(f"Swaps JSON: {swaps_json}")
        swaps_data = json.loads(swaps_json)

        if not swaps_data:
            print("No swaps found")
            return

        print(f"Found {len(swaps_data)} swap(s)\n")

        # Print each swap's basic info
        for i, swap in enumerate(swaps_data, 1):
            print(f"=== Swap {i} ===")
            print(f"ID: {swap.get('id', 'N/A')}")
            print(f"Status: {swap.get('status', 'N/A')}")
            print(f"Type: {swap.get('type', 'N/A')}")
    except Exception as e:
        print(f"Error listing all swaps: {e}")

def show_swaps_info(boltz_session):
    """Show swaps info from Boltz API"""
    try:
        # Fetch swaps info from Boltz
        swaps_info_json = boltz_session.fetch_swaps_info()

        # Parse and pretty print the JSON
        swaps_info = json.loads(swaps_info_json)
        print(json.dumps(swaps_info, indent=2))
    except Exception as e:
        print(f"Error fetching swaps info: {e}")

def get_quote(boltz_session):
    """Get a quote for a swap showing fees before creating it"""
    # Refresh swap info to ensure quote uses latest fees/limits
    boltz_session.refresh_swap_info()
    
    # Ask whether to specify send or receive amount
    print("\nQuote by:")
    print("  1) Send amount (how much will I receive?)")
    print("  2) Receive amount (how much do I need to send?)")
    
    while True:
        mode_choice = input("Select mode (1-2): ").strip()
        if mode_choice == '1':
            quote_by_receive = False
            break
        elif mode_choice == '2':
            quote_by_receive = True
            break
        else:
            print("Invalid choice. Please enter 1 or 2.")
    
    print("\nSwap Asset Types:")
    print("  1) Lightning BTC")
    print("  2) Onchain BTC")
    print("  3) Liquid")
    
    # Get 'from' asset
    while True:
        from_choice = input("Select source asset (1-3): ").strip()
        if from_choice == '1':
            from_asset = SwapAsset.LIGHTNING
            from_name = "Lightning BTC"
            break
        elif from_choice == '2':
            from_asset = SwapAsset.ONCHAIN
            from_name = "Onchain BTC"
            break
        elif from_choice == '3':
            from_asset = SwapAsset.LIQUID
            from_name = "Liquid"
            break
        else:
            print("Invalid choice. Please enter 1, 2, or 3.")
    
    # Get 'to' asset
    while True:
        to_choice = input("Select destination asset (1-3): ").strip()
        if to_choice == '1':
            to_asset = SwapAsset.LIGHTNING
            to_name = "Lightning BTC"
            break
        elif to_choice == '2':
            to_asset = SwapAsset.ONCHAIN
            to_name = "Onchain BTC"
            break
        elif to_choice == '3':
            to_asset = SwapAsset.LIQUID
            to_name = "Liquid"
            break
        else:
            print("Invalid choice. Please enter 1, 2, or 3.")
    
    # Get amount
    if quote_by_receive:
        prompt = "Enter desired receive amount in satoshis: "
    else:
        prompt = "Enter send amount in satoshis: "
    
    while True:
        try:
            amount_str = input(prompt).strip()
            amount = int(amount_str)
            if amount <= 0:
                print("Amount must be positive. Please try again.")
                continue
            break
        except ValueError:
            print("Invalid amount. Please enter a valid number.")
    
    try:
        # Create quote using appropriate method
        if quote_by_receive:
            builder = boltz_session.quote_receive(amount)
        else:
            builder = boltz_session.quote(amount)
        
        builder.send(from_asset)
        builder.receive(to_asset)
        quote = builder.build()
        
        print(f"\n=== Quote: {from_name} → {to_name} ===")
        print(f"Send amount:    {quote.send_amount:,} sats")
        print(f"Receive amount: {quote.receive_amount:,} sats")
        print(f"Network fee:    {quote.network_fee:,} sats")
        print(f"Boltz fee:      {quote.boltz_fee:,} sats")
        print(f"Min amount:     {quote.min:,} sats")
        print(f"Max amount:     {quote.max:,} sats")
        
    except Exception as e:
        # Handle errors - invalid swap pairs will raise an exception
        error_str = str(e)
        if "InvalidSwapPair" in error_str:
            print(f"Invalid swap pair: {from_name} → {to_name} is not supported")
        elif "PairNotAvailable" in error_str:
            print(f"Pair not available: {from_name} → {to_name}")
        else:
            print(f"Error getting quote: {e}")

def lbtc_to_btc_swap(boltz_session, wollet, esplora_client, signer):
    """Create a swap to convert LBTC to BTC"""
    # Ask for the swap amount
    while True:
        try:
            amount_str = input("Enter amount in satoshis to swap from LBTC to BTC: ").strip()
            amount = int(amount_str)
            if amount <= 0:
                print("Amount must be positive. Please try again.")
                continue
            break
        except ValueError:
            print("Invalid amount. Please enter a valid number.")

    # Ask for the Bitcoin claim address
    claim_address_str = input("Enter Bitcoin address to receive BTC: ").strip()

    # Get a Liquid refund address from the wallet
    refund_address = wollet.address(None).address()
    print(f"Refund address (Liquid): {refund_address}")

    try:
        claim_address = BitcoinAddress(claim_address_str)

        # Create the swap
        webhook_url = os.getenv('WEBHOOK')
        webhook = WebHook(webhook_url, status=[]) if webhook_url else None
        lockup_response = boltz_session.lbtc_to_btc(amount, refund_address, claim_address, webhook)

        # Get swap details
        swap_id = lockup_response.swap_id()
        lockup_address = lockup_response.lockup_address()
        expected_amount = lockup_response.expected_amount()
        from_chain = lockup_response.chain_from()
        to_chain = lockup_response.chain_to()

        print(f"Swap ID: {swap_id}")
        print(f"Lockup address: {lockup_address}")
        print(f"Expected amount: {expected_amount}")
        print(f"From chain: {from_chain}")
        print(f"To chain: {to_chain}")

        # Build and send transaction to the lockup address
        print(f"Sending {expected_amount} sats to {lockup_address}...")
        builder = network.tx_builder()
        lbtc_asset = network.policy_asset()
        lockup_addr = Address(lockup_address)
        builder.add_recipient(lockup_addr, expected_amount, lbtc_asset)

        # Create the PSET
        pset = builder.finish(wollet)

        # Sign the PSET
        signed_pset = signer.sign(pset)

        # Finalize and extract transaction
        finalized_pset = wollet.finalize(signed_pset)
        tx = finalized_pset.extract_tx()

        # Broadcast the transaction
        txid = esplora_client.broadcast(tx)
        print(f"Transaction broadcasted! TXID: {txid}")

        # Start thread to monitor swap completion
        thread = threading.Thread(target=lockup_thread, args=(lockup_response,))
        thread.daemon = True
        thread.start()
        print("Started thread to monitor chain swap completion.")

    except Exception as e:
        print(f"Error creating LBTC to BTC swap: {e}")

def btc_to_lbtc_swap(boltz_session, wollet):
    """Create a swap to convert BTC to LBTC"""
    # Ask for the swap amount
    while True:
        try:
            amount_str = input("Enter amount in satoshis to swap from BTC to LBTC: ").strip()
            amount = int(amount_str)
            if amount <= 0:
                print("Amount must be positive. Please try again.")
                continue
            break
        except ValueError:
            print("Invalid amount. Please enter a valid number.")

    # Get a Liquid claim address from the wallet
    claim_address = wollet.address(None).address()
    print(f"Claim address (Liquid): {claim_address}")

    # Ask for the Bitcoin refund address
    refund_address_str = input("Enter Bitcoin address for refunds: ").strip()

    try:
        refund_address = BitcoinAddress(refund_address_str)
        
        # Create the swap
        webhook_url = os.getenv('WEBHOOK')
        webhook = WebHook(webhook_url, status=[]) if webhook_url else None
        lockup_response = boltz_session.btc_to_lbtc(amount, refund_address, claim_address, webhook)

        # Get swap details
        swap_id = lockup_response.swap_id()
        lockup_address = lockup_response.lockup_address()
        expected_amount = lockup_response.expected_amount()
        from_chain = lockup_response.chain_from()
        to_chain = lockup_response.chain_to()

        print(f"\nSwap ID: {swap_id}")
        print(f"From chain: {from_chain}")
        print(f"To chain: {to_chain}")
        print(f"\n***** PLEASE SEND {expected_amount} sats from your Bitcoin wallet to: {lockup_address} *****\n")

        # Start thread to monitor swap completion
        thread = threading.Thread(target=lockup_thread, args=(lockup_response,))
        thread.daemon = True
        thread.start()
        print("Started thread to monitor chain swap completion.")

    except Exception as e:
        print(f"Error creating BTC to LBTC swap: {e}")

def main():
    # Get mnemonic from environment variable
    mnemonic_str = os.getenv('MNEMONIC')
    if not mnemonic_str:
        print("Error: MNEMONIC environment variable not set")
        return

    polling = os.getenv('POLLING')
    if polling:
        polling = True
    else:
        polling = False
    print(f"Polling: {polling}")

    mnemonic = Mnemonic(mnemonic_str)
    global network
    network = Network.mainnet()

    # Create the file-based store for swap persistence
    # Note: Keys are automatically namespaced by a hash of the mnemonic internally,
    # so multiple wallets can safely share the same store directory.
    file_store = FileStore("swaps")
    store_link = ForeignStoreLink(file_store)

    b = EsploraClientBuilder(
        base_url="https://waterfalls.liquidwebwallet.org/liquid/api",
        network=network,
        waterfalls=True,
        utxo_only=True,
    )
    esplora_client = EsploraClient.from_builder(b)

    signer = Signer(mnemonic, network)
    desc = signer.wpkh_slip77_descriptor()

    print("Wollet descriptor: ", desc)
    webwallet = f"https://liquidwebwallet.org/#{desc.url_encoded_descriptor()}"
    print("online watch-only:", webwallet)

    # Create POS config for btcpos.cash
    usd = CurrencyCode("USD")
    pos_config = PosConfig(desc, usd)
    pos_encoded = pos_config.encode()
    btcpos_link = f"https://btcpos.cash/#{pos_encoded}"
    print("POS terminal:", btcpos_link)

    wollet = Wollet(network, desc, datadir=None)

    mnemonic_derivation_index = os.getenv('MNEMONIC_DERIVATION_INDEX')
    if mnemonic_derivation_index:
        mnemonic_derivation_index = int(mnemonic_derivation_index)
    else:
        mnemonic_derivation_index = 26589
    mnemonic_lightning = signer.derive_bip85_mnemonic(mnemonic_derivation_index, 12) # for security reasons using a different mnemonic for the lightning session
    lightning_client = AnyClient.from_esplora(esplora_client)
    logger = MyLogger()

    # Use blockstream electrum instance for bitcoin, without specifying this, bull bitcoin electrum instances will be used
    bitcoin_electrum_url = "ssl://bitcoin-mainnet.blockstream.info:50002"

    builder = BoltzSessionBuilder(
        network=network,
        client=lightning_client,
        timeout=30,
        mnemonic=mnemonic_lightning,
        logging=logger,
        polling=polling,
        referral_id="LWK python example",
        bitcoin_electrum_client_url=bitcoin_electrum_url,
        random_preimages=True,
        store=store_link,  # Pass the store for automatic swap persistence
    )
    boltz_session = BoltzSession.from_builder(builder)

    # Initial balance update
    update_balance(wollet, esplora_client, desc)

    # Find and restore unfinished swaps from the store
    print("Checking for unfinished swaps...")
    pending_ids = boltz_session.pending_swap_ids()
    if pending_ids is None:
        print("No store configured")
    elif not pending_ids:
        print("No unfinished swaps found")
    else:
        print(f"Found {len(pending_ids)} pending swap(s)")
        for swap_id in pending_ids:
            try:
                data = boltz_session.get_swap_data(swap_id)
                if data is None:
                    print(f"Swap data not found for {swap_id}")
                    continue

                # Parse JSON to determine swap type
                swap_data = json.loads(data)
                swap_type = swap_data.get('swap_type')

                if swap_type == 'submarine':
                    print(f"Restoring submarine swap {swap_id}...")
                    try:
                        prepare_pay_response = boltz_session.restore_prepare_pay(data)
                        # Start thread to monitor payment
                        thread = threading.Thread(target=pay_invoice_thread, args=(prepare_pay_response,))
                        thread.daemon = True
                        thread.start()
                        print(f"Started monitoring thread for submarine swap {swap_id}")
                    except Exception as e:
                        print(f"Error restoring submarine swap {swap_id}: {e}")

                elif swap_type == 'reverse':
                    print(f"Restoring reverse swap {swap_id}...")
                    try:
                        invoice_response = boltz_session.restore_invoice(data)
                        # Start thread to monitor invoice
                        thread = threading.Thread(target=invoice_thread, args=(invoice_response, wollet.address(None).address()))
                        thread.daemon = True
                        thread.start()
                        print(f"Started monitoring thread for reverse swap {swap_id}")
                    except Exception as e:
                        print(f"Error restoring reverse swap {swap_id}: {e}")

                elif swap_type == 'chain':
                    print(f"Restoring chain swap {swap_id}...")
                    try:
                        lockup_response = boltz_session.restore_lockup(data)
                        # Start thread to monitor chain swap
                        thread = threading.Thread(target=lockup_thread, args=(lockup_response,))
                        thread.daemon = True
                        thread.start()
                        print(f"Started monitoring thread for chain swap {swap_id}")
                    except Exception as e:
                        print(f"Error restoring chain swap {swap_id}: {e}")
                else:
                    print(f"Unknown swap type '{swap_type}' for swap {swap_id}")

            except Exception as e:
                print(f"Error restoring swap {swap_id}: {e}")

    # Main menu loop
    while True:
        print("\n=== Menu ===")
        print("1) Update balance")
        print("2) Show invoice (reverse)")
        print("3) Pay invoice (submarine)")
        print("4) Pay invoice (submarine) (but don't start completion thread)")
        print("5) Generate rescue file")
        print("6) Fetch restorable reverse swaps")
        print("7) Fetch restorable submarine swaps")
        print("8) Fetch restorable BTC to LBTC swaps")
        print("9) Fetch restorable LBTC to BTC swaps")
        print("10) List all swaps (from Boltz API)")
        print("11) Show swaps info")
        print("12) Swap LBTC to BTC (chain swap)")
        print("13) Swap BTC to LBTC (chain swap) (requires external btc wallet)")
        print("14) Get swap quote")
        print("15) List pending swaps (from local store)")
        print("16) List completed swaps (from local store)")
        print("17) Remove swap from store")
        print("q) Quit")

        choice = input("Choose option: ").strip().lower()

        if choice == '1':
            print("\n=== Updating Balance ===")
            update_balance(wollet, esplora_client, desc)
        elif choice == '2':
            print("\n=== Creating Invoice ===")
            show_invoice(boltz_session, wollet)
        elif choice == '3':
            print("\n=== Paying Invoice ===")
            pay_invoice(boltz_session, wollet, esplora_client, signer)
        elif choice == '4':
            print("\n=== Paying Invoice (but don't start completion thread) ===")
            pay_invoice(boltz_session, wollet, esplora_client, signer, skip_completion_thread=True)
        elif choice == '5':
            print("\n=== Generating Rescue File ===")
            try:
                rescue_data = boltz_session.rescue_file()
                # Compute hash of the rescue data
                rescue_hash = hashlib.sha256(rescue_data.encode('utf-8')).hexdigest()[:16]
                filename = f"rescue_file_{rescue_hash}.json"

                with open(filename, "w") as f:
                    f.write(rescue_data)

                print(f"Rescue file generated: {filename}")
            except Exception as e:
                print(f"Error generating rescue file: {e}")
        elif choice == '6':
            print("\n=== Fetching Reverse Swaps ===")
            restorable_reverse_swaps(boltz_session, wollet)
        elif choice == '7':
            print("\n=== Fetching Submarine Swaps ===")
            restorable_submarine_swaps(boltz_session, wollet)
        elif choice == '8':
            print("\n=== Fetching BTC to LBTC Chain Swaps ===")
            restorable_btc_to_lbtc_swaps(boltz_session, wollet)
        elif choice == '9':
            print("\n=== Fetching LBTC to BTC Chain Swaps ===")
            restorable_lbtc_to_btc_swaps(boltz_session, wollet)
        elif choice == '10':
            print("\n=== Listing All Swaps ===")
            list_all_swaps(boltz_session)
        elif choice == '11':
            print("\n=== Showing Swaps Info ===")
            show_swaps_info(boltz_session)
        elif choice == '12':
            print("\n=== Swapping LBTC to BTC ===")
            lbtc_to_btc_swap(boltz_session, wollet, esplora_client, signer)
        elif choice == '13':
            print("\n=== Swapping BTC to LBTC ===")
            btc_to_lbtc_swap(boltz_session, wollet)
        elif choice == '14':
            print("\n=== Get Swap Quote ===")
            get_quote(boltz_session)
        elif choice == '15':
            print("\n=== Pending Swaps (from local store) ===")
            pending = boltz_session.pending_swap_ids()
            if pending is None:
                print("No store configured")
            elif not pending:
                print("No pending swaps")
            else:
                print(f"Found {len(pending)} pending swap(s):")
                for swap_id in pending:
                    data = boltz_session.get_swap_data(swap_id)
                    if data:
                        swap_data = json.loads(data)
                        swap_type = swap_data.get('swap_type', 'unknown')
                        last_state = swap_data.get('last_state', 'unknown')
                        print(f"  - {swap_id} ({swap_type}) - state: {last_state}")
                    else:
                        print(f"  - {swap_id} (data not found)")
        elif choice == '16':
            print("\n=== Completed Swaps (from local store) ===")
            completed = boltz_session.completed_swap_ids()
            if completed is None:
                print("No store configured")
            elif not completed:
                print("No completed swaps")
            else:
                print(f"Found {len(completed)} completed swap(s):")
                for swap_id in completed:
                    data = boltz_session.get_swap_data(swap_id)
                    if data:
                        swap_data = json.loads(data)
                        swap_type = swap_data.get('swap_type', 'unknown')
                        last_state = swap_data.get('last_state', 'unknown')
                        print(f"  - {swap_id} ({swap_type}) - state: {last_state}")
                    else:
                        print(f"  - {swap_id} (data not found)")
        elif choice == '17':
            print("\n=== Remove Swap from Store ===")
            swap_id = input("Enter swap ID to remove: ").strip()
            if swap_id:
                removed = boltz_session.remove_swap(swap_id)
                if removed:
                    print(f"Swap {swap_id} removed from store")
                else:
                    print("No store configured or swap not found")
            else:
                print("No swap ID provided")
        elif choice == 'q':
            print("Goodbye!")
            break
        else:
            print("Invalid choice. Please try again.")

if __name__ == "__main__":
    main()
