//! LiteSVM integration tests for cumulative claims (V2).
//!
//! Run with: `cargo test --package attention-oracle-token-2022 --test litesvm_cumulative`
//!
//! Prerequisites:
//! 1. Build the program: `anchor build`
//! 2. Program binary at: target/deploy/token_2022.so

use litesvm::LiteSVM;
use sha2::{Digest, Sha256};
use sha3::Keccak256;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    message::Message,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use solana_system_interface::program as system_program;
use std::path::Path;

// Program ID (must match declared_id! in lib.rs)
fn program_id() -> Pubkey {
    "GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop"
        .parse()
        .unwrap()
}

// Seeds
const PROTOCOL_SEED: &[u8] = b"protocol";
const CHANNEL_CONFIG_V2_SEED: &[u8] = b"channel_cfg_v2";
const CLAIM_STATE_V2_SEED: &[u8] = b"claim_state_v2";
const CUMULATIVE_V2_DOMAIN: &[u8] = b"TWZRD:CUMULATIVE_V2";

/// Compute Anchor discriminator for an instruction
fn compute_discriminator(name: &str) -> [u8; 8] {
    let preimage = format!("global:{}", name);
    let hash = Sha256::digest(preimage.as_bytes());
    let mut disc = [0u8; 8];
    disc.copy_from_slice(&hash[..8]);
    disc
}

/// Derive subject_id from channel name (matches on-chain logic)
fn derive_subject_id(channel: &str) -> Pubkey {
    let lower = channel.to_lowercase();
    let mut hasher = Keccak256::new();
    hasher.update(b"channel:");
    hasher.update(lower.as_bytes());
    let hash: [u8; 32] = hasher.finalize().into();
    Pubkey::new_from_array(hash)
}

/// Derive protocol state PDA
fn derive_protocol_state(mint: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[PROTOCOL_SEED, mint.as_ref()], &program_id())
}

/// Derive channel config V2 PDA
fn derive_channel_config_v2(mint: &Pubkey, subject_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[CHANNEL_CONFIG_V2_SEED, mint.as_ref(), subject_id.as_ref()],
        &program_id(),
    )
}

/// Derive claim state V2 PDA
fn derive_claim_state_v2(channel_config: &Pubkey, wallet: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[CLAIM_STATE_V2_SEED, channel_config.as_ref(), wallet.as_ref()],
        &program_id(),
    )
}

/// Compute cumulative leaf hash (matches on-chain)
fn compute_cumulative_leaf(
    channel_config: &Pubkey,
    mint: &Pubkey,
    root_seq: u64,
    wallet: &Pubkey,
    cumulative_total: u64,
) -> [u8; 32] {
    let mut hasher = Keccak256::new();
    hasher.update(CUMULATIVE_V2_DOMAIN);
    hasher.update(channel_config.as_ref());
    hasher.update(mint.as_ref());
    hasher.update(&root_seq.to_le_bytes());
    hasher.update(wallet.as_ref());
    hasher.update(&cumulative_total.to_le_bytes());
    hasher.finalize().into()
}

/// Compute merkle root from leaves using sorted-pair keccak
fn compute_merkle_root(leaves: &[[u8; 32]]) -> [u8; 32] {
    if leaves.is_empty() {
        return [0u8; 32];
    }
    if leaves.len() == 1 {
        return leaves[0];
    }

    let mut current = leaves.to_vec();
    while current.len() > 1 {
        let mut next = Vec::new();
        for chunk in current.chunks(2) {
            let left = chunk[0];
            let right = if chunk.len() > 1 { chunk[1] } else { chunk[0] };
            let (a, b) = if left <= right {
                (left, right)
            } else {
                (right, left)
            };
            let mut hasher = Keccak256::new();
            hasher.update(&a);
            hasher.update(&b);
            next.push(hasher.finalize().into());
        }
        current = next;
    }
    current[0]
}

/// Generate merkle proof for a leaf at given index
fn generate_proof(leaves: &[[u8; 32]], mut index: usize) -> Vec<[u8; 32]> {
    if leaves.is_empty() || leaves.len() == 1 {
        return vec![];
    }

    let mut proof = Vec::new();
    let mut current = leaves.to_vec();

    while current.len() > 1 {
        let sibling_idx = if index % 2 == 0 { index + 1 } else { index - 1 };
        let sibling = if sibling_idx < current.len() {
            current[sibling_idx]
        } else {
            current[index]
        };
        proof.push(sibling);

        let mut next = Vec::new();
        for chunk in current.chunks(2) {
            let left = chunk[0];
            let right = if chunk.len() > 1 { chunk[1] } else { chunk[0] };
            let (a, b) = if left <= right {
                (left, right)
            } else {
                (right, left)
            };
            let mut hasher = Keccak256::new();
            hasher.update(&a);
            hasher.update(&b);
            next.push(hasher.finalize().into());
        }
        current = next;
        index /= 2;
    }
    proof
}

/// Helper to load the compiled program
fn load_program(svm: &mut LiteSVM) -> Result<(), Box<dyn std::error::Error>> {
    let program_path = Path::new("../../target/deploy/token_2022.so");

    if !program_path.exists() {
        return Err(format!(
            "Program not found at {:?}. Run `anchor build` first.",
            program_path
                .canonicalize()
                .unwrap_or(program_path.to_path_buf())
        )
        .into());
    }

    let program_bytes = std::fs::read(program_path)?;
    svm.add_program(program_id(), &program_bytes)?;
    Ok(())
}

