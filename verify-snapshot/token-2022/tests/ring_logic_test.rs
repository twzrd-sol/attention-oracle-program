use anchor_lang::{prelude::*, InstructionData};
use solana_program::instruction::InstructionError;
use solana_program_test::*;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    signature::{Keypair, Signer},
    system_instruction,
    transaction::{Transaction, TransactionError},
    transport::TransportError,
};
use spl_associated_token_account as spl_ata;
use spl_token_2022 as spl_t22;
use token_2022::{
    constants::{
        CCM_DECIMALS, CHANNEL_BITMAP_BYTES, CHANNEL_RING_SLOTS, CHANNEL_STATE_SEED, PROTOCOL_SEED,
    },
    errors::ProtocolError,
    instruction::{ClaimWithRing, SetMerkleRootRing},
    instructions::claim::compute_leaf,
    state::{ChannelSlot, ChannelState},
};

mod helpers;
use helpers::*;

const CHANNEL_ACCOUNT_HEADER_LEN: usize = 1 + 1 + 32 + 32 + 8;
const CHANNEL_ACCOUNT_DISCRIMINATOR_LEN: usize = 8;
const CHANNEL_SLOT_BITMAP_OFFSET: usize = 8 + 32 + 2;
const CHANNEL_SLOT_START_OFFSET: usize =
    CHANNEL_ACCOUNT_DISCRIMINATOR_LEN + CHANNEL_ACCOUNT_HEADER_LEN;

fn program_test() -> ProgramTest {
    ProgramTest::new("token_2022", token_2022::id(), None)
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
    let tx =
        Transaction::new_signed_with_payer(&[ix], Some(&ctx.payer.pubkey()), &[&ctx.payer], recent);
    ctx.banks_client.process_transaction(tx).await.unwrap();

    // Create a dummy mint (we won't actually use it for token operations)
    let mint_kp = Keypair::new();
    let mint_space = 82; // Standard mint account size
    let rent = ctx
        .banks_client
        .get_rent()
        .await
        .unwrap()
        .minimum_balance(mint_space);

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
    send_ixs(&mut ctx, &admin, vec![ix_init_chan])
        .await
        .unwrap();

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
        &[
            token_2022::constants::PROTOCOL_SEED,
            mint_kp.pubkey().as_ref(),
        ],
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
    let tx =
        Transaction::new_signed_with_payer(&[ix], Some(&ctx.payer.pubkey()), &[&ctx.payer], recent);
    ctx.banks_client.process_transaction(tx).await.unwrap();

    // Create dummy mint
    let mint_kp = Keypair::new();
    let mint_space = 82;
    let rent = ctx
        .banks_client
        .get_rent()
        .await
        .unwrap()
        .minimum_balance(mint_space);
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
    send_ixs(&mut ctx, &admin, vec![ix_init_chan])
        .await
        .unwrap();

    let (protocol_pda, _) = Pubkey::find_program_address(
        &[
            token_2022::constants::PROTOCOL_SEED,
            mint_kp.pubkey().as_ref(),
        ],
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
    let tx =
        Transaction::new_signed_with_payer(&[ix], Some(&ctx.payer.pubkey()), &[&ctx.payer], recent);
    ctx.banks_client.process_transaction(tx).await.unwrap();

    // Setup
    let mint_kp = Keypair::new();
    let mint_space = 82;
    let rent = ctx
        .banks_client
        .get_rent()
        .await
        .unwrap()
        .minimum_balance(mint_space);
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
    send_ixs(&mut ctx, &admin, vec![ix_init_chan])
        .await
        .unwrap();

    let (protocol_pda, _) = Pubkey::find_program_address(
        &[
            token_2022::constants::PROTOCOL_SEED,
            mint_kp.pubkey().as_ref(),
        ],
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
        }
        .data(),
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
        }
        .data(),
    };

    let recent = ctx.banks_client.get_latest_blockhash().await.unwrap();
    let tx = Transaction::new_signed_with_payer(&[ix2], Some(&admin.pubkey()), &[&admin], recent);
    let result = ctx.banks_client.process_transaction(tx).await;
    assert!(result.is_err(), "Non-increasing epoch should fail");
    println!("✅ Monotonic epoch check passed");
}

