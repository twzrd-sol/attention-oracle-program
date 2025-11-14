use anchor_lang::{prelude::*, InstructionData};
use solana_program_test::*;
use solana_sdk::{
    signature::{Keypair, Signer},
    system_instruction,
    transaction::Transaction,
    instruction::{AccountMeta, Instruction},
};

mod helpers;
use helpers::*;

fn program_test() -> ProgramTest {
    ProgramTest::new(
        "token_2022",
        token_2022::id(),
        None,
    )
}

fn compute_leaf_index_amount_id(claimer: &Pubkey, index: u32, amount: u64, id: &str) -> [u8; 32] {
    use solana_program::keccak;
    let idx = index.to_le_bytes();
    let amt = amount.to_le_bytes();
    let id_bytes = id.as_bytes();
    keccak::hashv(&[claimer.as_ref(), &idx, &amt, id_bytes]).to_bytes()
}

#[tokio::test]
async fn test_channel_ring_buffer() {
    let pt = program_test();
    let mut ctx = pt.start_with_context().await;

    // Setup accounts
    let admin = Keypair::new();
    // Manual airdrop to avoid borrowing issues
    let ix = system_instruction::transfer(&ctx.payer.pubkey(), &admin.pubkey(), 10_000_000_000);
    let recent = ctx.banks_client.get_latest_blockhash().await.unwrap();
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&ctx.payer.pubkey()),
        &[&ctx.payer],
        recent,
    );
    ctx.banks_client.process_transaction(tx).await.unwrap();

    // Create a dummy mint (we won't actually use it for token operations)
    let mint_kp = Keypair::new();
    let mint_space = 82; // Standard mint account size
    let rent = ctx.banks_client.get_rent().await.unwrap().minimum_balance(mint_space);

    let create_mint = system_instruction::create_account(
        &admin.pubkey(),
        &mint_kp.pubkey(),
        rent,
        mint_space as u64,
        &anchor_lang::system_program::ID, // Just a dummy account, not a real mint
    );
    // Mint keypair must also sign
    let recent = ctx.banks_client.get_latest_blockhash().await.unwrap();
    let tx = Transaction::new_signed_with_payer(
        &[create_mint],
        Some(&admin.pubkey()),
        &[&admin, &mint_kp],
        recent,
    );
    ctx.banks_client.process_transaction(tx).await.unwrap();

    // Initialize protocol state
    let im_open = ix_initialize_mint_open(&admin.pubkey(), &mint_kp.pubkey());
    send_ixs(&mut ctx, &admin, vec![im_open]).await.unwrap();

    // Initialize channel
    let channel = "test_channel";
    let streamer = derive_streamer_key(channel);
    let ix_init_chan = ix_initialize_channel(&admin.pubkey(), &mint_kp.pubkey(), &streamer);
    send_ixs(&mut ctx, &admin, vec![ix_init_chan]).await.unwrap();

    let (chan_pda, _) = Pubkey::find_program_address(
        &[
            token_2022::constants::CHANNEL_STATE_SEED,
            mint_kp.pubkey().as_ref(),
            streamer.as_ref(),
        ],
        &token_2022::id(),
    );

    // Verify channel was created
    let chan_account = ctx.banks_client.get_account(chan_pda).await.unwrap();
    assert!(chan_account.is_some(), "Channel PDA should exist");

    // Publish 5 roots to test ring buffer
    let (protocol_pda, _) = Pubkey::find_program_address(
        &[token_2022::constants::PROTOCOL_SEED, mint_kp.pubkey().as_ref()],
        &token_2022::id(),
    );

    for i in 0..5 {
        let epoch = 1000 + (i * 100);
        let root = [i as u8; 32]; // Dummy root

        let ix_set = Instruction {
            program_id: token_2022::id(),
            accounts: vec![
                AccountMeta::new(admin.pubkey(), true),
                AccountMeta::new(protocol_pda, false),
                AccountMeta::new(chan_pda, false),
                AccountMeta::new_readonly(anchor_lang::system_program::ID, false),
            ],
            data: token_2022::instruction::SetMerkleRootRing {
                root,
                epoch,
                claim_count: 100,
                streamer_key: streamer,
            }
            .data(),
        };

        send_ixs(&mut ctx, &admin, vec![ix_set]).await.unwrap();
    }

    println!("✅ Ring buffer test passed - published 5 epochs successfully");
}

