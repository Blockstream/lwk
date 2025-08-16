extern crate lwk_wollet;

use lwk_common::Signer;
use lwk_signer::SwSigner;
use lwk_wollet::{
    clients::blocking::BlockchainBackend,
    elements::{Address, AssetId, Txid},
    ElectrumClient, ElementsNetwork, ExternalUtxo, NoPersist, Wollet, WolletDescriptor,
};

/// Fee Service implementation that helps users to send an issued asset without paying transaction fees
pub struct FeeService {
    signer: SwSigner,
    wollet: Wollet,
    electrum_client: ElectrumClient,
    network: ElementsNetwork,
}

impl FeeService {
    /// Create a new Fee Service with custom descriptor
    pub fn new(mnemonic: &str, descriptor_str: &str, electrum_url: &str, network: ElementsNetwork) -> Result<Self, Box<dyn std::error::Error>> {
        let signer = SwSigner::new(mnemonic, false)?;
        
        let descriptor: WolletDescriptor = descriptor_str.parse()?;
        let wollet = Wollet::new(network, NoPersist::new(), descriptor)?;

        let electrum_url = electrum_url.parse()?;
        let electrum_client = ElectrumClient::new(&electrum_url)?;

        Ok(FeeService {
            signer,
            wollet,
            electrum_client,
            network,
        })
    }



    /// Get a new address from the fee service
    pub fn get_address(&self) -> Result<Address, Box<dyn std::error::Error>> {
        let address_result = self.wollet.address(None)?;
        Ok(address_result.address().clone())
    }


    /// Convert internal UTXO to external format for use by other wallets
    fn make_external_utxo(&self, utxo: &lwk_wollet::WalletTxOut) -> Result<ExternalUtxo, Box<dyn std::error::Error>> {
        // Get the full transaction
        let transactions = self.electrum_client.get_transactions(&[utxo.outpoint.txid])?;
        let tx = transactions.into_iter().next()
            .ok_or("Transaction not found")?;
        
        // Extract the specific output
        let txout = tx.output.get(utxo.outpoint.vout as usize)
            .ok_or("Invalid output index")?
            .clone();
        
        // For non-segwit descriptors, include full transaction
        let full_tx = if self.wollet.is_segwit() {
            None
        } else {
            Some(tx)
        };
        
        Ok(ExternalUtxo {
            outpoint: utxo.outpoint,
            txout,
            tx: full_tx,
            unblinded: utxo.unblinded.clone(),
            max_weight_to_satisfy: self.wollet.max_weight_to_satisfy(),
        })
    }

