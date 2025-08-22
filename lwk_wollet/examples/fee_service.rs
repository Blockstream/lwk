extern crate lwk_wollet;

use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::Path;
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

    /// Convert user's UTXOs to external format for use by other wallets
    fn convert_user_utxos_to_external(
        &self,
        user_utxos: &[lwk_wollet::WalletTxOut],
        user_wollet: &Wollet,
        required_amount: u64,
    ) -> Result<(Vec<ExternalUtxo>, u64), Box<dyn std::error::Error>> {
        let mut external_utxos = vec![];
        let mut selected_value = 0u64;
        
        for utxo in user_utxos {
            // Get the full transaction
            let transactions = self.electrum_client.get_transactions(&[utxo.outpoint.txid])?;
            let tx = transactions.into_iter().next()
                .ok_or("Transaction not found")?;
            
            // Extract the specific output
            let txout = tx.output.get(utxo.outpoint.vout as usize)
                .ok_or("Invalid output index")?
                .clone();
            
            // For non-segwit descriptors, include full transaction
            let full_tx = if user_wollet.is_segwit() {
                None
            } else {
                Some(tx)
            };
            
            let external_utxo = ExternalUtxo {
                outpoint: utxo.outpoint,
                txout,
                tx: full_tx,
                unblinded: utxo.unblinded.clone(),
                max_weight_to_satisfy: user_wollet.max_weight_to_satisfy(),
            };
            
            external_utxos.push(external_utxo);
            selected_value += utxo.unblinded.value;
            
            // Stop when we have enough
            if selected_value >= required_amount {
                break;
            }
        }
        
        Ok((external_utxos, selected_value))
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
        
        // Check fee service asset balance
        let fee_balance = self.wollet.balance()?;
        let fee_asset_balance = fee_balance.get(&asset_id).copied().unwrap_or(0);
        println!("Fee service asset balance: {} units", fee_asset_balance);
        
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

    /// Process a transaction on the server side
    /// This function emulates what the fee service server would do:
    /// - Receives user's asset UTXOs and transaction details
    /// - Constructs the PSET with server's LBTC UTXO for fees
    /// - Signs the server's inputs
    /// - Returns the partially signed PSET
    fn process_server_wallet(
        &mut self,
        user_asset_utxos: Vec<ExternalUtxo>,
        recipient: &Address,
        amount: u64,
        asset_id: AssetId,
        fee_in_asset: u64,
        user_change_address: &Address,
    ) -> Result<elements::pset::PartiallySignedTransaction, Box<dyn std::error::Error>> {
        println!("\n=== Server Side Processing ===");
        
        // Check fee service has LBTC for fees
        let lbtc = self.wollet.policy_asset();
        let fee_service_balance = self.wollet.balance().unwrap_or_default().get(&lbtc).copied().unwrap_or(0);
        println!("Server LBTC balance: {} sats", fee_service_balance);
        
        if fee_service_balance == 0 {
            return Err("Server has no LBTC to pay transaction fees".into());
        }
        
        // Get fee service LBTC UTXO
        let fee_utxo = self.wollet
            .utxos()?
            .into_iter()
            .find(|u| u.unblinded.asset == lbtc)
            .ok_or("Server has no LBTC UTXO available")?;
        
        println!("Server UTXO value: {} sats", fee_utxo.unblinded.value);
        
        // Get fee service address for receiving fee payment and LBTC change
        let fee_service_address = self.get_address()?;
        
        // Calculate total asset input value from user's UTXOs
        let total_asset_input: u64 = user_asset_utxos.iter()
            .map(|utxo| utxo.unblinded.value)
            .sum();
        
        // Calculate asset change
        let asset_change = total_asset_input - amount - fee_in_asset;
        println!("Asset input total: {} units", total_asset_input);
        println!("Asset change to user: {} units", asset_change);
        
        // Build the transaction from server's perspective
        // The server only manages its own LBTC UTXO
        let mut tx_builder = self.wollet
            .tx_builder()
            // Send asset to recipient
            .add_recipient(&recipient, amount, asset_id)?;
        
        // Add asset change back to user if any
        if asset_change > 0 {
            tx_builder = tx_builder.add_recipient(&user_change_address, asset_change, asset_id)?;
        }
        
        // Only add fee payment if fee_in_asset > 0
        if fee_in_asset > 0 {
            tx_builder = tx_builder.add_recipient(&fee_service_address, fee_in_asset, asset_id)?;
        }
        
        let mut pset = tx_builder
            // Add user's asset UTXOs as external inputs (server won't select them as wallet utxos)
            .add_external_utxos(user_asset_utxos)?
            // Only select the server's LBTC UTXO for fee payment
            .set_wallet_utxos(vec![fee_utxo.outpoint])
            // Drain all LBTC back to fee service
            .drain_lbtc_wallet()
            .drain_lbtc_to(fee_service_address.clone())
            .finish()?;
        
        // Add details from wallet for signing
        self.wollet.add_details(&mut pset)?;
        let fee_sigs = self.signer.sign(&mut pset)?;
        println!("Server signed {} inputs", fee_sigs);
        
        Ok(pset)
    }

    /// Send an asset from user wallet using fee service to pay transaction fees
    /// This split approach separates app and server responsibilities:
    /// - App side: Prepares user wallet and asset UTXOs
    /// - Server side: Constructs PSET and signs its inputs
    /// - App side: Signs user inputs and broadcasts
    pub fn send_emulating_app_plus_server(
        &mut self,
        user_mnemonic: &str,
        user_descriptor: &str,
        asset_id: &str,
        amount: u64,
        recipient_address: &str,
        fee_in_asset: u64,
    ) -> Result<Txid, Box<dyn std::error::Error>> {
        println!("=== Fee Service Transaction (App + Server Split) ===");
        println!("This example shows the separation between:");
        println!("- App side: User wallet management and signing");
        println!("- Server side: Fee service PSET construction and signing\n");
        
        // Parse inputs
        let asset_id: AssetId = asset_id.parse()?;
        let recipient: Address = recipient_address.parse()?;
        
        println!("Transaction details:");
        println!("- Asset ID: {}", asset_id);
        println!("- Amount to recipient: {} units", amount);
        println!("- Fee to service: {} units", fee_in_asset);
        println!("- Recipient: {}", recipient_address);
        
        // ========== APP SIDE: Step 1 - Initialize user wallet ==========
        println!("\n=== App Side: Step 1 - Initialize User Wallet ===");
        let user_signer = SwSigner::new(user_mnemonic, false)?;
        let user_descriptor = user_descriptor.parse()?;
        let mut user_wollet = Wollet::new(self.network, NoPersist::new(), user_descriptor)?;
        
        // Sync user wallet
        let update = self.electrum_client.full_scan(&user_wollet)?;
        if let Some(update) = update {
            user_wollet.apply_update(update)?;
        }
        println!("User wallet synced");
        
        // ========== APP SIDE: Step 2 - Get asset UTXOs ==========
        println!("\n=== App Side: Step 2 - Get Asset UTXOs ===");
        let user_balance = user_wollet.balance()?;
        let user_asset_balance = user_balance.get(&asset_id).copied().unwrap_or(0);
        println!("User asset balance: {} units", user_asset_balance);
        
        let total_asset_needed = amount + fee_in_asset;
        if user_asset_balance < total_asset_needed {
            return Err(format!(
                "Insufficient asset balance. Available: {} units, Required: {} units (amount: {} + fee: {})",
                user_asset_balance, total_asset_needed, amount, fee_in_asset
            ).into());
        }
        
        // Get user's asset UTXOs
        let user_utxos = user_wollet
            .utxos()?
            .into_iter()
            .filter(|u| u.unblinded.asset == asset_id)
            .collect::<Vec<_>>();
        
        // Convert to external UTXOs
        let (external_user_utxos, selected_value) = self.convert_user_utxos_to_external(&user_utxos, &user_wollet, total_asset_needed)?;
        
        println!("Selected {} asset UTXOs with total value: {} units", 
                 external_user_utxos.len(), selected_value);
        
        // Get user change address
        let user_change_address = user_wollet.address(None)?;
        let user_change_addr = user_change_address.address().clone();
        
        // ========== APP SIDE: Step 3 - Send details to server ==========
        println!("\n=== App Side: Step 3 - Send Details to Server ===");
        println!("Sending to server:");
        println!("- User's asset UTXOs");
        println!("- Recipient address: {}", recipient_address);
        println!("- Amount: {} units", amount);
        println!("- Fee in asset: {} units", fee_in_asset);
        println!("- User change address: {}", user_change_addr);
        
        // Sync server wallet before processing
        let update = self.electrum_client.full_scan(&self.wollet)?;
        if let Some(update) = update {
            self.wollet.apply_update(update)?;
        }
        
        // ========== SERVER SIDE: Step 4 - Process and create PSET ==========
        let mut pset = self.process_server_wallet(
            external_user_utxos,
            &recipient,
            amount,
            asset_id,
            fee_in_asset,
            &user_change_addr,
        )?;
        
        // ========== APP SIDE: Step 5 - Sign and broadcast ==========
        println!("\n=== App Side: Step 5 - Sign and Broadcast ===");
        println!("Received partially signed PSET from server");
        
        // Add user wallet details and sign
        user_wollet.add_details(&mut pset)?;
        let user_sigs = user_signer.sign(&mut pset)?;
        println!("App signed {} inputs", user_sigs);
        
        // Finalize and broadcast
        let tx = user_wollet.finalize(&mut pset)?;
        let txid = self.electrum_client.broadcast(&tx)?;
        
        println!("\n✅ Transaction broadcast successfully!");
        println!("TXID: {}", txid);
        
        Ok(txid)
    }

    /// Send an asset from user wallet using fee service to pay transaction fees
    /// This unified approach creates a single PSET where:
    /// - User sends asset to recipient
    /// - User pays fee service with some asset units
    /// - Fee service provides LBTC for transaction fees
    pub fn send_asset_with_fee_service_unified(
        &mut self,
        user_mnemonic: &str,
        user_descriptor: &str,
        asset_id: &str,
        amount: u64,
        recipient_address: &str,
        fee_in_asset: u64,  // How much asset to pay the fee service
    ) -> Result<Txid, Box<dyn std::error::Error>> {
        // Parse inputs
        let asset_id: AssetId = asset_id.parse()?;
        let recipient: Address = recipient_address.parse()?;
        
        println!("=== Fee Service Transaction (Unified PSET) ===");
        println!("Asset ID: {}", asset_id);
        println!("Amount to recipient: {} units", amount);
        println!("Fee to service: {} units", fee_in_asset);
        println!("Recipient: {}", recipient_address);
        
        // Create user wallet
        let user_signer = SwSigner::new(user_mnemonic, false)?;
        let user_descriptor = user_descriptor.parse()?;
        let mut user_wollet = Wollet::new(self.network, NoPersist::new(), user_descriptor)?;
        
        // Sync both wallets
        let update = self.electrum_client.full_scan(&self.wollet)?;
        if let Some(update) = update {
            self.wollet.apply_update(update)?;
        }
        
        let update = self.electrum_client.full_scan(&user_wollet)?;
        if let Some(update) = update {
            user_wollet.apply_update(update)?;
        }
        
        // Check user has enough asset
        let user_balance = user_wollet.balance()?;
        let user_asset_balance = user_balance.get(&asset_id).copied().unwrap_or(0);
        println!("User asset balance: {} units", user_asset_balance);
        
        let total_asset_needed = amount + fee_in_asset;
        if user_asset_balance < total_asset_needed {
            return Err(format!(
                "Insufficient asset balance. Available: {} units, Required: {} units (amount: {} + fee: {})",
                user_asset_balance, total_asset_needed, amount, fee_in_asset
            ).into());
        }
        
        // Check fee service has LBTC for fees
        let lbtc = self.wollet.policy_asset();
        let fee_service_balance = self.wollet.balance().unwrap_or_default().get(&lbtc).copied().unwrap_or(0);
        println!("Fee service LBTC balance: {} sats", fee_service_balance);
        
        if fee_service_balance == 0 {
            return Err("Fee service has no LBTC to pay transaction fees".into());
        }
        
        // Get fee service LBTC UTXO
        let fee_utxo = self.wollet
            .utxos()?
            .into_iter()
            .find(|u| u.unblinded.asset == lbtc)
            .ok_or("Fee service has no LBTC UTXO available")?;
        
        println!("Fee service UTXO value: {} sats", fee_utxo.unblinded.value);
        
        // Convert to external UTXO format
        let external_fee_utxo = self.make_external_utxo(&fee_utxo)?;
        
        // Get fee service addresses
        let fee_service_address = self.get_address()?;
        
        // Get user's asset UTXOs
        let user_utxos = user_wollet
            .utxos()?
            .into_iter()
            .filter(|u| u.unblinded.asset == asset_id)
            .collect::<Vec<_>>();
        
        // Select asset UTXOs
        let mut selected_value = 0u64;
        let mut user_asset_utxos = vec![];
        for utxo in &user_utxos {
            user_asset_utxos.push(utxo.outpoint);
            selected_value += utxo.unblinded.value;
            if selected_value >= total_asset_needed {
                break;
            }
        }
        
        println!("Selected {} asset UTXOs with total value: {} units", 
                 user_asset_utxos.len(), selected_value);
        
        // Create unified transaction from user wallet
        let mut tx_builder = user_wollet
            .tx_builder()
            // Send asset to recipient
            .add_recipient(&recipient, amount, asset_id)?;
        
        // Only add fee payment if fee_in_asset > 0
        if fee_in_asset > 0 {
            tx_builder = tx_builder.add_recipient(&fee_service_address, fee_in_asset, asset_id)?;
        }
        
        let mut pset = tx_builder
            // Select user's asset UTXOs
            .set_wallet_utxos(user_asset_utxos)
            // Add fee service's LBTC UTXO
            .add_external_utxos(vec![external_fee_utxo])?
            // Drain all LBTC back to fee service
            .drain_lbtc_wallet()
            .drain_lbtc_to(fee_service_address.clone())
            .finish()?;
        
        // Add details from both wallets for signing
        self.wollet.add_details(&mut pset)?;
        let fee_sigs = self.signer.sign(&mut pset)?;
        user_wollet.add_details(&mut pset)?;
        
        // Both parties sign their respective inputs
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

/// Load environment variables from keys.env file
fn load_env_file(file_path: &str) -> Result<HashMap<String, String>, Box<dyn std::error::Error>> {
    let mut env_vars = HashMap::new();
    
    if !Path::new(file_path).exists() {
        return Err(format!("Environment file {} not found", file_path).into());
    }
    
    let contents = fs::read_to_string(file_path)?;
    
    for line in contents.lines() {
        let line = line.trim();
        
        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        
        // Split on first '=' to handle values that might contain '='
        if let Some(eq_pos) = line.find('=') {
            let key = line[..eq_pos].trim().to_string();
            let value = line[eq_pos + 1..].trim();
            
            // Remove quotes if present
            let value = value.trim_matches('"').trim_matches('\'').to_string();
            
            env_vars.insert(key, value);
        }
    }
    
    Ok(env_vars)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Fee Service Example");
    println!("This demonstrates how a fee service allows users to send assets without having LBTC in his wallet to pay the fees.");
    
    // Parse command line arguments
    let args: Vec<String> = env::args().collect();
    let use_unified = args.contains(&"unified".to_string());
    let use_split_server = args.contains(&"splited_server".to_string());
    
    // Parse fee amount if provided after "unified"
    let mut fee_from_args: Option<u64> = None;
    if let Some(pos) = args.iter().position(|arg| arg == "unified") {
        if let Some(fee_str) = args.get(pos + 1) {
            fee_from_args = fee_str.parse().ok();
        }
    }
    
    // Parse fee amount if provided after "splited_server"
    let mut split_fee_from_args: Option<u64> = None;
    if let Some(pos) = args.iter().position(|arg| arg == "splited_server") {
        if let Some(fee_str) = args.get(pos + 1) {
            split_fee_from_args = fee_str.parse().ok();
        }
    }
    
    if args.contains(&"help".to_string()) || args.contains(&"-h".to_string()) {
        println!("\nUsage: cargo run -p lwk_wollet --features electrum --example fee_service [OPTIONS]");
        println!("\nOptions:");
        println!("  -- unified [FEE]         Use unified PSET approach (with optional fee payment)");
        println!("                           FEE: Amount of asset to pay as fee (default: from keys.env or 2)");
        println!("                           If FEE is 0, no fee payment to service is included");
        println!("  -- splited_server [FEE]  Use split app/server approach (BEST for learning)");
        println!("                           FEE: Amount of asset to pay as fee (default: from keys.env or 0)");
        println!("                 Default: Basic fee service approach with manual fee calculation");
        println!("\nExamples:");
        println!("  cargo run -p lwk_wollet --features electrum --example fee_service");
        println!("  cargo run -p lwk_wollet --features electrum --example fee_service -- unified");
        println!("  cargo run -p lwk_wollet --features electrum --example fee_service -- unified 0");
        println!("  cargo run -p lwk_wollet --features electrum --example fee_service -- unified 5");
        println!("  cargo run -p lwk_wollet --features electrum --example fee_service -- splited_server");
        println!("  cargo run -p lwk_wollet --features electrum --example fee_service -- splited_server 3");
        return Ok(());
    }
    
    if use_unified {
        println!("\nUsing unified PSET approach...");
    } else if use_split_server {
        println!("\nUsing split app/server approach...");
    } else {
        println!("\nUsing basic fee service approach...");
    }
    
    // Load environment variables from keys.env
    let env_file_path = "lwk_wollet/examples/keys.env";
    println!("Loading configuration from {}...", env_file_path);
    
    let env_vars = match load_env_file(env_file_path) {
        Ok(vars) => vars,
        Err(e) => {
            println!("❌ ERROR: Failed to load {}: {}", env_file_path, e);
            println!("\nPlease create a keys.env file with the following variables:");
            println!("fee_service_mnemonic=\"your fee service mnemonic here\"");
            println!("fee_service_descriptor=\"your fee service descriptor here\"");
            println!("user_mnemonic=\"your user mnemonic here\"");
            println!("user_descriptor=\"your user descriptor here\"");
            println!("asset_id=\"your asset id here\"");
            println!("amount=10");
            println!("recipient_address=\"recipient address here\"");
            println!("fee_in_asset=2  # Optional: for -- unified mode, how many asset units to pay as fee");
            return Ok(());
        }
    };
    
    // Example wallets 
    let dummy_mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    let _dummy_descriptor = "ct(slip77(ab5824f4477b4ebb00a132adfd8eb0b7935cf24f6ac151add5d1913db374ce92),elwpkh([759db348/84'/1'/0']tpubDCRMaF33e44pcJj534LXVhFbHibPbJ5vuLhSSPFAw57kYURv4tzXFL6LSnd78bkjqdmE3USedkbpXJUPA1tdzKfuYSL7PianceqAhwL2UkA/<0;1>/*))#cch6wrnp";

    // Fee service wallet needs to have LBTC to pay the fees.
    let fee_service_mnemonic = env_vars.get("fee_service_mnemonic")
        .ok_or("fee_service_mnemonic not found in keys.env")?;
    let fee_service_descriptor = env_vars.get("fee_service_descriptor")
        .ok_or("fee_service_descriptor not found in keys.env")?;

    // User wallet needs to have the asset to send.
    let user_mnemonic = env_vars.get("user_mnemonic")
        .ok_or("user_mnemonic not found in keys.env")?;
    let user_descriptor = env_vars.get("user_descriptor")
        .ok_or("user_descriptor not found in keys.env")?;

    // Transaction parameters
    let asset_id = env_vars.get("asset_id")
        .ok_or("asset_id not found in keys.env")?;
    let amount: u64 = env_vars.get("amount")
        .ok_or("amount not found in keys.env")?
        .parse()
        .map_err(|_| "Invalid amount value in keys.env")?;
    let recipient_address = env_vars.get("recipient_address")
        .ok_or("recipient_address not found in keys.env")?;
    
    // Default values that can be overridden
    let electrum_url = env_vars.get("electrum_url")
        .map(|s| s.as_str())
        .unwrap_or("ssl://elements-testnet.blockstream.info:50002");
    let network = ElementsNetwork::LiquidTestnet;

    // Validate that dummy values have been replaced
    if fee_service_mnemonic == &dummy_mnemonic || user_mnemonic == &dummy_mnemonic {
        println!("❌ ERROR: You must replace the dummy wallet values with real ones in keys.env!\n");
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
        println!("5. Update the values in keys.env:");
        println!("   - fee_service_mnemonic");
        println!("   - fee_service_descriptor");
        println!("   - user_mnemonic");
        println!("   - user_descriptor");
        println!("   - asset_id (with the issued asset ID)");
        return Ok(());
    }

    let dummy_asset_id = "0000000000000000000000000000000000000000000000000000000000000000";
    if asset_id == dummy_asset_id {
        println!("❌ ERROR: You must replace the dummy asset_id with a real one in keys.env!\n");
        println!("To get an asset ID:");
        println!("1. Issue an asset using lwk_cli:");
        println!("   lwk_cli wallet issue --wallet <user-wallet> --satoshi-asset 1000 --satoshi-token 0\n");
        println!("2. Or use an existing asset like USDT on mainnet");
        println!("3. Update the asset_id value in keys.env");
        return Ok(());
    }

    // Create and use fee service
    let mut fee_service = FeeService::new(fee_service_mnemonic, fee_service_descriptor, electrum_url, network)?;
    
    println!("Fee service address: {}", fee_service.get_address()?);
    println!("\n=== Sending Asset with Fee Service ===");
    
    let result = if use_unified {
        // Get fee_in_asset from command line args, then env vars, then default
        let fee_in_asset: u64 = fee_from_args
            .or_else(|| env_vars.get("fee_in_asset").and_then(|s| s.parse().ok()))
            .unwrap_or(0); // Default: 0 units of asset as fee
        
        println!("Asset fee for service: {} units", fee_in_asset);
        
        fee_service.send_asset_with_fee_service_unified(
            user_mnemonic,
            user_descriptor,
            asset_id,
            amount,
            recipient_address,
            fee_in_asset,
        )
    } else if use_split_server {
        // Get fee_in_asset from command line args, then env vars, then default
        let fee_in_asset: u64 = split_fee_from_args
            .or_else(|| env_vars.get("fee_in_asset").and_then(|s| s.parse().ok()))
            .unwrap_or(0); // Default: 0 units of asset as fee
        
        println!("Asset fee for service: {} units", fee_in_asset);
        
        fee_service.send_emulating_app_plus_server(
            user_mnemonic,
            user_descriptor,
            asset_id,
            amount,
            recipient_address,
            fee_in_asset,
        )
    } else {
        // Basic approach - no fee payment
        fee_service.send_asset_with_fee_service(
            user_mnemonic,
            user_descriptor,
            asset_id,
            amount,
            recipient_address,
        )
    };
    
    match result {
        Ok(txid) => println!("✅ Success! TXID: {}", txid),
        Err(e) => println!("❌ Error: {}", e),
    }

    Ok(())
}