#[tokio::test]
async fn test_8192_boundary() {
    let pt = program_test();
    let mut ctx = pt.start_with_context().await;

    let admin = Keypair::new();
    // Manual airdrop to avoid borrowing issues
    let ix = system_instruction::transfer(&ctx.payer.pubkey(), &admin.pubkey(), 10_000_000_000);
    let recent = ctx.banks_client.get_latest_blockhash().await.unwrap();
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&ctx.payer.pubkey()),
        &[&ctx.payer],
        recent,
    );
    ctx.banks_client.process_transaction(tx).await.unwrap();

    // Create dummy mint
    let mint_kp = Keypair::new();
    let mint_space = 82;
    let rent = ctx.banks_client.get_rent().await.unwrap().minimum_balance(mint_space);
    let create_mint = system_instruction::create_account(
        &admin.pubkey(),
        &mint_kp.pubkey(),
        rent,
        mint_space as u64,
        &anchor_lang::system_program::ID,
    );
    send_ixs(&mut ctx, &admin, vec![create_mint]).await.unwrap();

    // Initialize protocol
    let im_open = ix_initialize_mint_open(&admin.pubkey(), &mint_kp.pubkey());
    send_ixs(&mut ctx, &admin, vec![im_open]).await.unwrap();

    // Initialize channel
    let channel = "boundary_test";
    let streamer = derive_streamer_key(channel);
    let ix_init_chan = ix_initialize_channel(&admin.pubkey(), &mint_kp.pubkey(), &streamer);
    send_ixs(&mut ctx, &admin, vec![ix_init_chan]).await.unwrap();

    let (protocol_pda, _) = Pubkey::find_program_address(
        &[token_2022::constants::PROTOCOL_SEED, mint_kp.pubkey().as_ref()],
        &token_2022::id(),
    );
    let (chan_pda, _) = Pubkey::find_program_address(
        &[
            token_2022::constants::CHANNEL_STATE_SEED,
            mint_kp.pubkey().as_ref(),
            streamer.as_ref(),
        ],
        &token_2022::id(),
    );

    // Test publishing with claim_count = 8192 (should succeed)
    let epoch = 1000;
    let root = [1u8; 32];

    let ix_set_8192 = Instruction {
        program_id: token_2022::id(),
        accounts: vec![
            AccountMeta::new(admin.pubkey(), true),
            AccountMeta::new(protocol_pda, false),
            AccountMeta::new(chan_pda, false),
            AccountMeta::new_readonly(anchor_lang::system_program::ID, false),
        ],
        data: token_2022::instruction::SetMerkleRootRing {
            root,
            epoch,
            claim_count: 8192, // Maximum allowed
            streamer_key: streamer,
        }
        .data(),
    };

    send_ixs(&mut ctx, &admin, vec![ix_set_8192]).await.unwrap();
    println!("✅ Successfully published with claim_count=8192");

    // Test publishing with claim_count = 8193 (should fail)
    let epoch2 = 2000;
    let ix_set_8193 = Instruction {
        program_id: token_2022::id(),
        accounts: vec![
            AccountMeta::new(admin.pubkey(), true),
            AccountMeta::new(protocol_pda, false),
            AccountMeta::new(chan_pda, false),
            AccountMeta::new_readonly(anchor_lang::system_program::ID, false),
        ],
        data: token_2022::instruction::SetMerkleRootRing {
            root,
            epoch: epoch2,
            claim_count: 8193, // Over the limit
            streamer_key: streamer,
        }
        .data(),
    };

    let recent = ctx.banks_client.get_latest_blockhash().await.unwrap();
    let tx = Transaction::new_signed_with_payer(
        &[ix_set_8193],
        Some(&admin.pubkey()),
        &[&admin],
        recent,
    );

    let result = ctx.banks_client.process_transaction(tx).await;
    assert!(result.is_err(), "claim_count=8193 should fail");
    println!("✅ Correctly rejected claim_count=8193");
}