#[tokio::test]
async fn test_claim_with_ring_bitmap_rotation() {
    let mut ctx = program_test().start_with_context().await;

    let admin = Keypair::new();
    fund_account(&mut ctx, &admin, 8_000_000_000)
        .await
        .expect("fund admin");

    let mint = Keypair::new();
    create_mint_with_transfer_fee(
        &mut ctx,
        &admin,
        &mint,
        CCM_DECIMALS,
        &admin.pubkey(),
        &admin.pubkey(),
        0,
        0,
    )
    .await
    .expect("mint creation");

    send_ixs(
        &mut ctx,
        &admin,
        vec![ix_initialize_mint_open(&admin.pubkey(), &mint.pubkey())],
    )
    .await
    .expect("protocol init");

    let channel = "regression_ring";
    let streamer = derive_streamer_key(channel);
    send_ixs(
        &mut ctx,
        &admin,
        vec![ix_initialize_channel(
            &admin.pubkey(),
            &mint.pubkey(),
            &streamer,
        )],
    )
    .await
    .expect("channel init");

    let (protocol_pda, _) =
        Pubkey::find_program_address(&[PROTOCOL_SEED, mint.pubkey().as_ref()], &token_2022::id());
    let (channel_pda, _) = Pubkey::find_program_address(
        &[
            CHANNEL_STATE_SEED,
            mint.pubkey().as_ref(),
            streamer.as_ref(),
        ],
        &token_2022::id(),
    );

    let treasury_ata = create_ata(&mut ctx, &admin, &protocol_pda, &mint.pubkey())
        .await
        .expect("treasury ATA");
    mint_to(
        &mut ctx,
        &admin,
        &mint.pubkey(),
        &treasury_ata,
        &admin,
        2, // cover two claims
        CCM_DECIMALS,
    )
    .await
    .expect("mint to treasury");

    let claimer = Keypair::new();
    fund_account(&mut ctx, &claimer, 2_000_000_000)
        .await
        .expect("fund claimer");
    let claimer_ata = spl_ata::get_associated_token_address_with_program_id(
        &claimer.pubkey(),
        &mint.pubkey(),
        &spl_t22::id(),
    );

    let epoch_one = 1_000;
    let claim_amount = 1;
    let claim_index = 0u32;
    let id_one = String::from("ring-claim-1");
    let root_one = compute_leaf(&claimer.pubkey(), claim_index, claim_amount, &id_one);

    send_ixs(
        &mut ctx,
        &admin,
        vec![set_ring_root_ix(
            &admin.pubkey(),
            protocol_pda,
            channel_pda,
            epoch_one,
            1,
            streamer,
            root_one,
        )],
    )
    .await
    .expect("publish first epoch");

    assert!(
        !is_slot_claimed(&mut ctx, channel_pda, epoch_one, claim_index as usize).await,
        "bitmap starts empty"
    );

    let claim_one_ix = claim_with_ring_ix(
        &claimer.pubkey(),
        protocol_pda,
        channel_pda,
        mint.pubkey(),
        treasury_ata,
        claimer_ata,
        spl_t22::id(),
        spl_ata::id(),
        epoch_one,
        claim_index,
        claim_amount,
        vec![],
        id_one.clone(),
        streamer,
    );

    send_ixs(&mut ctx, &claimer, vec![claim_one_ix])
        .await
        .expect("claim once");

    assert!(
        is_slot_claimed(&mut ctx, channel_pda, epoch_one, claim_index as usize).await,
        "bitmap flips on claim"
    );

    let claim_again_ix = claim_with_ring_ix(
        &claimer.pubkey(),
        protocol_pda,
        channel_pda,
        mint.pubkey(),
        treasury_ata,
        claimer_ata,
        spl_t22::id(),
        spl_ata::id(),
        epoch_one,
        claim_index,
        claim_amount,
        vec![],
        id_one.clone(),
        streamer,
    );

    let recent = ctx.banks_client.get_latest_blockhash().await.unwrap();
    let mut tx = Transaction::new_signed_with_payer(&[claim_again_ix], Some(&claimer.pubkey()));
    tx.sign(&[&claimer], recent);

    let err = ctx.banks_client.process_transaction(tx).await.unwrap_err();
    match err {
        TransportError::TransactionError(tx_err) => match tx_err {
            TransactionError::InstructionError(_, InstructionError::Custom(code)) => {
                assert_eq!(code, ProtocolError::AlreadyClaimed as u32);
            }
            other => panic!("unexpected double-claim failure: {:?}", other),
        },
        other => panic!("double claim transaction failed differently: {:?}", other),
    }

    let epoch_two = epoch_one + CHANNEL_RING_SLOTS as u64;
    let id_two = String::from("ring-claim-2");
    let root_two = compute_leaf(&claimer.pubkey(), claim_index, claim_amount, &id_two);

    send_ixs(
        &mut ctx,
        &admin,
        vec![set_ring_root_ix(
            &admin.pubkey(),
            protocol_pda,
            channel_pda,
            epoch_two,
            1,
            streamer,
            root_two,
        )],
    )
    .await
    .expect("publish rotated epoch");

    assert!(
        !is_slot_claimed(&mut ctx, channel_pda, epoch_two, claim_index as usize).await,
        "bitmap cleared after rotation"
    );

    let claim_two_ix = claim_with_ring_ix(
        &claimer.pubkey(),
        protocol_pda,
        channel_pda,
        mint.pubkey(),
        treasury_ata,
        claimer_ata,
        spl_t22::id(),
        spl_ata::id(),
        epoch_two,
        claim_index,
        claim_amount,
        vec![],
        id_two,
        streamer,
    );

    send_ixs(&mut ctx, &claimer, vec![claim_two_ix])
        .await
        .expect("claim after rotation");

    assert!(
        is_slot_claimed(&mut ctx, channel_pda, epoch_two, claim_index as usize).await,
        "bitmap flips again on new epoch"
    );
}

