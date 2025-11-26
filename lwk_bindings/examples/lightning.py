import glob
import hashlib
import json
import os
import shutil
import threading
import time
from lwk import *

def save_swap_data(swap_id, data):
    """Save swap data to a JSON file"""
    swap_file = f"{swaps_dir}/{swap_id}.json"
    with open(swap_file, "w") as f:
        f.write(data)
    print(f"Swap data saved to {swap_file}")


def rename_swap_data(swap_id, status):
    """Move swap data file to completed or failed directory
    
    Args:
        swap_id: The swap identifier
        status: Either "completed" or "failed"
    """
    src_file = f"{swaps_dir}/{swap_id}.json"
    dst_file = f"{swaps_dir}/{status}/{swap_id}.json"
    try:
        shutil.move(src_file, dst_file)
        print(f"Swap data moved to {status}: {dst_file}")
    except FileNotFoundError:
        print(f"Swap data file not found: {src_file}")
    except Exception as e:
        print(f"Error moving swap data: {e}")

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
            if state == PaymentState.CONTINUE:
                data = invoice_response.serialize()
                save_swap_data(swap_id, data)
            elif state == PaymentState.SUCCESS:
                print("Invoice payment completed successfully!")
                rename_swap_data(swap_id, "completed")
                break
            elif state == PaymentState.FAILED:
                print("Invoice payment failed!")
                rename_swap_data(swap_id, "failed")
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
            if state == PaymentState.CONTINUE:
                data = prepare_pay_response.serialize()
                save_swap_data(swap_id, data)
            elif state == PaymentState.SUCCESS:
                print("Payment completed successfully!")
                rename_swap_data(swap_id, "completed")
                break
            elif state == PaymentState.FAILED:
                print("Payment failed!")
                rename_swap_data(swap_id, "failed")
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
            if state == PaymentState.CONTINUE:
                data = lockup_response.serialize()
                save_swap_data(swap_id, data)
            elif state == PaymentState.SUCCESS:
                print("Chain swap completed successfully!")
                rename_swap_data(swap_id, "completed")
                break
            elif state == PaymentState.FAILED:
                print("Chain swap failed!")
                rename_swap_data(swap_id, "failed")
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

    # Get and print the bolt11 invoice
    bolt11_invoice_obj = invoice_response.bolt11_invoice()
    bolt11_invoice = str(bolt11_invoice_obj)
    print(f"Invoice: {bolt11_invoice}")

    data=invoice_response.serialize()
    swap_id=invoice_response.swap_id()

    # Save swap data to file
    save_swap_data(swap_id, data)

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

        data=prepare_pay_response.serialize()
        swap_id=prepare_pay_response.swap_id()

        # Save swap data to file
        save_swap_data(swap_id, data)

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
    except Exception as e:
        print(f"Error fetching submarine swaps: {e}")

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
    claim_address = input("Enter Bitcoin address to receive BTC: ").strip()

    # Get a Liquid refund address from the wallet
    refund_address = wollet.address(None).address()
    print(f"Refund address (Liquid): {refund_address}")

    try:
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

        # Save swap data
        data = lockup_response.serialize()
        save_swap_data(swap_id, data)

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
    refund_address = input("Enter Bitcoin address for refunds: ").strip()

    try:
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

        # Save swap data
        data = lockup_response.serialize()
        save_swap_data(swap_id, data)

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
    global swaps_dir
    # Compute hash of mnemonic for wallet-specific directory
    mnemonic_hash = hashlib.sha256(mnemonic_str.encode('utf-8')).hexdigest()[:16]
    swaps_dir = "swaps/{}".format(mnemonic_hash)

    os.makedirs("swaps", exist_ok=True)
    os.makedirs(swaps_dir, exist_ok=True)
    os.makedirs(f"{swaps_dir}/failed", exist_ok=True)
    os.makedirs(f"{swaps_dir}/completed", exist_ok=True)

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

    wollet = Wollet(network, desc, datadir=None)

    mnemonic_lightning = signer.derive_bip85_mnemonic(0, 12) # for security reasons using a different mnemonic for the lightning session
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
    )
    boltz_session = BoltzSession.from_builder(builder)

    # Initial balance update
    update_balance(wollet, esplora_client, desc)

    # Find and restore unfinished swaps
    print("Checking for unfinished swaps...")
    swap_files = glob.glob(f"{swaps_dir}/*.json")
    if not swap_files:
        print("No unfinished swaps found")
    for swap_file in swap_files:
        try:
            with open(swap_file, 'r') as f:
                data = f.read()

            # Parse JSON to determine swap type
            swap_data = json.loads(data)
            swap_type = swap_data.get('swap_type')
            swap_id = swap_data.get('create_swap_response', {}).get('id') or swap_data.get('create_reverse_response', {}).get('id')

            if swap_type == 'submarine':
                print(f"Restoring submarine swap {swap_id}...")
                try:
                    prepare_pay_response = boltz_session.restore_prepare_pay(data)
                    # Start thread to monitor payment
                    thread = threading.Thread(target=pay_invoice_thread, args=(prepare_pay_response,))
                    thread.daemon = True
                    thread.start()
                    print(f"Started monitoring thread for submarine swap {swap_id}")
                except LwkError.SwapExpired as e:
                    print(f"Submarine swap {swap_id} has expired, moving to failed directory")
                    rename_swap_data(swap_id, "failed")
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
                except LwkError.SwapExpired as e:
                    print(f"Reverse swap {swap_id} has expired, moving to failed directory")
                    rename_swap_data(swap_id, "failed")
                except Exception as e:
                    print(f"Error restoring reverse swap {swap_id}: {e}")
            elif swap_type == 'chain':
                print(f"Restoring chain swap {swap_id}...")
                try:
                    lockup_response = boltz_session.restore_lockup(data)
                    # Start thread to monitor invoice
                    thread = threading.Thread(target=lockup_thread, args=(lockup_response,))
                    thread.daemon = True
                    thread.start()
                    print(f"Started monitoring thread for chain swap {swap_id}")
                except LwkError.SwapExpired as e:
                    print(f"Chain swap {swap_id} has expired, moving to failed directory")
                    rename_swap_data(swap_id, "failed")
                except Exception as e:
                    print(f"Error restoring chain swap {swap_id}: {e}")
            else:
                print(f"Unknown swap type '{swap_type}' in file {swap_file}")

        except Exception as e:
            print(f"Error restoring swap from {swap_file}: {e}")

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
        print("8) List all swaps")
        print("9) Show swaps info")
        print("10) Swap LBTC to BTC (chain swap)")
        print("11) Swap BTC to LBTC (chain swap) (requires external btc wallet)")
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
            print("\n=== Listing All Swaps ===")
            list_all_swaps(boltz_session)
        elif choice == '9':
            print("\n=== Showing Swaps Info ===")
            show_swaps_info(boltz_session)
        elif choice == '10':
            print("\n=== Swapping LBTC to BTC ===")
            lbtc_to_btc_swap(boltz_session, wollet, esplora_client, signer)
        elif choice == '11':
            print("\n=== Swapping BTC to LBTC ===")
            btc_to_lbtc_swap(boltz_session, wollet)
        elif choice == 'q':
            print("Goodbye!")
            break
        else:
            print("Invalid choice. Please try again.")

if __name__ == "__main__":
    main()