    /// Send an asset from user wallet using fee service to pay transaction fees
    /// This is the core functionality: user can send assets without needing LBTC
    pub fn send_asset_with_fee_service(
        &mut self,
        user_mnemonic: &str,
        user_descriptor: &str,
        asset_id: &str,
        amount: u64,
        recipient_address: &str,
    ) -> Result<Txid, Box<dyn std::error::Error>> {
        // Parse inputs
        let asset_id: AssetId = asset_id.parse()?;
        let recipient: Address = recipient_address.parse()?;
        
        println!("=== Fee Service Transaction ===");
        println!("Asset ID: {}", asset_id);
        println!("Amount: {} units", amount);
        println!("Recipient: {}", recipient_address);
        
        // Create user wallet
        let user_signer = SwSigner::new(user_mnemonic, false)?;
        let user_descriptor = user_descriptor.parse()?;
        let mut user_wollet = Wollet::new(self.network, NoPersist::new(), user_descriptor)?;
        
        // Sync fee service wallet
        let update = self.electrum_client.full_scan(&self.wollet)?;
        if let Some(update) = update {
            self.wollet.apply_update(update)?;
        }
        
        // Sync user wallet
        let update = self.electrum_client.full_scan(&user_wollet)?;
        if let Some(update) = update {
            user_wollet.apply_update(update)?;
        }
        
        // Check user has the asset
        let user_balance = user_wollet.balance()?;
        let user_asset_balance = user_balance.get(&asset_id).copied().unwrap_or(0);
        println!("User asset balance: {} units", user_asset_balance);
        
        if user_asset_balance < amount {
            return Err(format!(
                "Insufficient asset balance. Available: {} units, Required: {} units",
                user_asset_balance, amount
            ).into());
        }
        
        // Check fee service has LBTC for fees
        let lbtc = self.wollet.policy_asset();
        let fee_service_balance = self.wollet.balance().unwrap_or_default().get(&lbtc).copied().unwrap_or(0);
        println!("Fee service LBTC balance: {} sats", fee_service_balance);
        
        if fee_service_balance == 0 {
            return Err("Fee service has no LBTC to pay transaction fees".into());
        }
        
        // Get a UTXO from the Fee Service for paying fees
        let fee_utxo = self.wollet
            .utxos()?
            .into_iter()
            .find(|u| u.unblinded.asset == lbtc)
            .ok_or("Fee service has no LBTC UTXO available")?;
        
        // Store the UTXO value for change calculation
        let fee_utxo_value = fee_utxo.unblinded.value;
        println!("Fee service UTXO value: {} sats", fee_utxo_value);
        
        // Convert to external UTXO format
        let external_fee_utxo = self.make_external_utxo(&fee_utxo)?;
        
        // Get fee service address for LBTC change
        let fee_service_address = self.get_address()?;
        
        // Estimate transaction fees
        // Liquid Network uses discount vsize for confidential transactions
        // Typical fee: ~50-60 sats for a 2-input, 3-output transaction
        let estimated_discount_vsize = 530; // Conservative estimate
        let fee_rate = 100.0; // sats per kilobyte (0.1 sat/vbyte)
        let estimated_fee = ((estimated_discount_vsize as f32 * fee_rate / 1000.0).ceil() as u64).max(50); // minimum 50 sats
        println!("Estimated fee: {} sats (based on ~{} discount vbytes @ {} sat/kvB)", 
                 estimated_fee, estimated_discount_vsize, fee_rate);

        // Calculate change amount to return to fee service
        if fee_utxo_value <= estimated_fee {
            return Err(format!("Fee UTXO value ({} sats) is too small to cover estimated fees ({} sats)", 
                fee_utxo_value, estimated_fee).into());
        }
        let lbtc_change_amount = fee_utxo_value - estimated_fee;
        println!("LBTC change to fee service: {} sats", lbtc_change_amount);
        
        // Create transaction: user sends asset, fee service provides LBTC UTXO for fees
        let mut pset = user_wollet
            .tx_builder()
            .add_recipient(&recipient, amount, asset_id)?
            // Add explicit LBTC change recipient for fee service
            .add_lbtc_recipient(&fee_service_address, lbtc_change_amount)?
            // Add Fee Service UTXO for fees
            .add_external_utxos(vec![external_fee_utxo])?
            .finish()?;
            
        // Add fee service details for signing
        self.wollet.add_details(&mut pset)?;
        
        // Fee service signs its LBTC input
        let fee_sigs = self.signer.sign(&mut pset)?;
        let user_sigs = user_signer.sign(&mut pset)?;
        println!("Fee service signed {} inputs, User signed {} inputs", fee_sigs, user_sigs);
        
        // Finalize and broadcast
        let tx = user_wollet.finalize(&mut pset)?;
        let txid = self.electrum_client.broadcast(&tx)?;
        
        println!("Transaction sent successfully!");
        println!("TXID: {}", txid);
        
        Ok(txid)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Fee Service Example");
    println!("This demonstrates how a fee service allows users to send assets without having LBTC in his wallet to pay the fees.\n");
    
    // Example wallets 
    let dummy_mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    let dummy_descriptor = "ct(slip77(ab5824f4477b4ebb00a132adfd8eb0b7935cf24f6ac151add5d1913db374ce92),elwpkh([759db348/84'/1'/0']tpubDCRMaF33e44pcJj534LXVhFbHibPbJ5vuLhSSPFAw57kYURv4tzXFL6LSnd78bkjqdmE3USedkbpXJUPA1tdzKfuYSL7PianceqAhwL2UkA/<0;1>/*))#cch6wrnp";

    // Fee service wallet needs to have LBTC to pay the fees.
    let fee_service_mnemonic = dummy_mnemonic; // Replace with your own for testing
    let fee_service_descriptor = dummy_descriptor; // Replace with your own for testing

    // User wallet needs to have the asset to send.
    let user_mnemonic = dummy_mnemonic; // Replace with your own for testing
    let user_descriptor = dummy_descriptor; // Replace with your own for testing

    // Transaction parameters
    // You will need an issued asset to send. You can issue one with lwk_cli for testnet. Or use USDT for mainnet.
    let dummy_asset_id = "0000000000000000000000000000000000000000000000000000000000000000";
    let asset_id = dummy_asset_id; // Replace with your own asset ID
    let amount = 10;
    let recipient_address = "tlq1qqtwyzvawpx8lz2ghhkufzc5d79j3rvgse9hkt9l53j3dfs86jek7kc45ksdvzfgrt95hfag5sypkw72p3gzq2v7k5mt7ug8n6";
    let electrum_url = "ssl://elements-testnet.blockstream.info:50002";
    let network = ElementsNetwork::LiquidTestnet;

    // Validate that dummy values have been replaced
    if fee_service_mnemonic == dummy_mnemonic || user_mnemonic == dummy_mnemonic {
        println!("❌ ERROR: You must replace the dummy wallet values with real ones!\n");
        println!("To run this example, you need to:");
        println!("1. Create two wallets using lwk_cli:");
        println!("   - Fee service wallet: Must have LBTC to pay transaction fees");
        println!("   - User wallet: Must have the asset you want to send\n");
        println!("2. Generate wallets:");
        println!("   lwk_cli signer generate");
        println!("   lwk_cli wallet load --wallet <name> -d <descriptor>\n");
        println!("3. Fund the fee service wallet with LBTC");
        println!("4. Issue an asset to the user wallet (or send existing assets):");
        println!("   lwk_cli wallet issue --wallet <name> --satoshi-asset <amount> --satoshi-token 0\n");
        println!("5. Replace the dummy values in this file:");
        println!("   - fee_service_mnemonic");
        println!("   - fee_service_descriptor");
        println!("   - user_mnemonic");
        println!("   - user_descriptor");
        println!("   - asset_id (with the issued asset ID)");
        return Ok(());
    }

    if asset_id == dummy_asset_id {
        println!("❌ ERROR: You must replace the dummy asset_id with a real one!\n");
        println!("To get an asset ID:");
        println!("1. Issue an asset using lwk_cli:");
        println!("   lwk_cli wallet issue --wallet <user-wallet> --satoshi-asset 1000 --satoshi-token 0\n");
        println!("2. Or use an existing asset like USDT on mainnet");
        println!("3. Replace the asset_id variable with your asset ID");
        return Ok(());
    }

    // Create and use fee service
    let mut fee_service = FeeService::new(fee_service_mnemonic, fee_service_descriptor, electrum_url, network)?;
    
    println!("Fee service address: {}", fee_service.get_address()?);
    println!("\n=== Sending Asset with Fee Service ===");
    
    match fee_service.send_asset_with_fee_service(
        user_mnemonic,
        user_descriptor,
        asset_id,
        amount,
        recipient_address,
    ) {
        Ok(txid) => println!("✅ Success! TXID: {}", txid),
        Err(e) => println!("❌ Error: {}", e),
    }

    Ok(())
}