fn set_ring_root_ix(
    admin: &Pubkey,
    protocol_pda: Pubkey,
    channel_pda: Pubkey,
    epoch: u64,
    claim_count: u16,
    streamer: Pubkey,
    root: [u8; 32],
) -> Instruction {
    Instruction {
        program_id: token_2022::id(),
        accounts: vec![
            AccountMeta::new(*admin, true),
            AccountMeta::new(protocol_pda, false),
            AccountMeta::new(channel_pda, false),
            AccountMeta::new_readonly(anchor_lang::system_program::ID, false),
        ],
        data: SetMerkleRootRing {
            root,
            epoch,
            claim_count,
            streamer_key: streamer,
        }
        .data(),
    }
}

fn claim_with_ring_ix(
    claimer: &Pubkey,
    protocol_pda: Pubkey,
    channel_pda: Pubkey,
    mint: Pubkey,
    treasury_ata: Pubkey,
    claimer_ata: Pubkey,
    token_program: Pubkey,
    associated_program: Pubkey,
    epoch: u64,
    index: u32,
    amount: u64,
    proof: Vec<[u8; 32]>,
    id: String,
    streamer_key: Pubkey,
) -> Instruction {
    Instruction {
        program_id: token_2022::id(),
        accounts: vec![
            AccountMeta::new(*claimer, true),
            AccountMeta::new(protocol_pda, false),
            AccountMeta::new(channel_pda, false),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new(treasury_ata, false),
            AccountMeta::new(claimer_ata, false),
            AccountMeta::new_readonly(token_program, false),
            AccountMeta::new_readonly(associated_program, false),
            AccountMeta::new_readonly(anchor_lang::system_program::ID, false),
        ],
        data: ClaimWithRing {
            epoch,
            index,
            amount,
            proof,
            id,
            streamer_key,
        }
        .data(),
    }
}

async fn is_slot_claimed(
    ctx: &mut ProgramTestContext,
    channel_pda: Pubkey,
    epoch: u64,
    index: usize,
) -> bool {
    assert!(
        index / 8 < CHANNEL_BITMAP_BYTES,
        "index must fall inside bitmap"
    );
    let account = ctx
        .banks_client
        .get_account(channel_pda)
        .await
        .unwrap()
        .expect("channel state must exist");
    let slot_idx = ChannelState::slot_index(epoch);
    let slot_offset = CHANNEL_SLOT_START_OFFSET + slot_idx * ChannelSlot::LEN;
    let bitmap_offset = slot_offset + CHANNEL_SLOT_BITMAP_OFFSET;
    let byte_offset = bitmap_offset + (index / 8);
    assert!(
        byte_offset < account.data.len(),
        "bitmap offset must stay within account"
    );
    let mask = 1u8 << (index % 8);
    (account.data[byte_offset] & mask) != 0
}