#[tokio::test]
async fn test_monotonic_epochs() {
    let pt = program_test();
    let mut ctx = pt.start_with_context().await;

    let admin = Keypair::new();
    // Manual airdrop to avoid borrowing issues
    let ix = system_instruction::transfer(&ctx.payer.pubkey(), &admin.pubkey(), 10_000_000_000);
    let recent = ctx.banks_client.get_latest_blockhash().await.unwrap();
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&ctx.payer.pubkey()),
        &[&ctx.payer],
        recent,
    );
    ctx.banks_client.process_transaction(tx).await.unwrap();

    // Setup
    let mint_kp = Keypair::new();
    let mint_space = 82;
    let rent = ctx.banks_client.get_rent().await.unwrap().minimum_balance(mint_space);
    // Mint keypair must also sign
    let create_mint_ix = system_instruction::create_account(
        &admin.pubkey(),
        &mint_kp.pubkey(),
        rent,
        mint_space as u64,
        &anchor_lang::system_program::ID,
    );
    let recent = ctx.banks_client.get_latest_blockhash().await.unwrap();
    let tx = Transaction::new_signed_with_payer(
        &[create_mint_ix],
        Some(&admin.pubkey()),
        &[&admin, &mint_kp],
        recent,
    );
    ctx.banks_client.process_transaction(tx).await.unwrap();

    let im_open = ix_initialize_mint_open(&admin.pubkey(), &mint_kp.pubkey());
    send_ixs(&mut ctx, &admin, vec![im_open]).await.unwrap();

    let channel = "monotonic_test";
    let streamer = derive_streamer_key(channel);
    let ix_init_chan = ix_initialize_channel(&admin.pubkey(), &mint_kp.pubkey(), &streamer);
    send_ixs(&mut ctx, &admin, vec![ix_init_chan]).await.unwrap();

    let (protocol_pda, _) = Pubkey::find_program_address(
        &[token_2022::constants::PROTOCOL_SEED, mint_kp.pubkey().as_ref()],
        &token_2022::id(),
    );
    let (chan_pda, _) = Pubkey::find_program_address(
        &[token_2022::constants::CHANNEL_STATE_SEED, mint_kp.pubkey().as_ref(), streamer.as_ref()],
        &token_2022::id(),
    );

    // Publish epoch 1000
    let epoch1 = 1000;
    let root = [1u8; 32];
    let ix1 = Instruction {
        program_id: token_2022::id(),
        accounts: vec![
            AccountMeta::new(admin.pubkey(), true),
            AccountMeta::new(protocol_pda, false),
            AccountMeta::new(chan_pda, false),
            AccountMeta::new_readonly(anchor_lang::system_program::ID, false),
        ],
        data: token_2022::instruction::SetMerkleRootRing {
            root,
            epoch: epoch1,
            claim_count: 100,
            streamer_key: streamer,
        }.data(),
    };
    send_ixs(&mut ctx, &admin, vec![ix1]).await.unwrap();

    // Try to publish epoch 999 (should fail - not increasing)
    let ix2 = Instruction {
        program_id: token_2022::id(),
        accounts: vec![
            AccountMeta::new(admin.pubkey(), true),
            AccountMeta::new(protocol_pda, false),
            AccountMeta::new(chan_pda, false),
            AccountMeta::new_readonly(anchor_lang::system_program::ID, false),
        ],
        data: token_2022::instruction::SetMerkleRootRing {
            root,
            epoch: 999,
            claim_count: 100,
            streamer_key: streamer,
        }.data(),
    };

    let recent = ctx.banks_client.get_latest_blockhash().await.unwrap();
    let tx = Transaction::new_signed_with_payer(&[ix2], Some(&admin.pubkey()), &[&admin], recent);
    let result = ctx.banks_client.process_transaction(tx).await;
    assert!(result.is_err(), "Non-increasing epoch should fail");
    println!("✅ Monotonic epoch check passed");
}