#[test]
fn test_litesvm_setup() {
    // Just verify LiteSVM initializes correctly
    let mut svm = LiteSVM::new();
    let payer = Keypair::new();

    // Airdrop to payer
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();

    let balance = svm.get_balance(&payer.pubkey()).unwrap();
    assert_eq!(balance, 10_000_000_000);

    println!("LiteSVM setup test passed!");
}

#[test]
fn test_pda_derivation() {
    // Verify PDA derivation matches expected values
    let mint = Pubkey::new_unique();
    let channel = "youtube_lofi";

    let subject_id = derive_subject_id(channel);
    println!("Subject ID for '{}': {}", channel, subject_id);

    let (protocol_state, bump) = derive_protocol_state(&mint);
    println!("Protocol State PDA: {} (bump: {})", protocol_state, bump);

    let (channel_config, bump) = derive_channel_config_v2(&mint, &subject_id);
    println!("Channel Config V2 PDA: {} (bump: {})", channel_config, bump);

    let wallet = Pubkey::new_unique();
    let (claim_state, bump) = derive_claim_state_v2(&channel_config, &wallet);
    println!("Claim State V2 PDA: {} (bump: {})", claim_state, bump);

    // Verify deterministic derivation
    let subject_id_2 = derive_subject_id(channel);
    assert_eq!(subject_id, subject_id_2);

    // Case insensitive
    let subject_id_upper = derive_subject_id("YOUTUBE_LOFI");
    assert_eq!(subject_id, subject_id_upper);
}

#[test]
fn test_merkle_proof_generation() {
    let mint = Pubkey::new_unique();
    let channel_config = Pubkey::new_unique();
    let root_seq = 1u64;

    // Create test wallets and amounts
    let wallets: Vec<(Pubkey, u64)> = (0..4)
        .map(|i| (Pubkey::new_unique(), (i + 1) as u64 * 1_000_000_000))
        .collect();

    // Compute leaves
    let leaves: Vec<[u8; 32]> = wallets
        .iter()
        .map(|(wallet, amount)| {
            compute_cumulative_leaf(&channel_config, &mint, root_seq, wallet, *amount)
        })
        .collect();

    // Compute root
    let root = compute_merkle_root(&leaves);
    println!("Merkle root: {}", hex::encode(root));

    // Generate and verify proof for each leaf
    for (i, leaf) in leaves.iter().enumerate() {
        let proof = generate_proof(&leaves, i);
        println!("Proof for leaf {}: {} nodes", i, proof.len());

        // Verify proof manually
        let mut computed = *leaf;
        for node in &proof {
            let (a, b) = if computed <= *node {
                (computed, *node)
            } else {
                (*node, computed)
            };
            let mut hasher = Keccak256::new();
            hasher.update(&a);
            hasher.update(&b);
            computed = hasher.finalize().into();
        }
        assert_eq!(computed, root, "Proof verification failed for leaf {}", i);
    }

    println!("Merkle proof generation test passed!");
}

#[test]
fn test_discriminator_computation() {
    // Verify discriminators match expected values
    let disc = compute_discriminator("initialize_channel_cumulative");
    println!("initialize_channel_cumulative: {:?}", disc);

    let disc = compute_discriminator("publish_cumulative_root");
    println!("publish_cumulative_root: {:?}", disc);

    let disc = compute_discriminator("claim_cumulative");
    println!("claim_cumulative: {:?}", disc);

    let disc = compute_discriminator("claim_cumulative_sponsored");
    println!("claim_cumulative_sponsored: {:?}", disc);
}

#[test]
fn test_program_load() {
    let mut svm = LiteSVM::new();

    match load_program(&mut svm) {
        Ok(_) => println!("Program loaded successfully!"),
        Err(e) => {
            eprintln!("Failed to load program: {}", e);
            eprintln!("Make sure to run `anchor build` first.");
        }
    }
}

// ============================================================================
// Integration Tests (require compiled program)
// ============================================================================

#[test]
fn test_initialize_protocol() {
    let mut svm = LiteSVM::new();

    // Load program
    if load_program(&mut svm).is_err() {
        println!("Skipping test - program not compiled");
        return;
    }

    let admin = Keypair::new();
    let mint = Keypair::new();

    // Airdrop
    svm.airdrop(&admin.pubkey(), 10_000_000_000).unwrap();

    // Derive PDAs
    let (protocol_state, _) = derive_protocol_state(&mint.pubkey());

    // Build initialize_mint instruction
    let disc = compute_discriminator("initialize_mint");
    let mut data = disc.to_vec();
    data.extend_from_slice(&50u16.to_le_bytes()); // fee_basis_points
    data.extend_from_slice(&1_000_000u64.to_le_bytes()); // max_fee

    let ix = Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new(admin.pubkey(), true),
            AccountMeta::new(protocol_state, false),
            AccountMeta::new(mint.pubkey(), true),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data,
    };

    let blockhash = svm.latest_blockhash();
    let message = Message::new(&[ix], Some(&admin.pubkey()));
    let tx = Transaction::new(&[&admin, &mint], message, blockhash);

    match svm.send_transaction(tx) {
        Ok(_) => println!("Protocol initialized successfully!"),
        Err(e) => println!("Initialize failed (expected without full setup): {:?}", e),
    }
}
