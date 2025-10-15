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

def delete_swap_data(swap_id):
    """Delete swap data file"""
    swap_file = f"{swaps_dir}/{swap_id}.json"
    try:
        os.remove(swap_file)
        print(f"Swap data deleted: {swap_file}")
    except FileNotFoundError:
        print(f"Swap data file not found: {swap_file}")
    except Exception as e:
        print(f"Error deleting swap data: {e}")

def rename_swap_data(swap_id):
    """Move swap data file to failed directory"""
    src_file = f"{swaps_dir}/{swap_id}.json"
    dst_file = f"{swaps_dir}/failed/{swap_id}.json"
    try:
        shutil.move(src_file, dst_file)
        print(f"Swap data moved to failed: {dst_file}")
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
    while True:
        try:
            state = invoice_response.advance()
            swap_id = invoice_response.swap_id()
            if state == PaymentState.CONTINUE:
                data = invoice_response.serialize()
                save_swap_data(swap_id, data)
            elif state == PaymentState.SUCCESS:
                print("Invoice payment completed successfully!")
                delete_swap_data(swap_id)
                break
            elif state == PaymentState.FAILED:
                print("Invoice payment failed!")
                rename_swap_data(swap_id)
                break
               
        except Exception as e:
            print(f"Error in invoice thread: {e}")
            break

def pay_invoice_thread(prepare_pay_response):
    """Thread function to handle payment completion"""
    print("Waiting for payment completion...")
    while True:
        try:
            state = prepare_pay_response.advance()
            if state == PaymentState.CONTINUE:
                swap_id = prepare_pay_response.swap_id()
                data = prepare_pay_response.serialize()
                save_swap_data(swap_id, data)
            elif state == PaymentState.SUCCESS:
                print("Payment completed successfully!")
                break
            elif state == PaymentState.FAILED:
                print("Payment failed!")
                break
            
        except Exception as e:
            print(f"Error in payment thread: {e} last_state: {state}")
            break

def show_invoice(lightning_session, wollet):
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
    invoice_response = lightning_session.invoice(amount, "Lightning payment", claim_address, None) # optionally accept a WebHook("https://example.com/webhook")

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

def pay_invoice(lightning_session, wollet, esplora_client, signer):
    """Pay a bolt11 invoice"""
    # Read bolt11 invoice from user
    bolt11_str = input("Enter bolt11 invoice: ").strip()

    try:
        # Parse the invoice
        bolt11_invoice_obj = Bolt11Invoice(bolt11_str)

        # Get refund address
        refund_address = wollet.address(None).address()
        print(f"Refund address: {refund_address}")

        # Prepare payment
        prepare_pay_response = lightning_session.prepare_pay(bolt11_invoice_obj, refund_address, None) # optionally accept a WebHook("https://example.com/webhook")

        data=prepare_pay_response.serialize()
        swap_id=prepare_pay_response.swap_id()


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

        # Save swap data to file
        save_swap_data(swap_id, data)

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

def main():
    # Get mnemonic from environment variable
    mnemonic_str = os.getenv('MNEMONIC')
    if not mnemonic_str:
        print("Error: MNEMONIC environment variable not set")
        return

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

    # Create wallet components
    # Use ElectrumClient only for LightningSession
    electrum_client = network.default_electrum_client()
    # Use EsploraClient with waterfalls for all other operations
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

    # Create lightning session with ElectrumClient
    mnemonic_lightning = signer.derive_bip85_mnemonic(0, 12) # for security reasons using a different mnemonic for the lightning session
    logger = MyLogger()
    lightning_session = LightningSession(network, electrum_client, 30, logger, mnemonic_lightning)  # 30 second timeout

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

            if swap_type == 'Submarine':
                print(f"Restoring submarine swap {swap_id}...")
                prepare_pay_response = lightning_session.restore_prepare_pay(data)
                # Start thread to monitor payment
                thread = threading.Thread(target=pay_invoice_thread, args=(prepare_pay_response,))
                thread.daemon = True
                thread.start()
                print(f"Started monitoring thread for submarine swap {swap_id}")

            elif swap_type == 'Reverse':
                print(f"Restoring reverse swap {swap_id}...")
                invoice_response = lightning_session.restore_invoice(data)
                # Start thread to monitor invoice
                thread = threading.Thread(target=invoice_thread, args=(invoice_response, wollet.address(None).address()))
                thread.daemon = True
                thread.start()
                print(f"Started monitoring thread for reverse swap {swap_id}")

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
        print("4) Generate rescue file")
        print("q) Quit")

        choice = input("Choose option: ").strip().lower()

        if choice == '1':
            print("\n=== Updating Balance ===")
            update_balance(wollet, esplora_client, desc)
        elif choice == '2':
            print("\n=== Creating Invoice ===")
            show_invoice(lightning_session, wollet)
        elif choice == '3':
            print("\n=== Paying Invoice ===")
            pay_invoice(lightning_session, wollet, esplora_client, signer)
        elif choice == '4':
            print("\n=== Generating Rescue File ===")
            try:
                rescue_data = lightning_session.rescue_file()
                # Compute hash of the rescue data
                rescue_hash = hashlib.sha256(rescue_data.encode('utf-8')).hexdigest()[:16]
                filename = f"rescue_file_{rescue_hash}.json"

                with open(filename, "w") as f:
                    f.write(rescue_data)

                print(f"Rescue file generated: {filename}")
            except Exception as e:
                print(f"Error generating rescue file: {e}")
        elif choice == 'q':
            print("Goodbye!")
            break
        else:
            print("Invalid choice. Please try again.")

if __name__ == "__main__":
    main()
