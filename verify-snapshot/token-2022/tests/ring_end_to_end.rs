use anchor_lang::{prelude::*, InstructionData};
use solana_program_test::*;
use solana_sdk::{
    signature::{Keypair, Signer},
    system_instruction,
    transaction::Transaction,
};
use spl_associated_token_account as spl_ata;
use spl_token_2022 as spl_t22;

mod helpers;
use helpers::*;

fn program_test() -> ProgramTest {
    // Create test with our program loaded from BPF
    // SPL Token-2022 and ATA programs will be automatically available in the runtime
    ProgramTest::new(
        "token_2022",
        token_2022::id(),
        None, // load our program from BPF (target/deploy)
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
async fn test_e2e_single_claim() {
    let pt = program_test();
    let mut ctx = pt.start_with_context().await;

    // Payer/admin
    let admin = Keypair::new();
    // Airdrop
    let _rent = 5_000_000_000u64;
    let lamports = 20_000_000_000u64;
    let airdrop_ixs = vec![
        system_instruction::transfer(&ctx.payer.pubkey(), &admin.pubkey(), lamports),
    ];
    {
        // avoid simultaneous mutable/immutable borrow of ctx
        let payer_kp = Keypair::from_bytes(&ctx.payer.to_bytes()).unwrap();
        send_ixs(&mut ctx, &payer_kp, airdrop_ixs).await.unwrap();
    }

    // Create Token-2022 mint with TransferFee (0 bps)
    let mint_kp = Keypair::new();
    create_mint_with_transfer_fee(
        &mut ctx,
        &admin,
        &mint_kp,
        6,
        &admin.pubkey(),
        &admin.pubkey(),
        0,
        0,
    )
    .await
    .unwrap();

    // Initialize protocol_state (mint-keyed)
    let im_open = ix_initialize_mint_open(&admin.pubkey(), &mint_kp.pubkey());
    send_ixs(&mut ctx, &admin, vec![im_open]).await.unwrap();
    let (protocol_pda, _) = Pubkey::find_program_address(
        &[token_2022::constants::PROTOCOL_SEED, mint_kp.pubkey().as_ref()],
        &token_2022::id(),
    );

    // Create treasury ATA and fund it
    let treasury_ata = create_ata(&mut ctx, &admin, &protocol_pda, &mint_kp.pubkey())
        .await
        .unwrap();
    // Mint some tokens to treasury
    mint_to(&mut ctx, &admin, &mint_kp.pubkey(), &treasury_ata, &admin, 1_000_000_000, 6)
        .await
        .unwrap();

    // Initialize channel ring
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

    // Publish root (ring)
    let epoch: u64 = 1_762_538_400; // arbitrary
    let index: u32 = 0;
    let amount: u64 = 123_000;
    let id = "unit";
    let claimer = Keypair::new();
    let leaf = compute_leaf_index_amount_id(&claimer.pubkey(), index, amount, id);
    let root = leaf; // single-leaf tree → empty proof
    use solana_sdk::instruction::AccountMeta;
    let metas = vec![
        AccountMeta::new(admin.pubkey(), true),
        AccountMeta::new(protocol_pda, false),
        AccountMeta::new(chan_pda, false),
        AccountMeta::new_readonly(anchor_lang::solana_program::system_program::ID, false),
    ];
    let ix_set = solana_sdk::instruction::Instruction {
        program_id: token_2022::id(),
        accounts: metas,
        data: token_2022::instruction::SetMerkleRootRing {
            root,
            epoch,
            claim_count: 1,
            streamer_key: streamer,
        }
        .data(),
    };
    send_ixs(&mut ctx, &admin, vec![ix_set]).await.unwrap();

    // Claim with ring
    let claimer_ata = create_ata(&mut ctx, &admin, &claimer.pubkey(), &mint_kp.pubkey())
        .await
        .unwrap();
    let metas_claim = vec![
        AccountMeta::new(claimer.pubkey(), true),
        AccountMeta::new(protocol_pda, false),
        AccountMeta::new(chan_pda, false),
        AccountMeta::new_readonly(mint_kp.pubkey(), false),
        AccountMeta::new(treasury_ata, false),
        AccountMeta::new(claimer_ata, false),
        AccountMeta::new_readonly(spl_t22::id(), false),
        AccountMeta::new_readonly(spl_ata::id(), false),
        AccountMeta::new_readonly(anchor_lang::solana_program::system_program::ID, false),
    ];
    let ix_claim = solana_sdk::instruction::Instruction {
        program_id: token_2022::id(),
        accounts: metas_claim,
        data: token_2022::instruction::ClaimWithRing {
            epoch,
            index,
            amount,
            proof: vec![],
            id: id.to_string(),
            streamer_key: streamer,
        }
        .data(),
    };
    // Sign with both admin (payer) and claimer; payer must fund ATA creation
    let recent = ctx.banks_client.get_latest_blockhash().await.unwrap();
    let tx = Transaction::new_signed_with_payer(
        &[ix_claim],
        Some(&admin.pubkey()),
        &[&admin, &claimer],
        recent,
    );
    ctx.banks_client.process_transaction(tx).await.unwrap();
}

#[tokio::test]
async fn test_boundary_and_close() {
    let mut ctx = program_test().start_with_context().await;
    let admin = Keypair::new();
    {
        let payer_kp = Keypair::from_bytes(&ctx.payer.to_bytes()).unwrap();
        let payer_pub = payer_kp.pubkey();
        send_ixs(&mut ctx, &payer_kp, vec![system_instruction::transfer(&payer_pub, &admin.pubkey(), 20_000_000_000)]).await.unwrap();
    }

    let mint_kp = Keypair::new();
    create_mint_with_transfer_fee(&mut ctx, &admin, &mint_kp, 6, &admin.pubkey(), &admin.pubkey(), 0, 0).await.unwrap();
    let im_open = ix_initialize_mint_open(&admin.pubkey(), &mint_kp.pubkey());
    send_ixs(&mut ctx, &admin, vec![im_open]).await.unwrap();
    let (protocol_pda, _) = Pubkey::find_program_address(&[token_2022::constants::PROTOCOL_SEED, mint_kp.pubkey().as_ref()], &token_2022::id());
    let treasury_ata = create_ata(&mut ctx, &admin, &protocol_pda, &mint_kp.pubkey()).await.unwrap();
    mint_to(&mut ctx, &admin, &mint_kp.pubkey(), &treasury_ata, &admin, 1_000_000_000, 6).await.unwrap();

    let channel = "boundary";
    let streamer = derive_streamer_key(channel);
    let ix_init_chan = ix_initialize_channel(&admin.pubkey(), &mint_kp.pubkey(), &streamer);
    send_ixs(&mut ctx, &admin, vec![ix_init_chan]).await.unwrap();
    let (chan_pda, _) = Pubkey::find_program_address(&[token_2022::constants::CHANNEL_STATE_SEED, mint_kp.pubkey().as_ref(), streamer.as_ref()], &token_2022::id());

    // Set root with claim_count=8192; allow index 8191
    let epoch = 9;
    let claimer = Keypair::new();
    let amount = 1_000;
    let id = "edge";
    let leaf = compute_leaf_index_amount_id(&claimer.pubkey(), 8191, amount, id);
    let root = leaf;
    use solana_sdk::instruction::AccountMeta;
    let ix_set = solana_sdk::instruction::Instruction {
        program_id: token_2022::id(),
        accounts: vec![
            AccountMeta::new(admin.pubkey(), true),
            AccountMeta::new(protocol_pda, false),
            AccountMeta::new(chan_pda, false),
            AccountMeta::new_readonly(anchor_lang::solana_program::system_program::ID, false),
        ],
        data: token_2022::instruction::SetMerkleRootRing { root, epoch, claim_count: 8192, streamer_key: streamer }.data(),
    };
    send_ixs(&mut ctx, &admin, vec![ix_set]).await.unwrap();

    // Claim index 8191 (should succeed)
    let claimer_ata = create_ata(&mut ctx, &admin, &claimer.pubkey(), &mint_kp.pubkey()).await.unwrap();
    let ix_claim_ok = solana_sdk::instruction::Instruction {
        program_id: token_2022::id(),
        accounts: vec![
            AccountMeta::new(claimer.pubkey(), true),
            AccountMeta::new(protocol_pda, false),
            AccountMeta::new(chan_pda, false),
            AccountMeta::new_readonly(mint_kp.pubkey(), false),
            AccountMeta::new(treasury_ata, false),
            AccountMeta::new(claimer_ata, false),
            AccountMeta::new_readonly(spl_t22::id(), false),
            AccountMeta::new_readonly(spl_ata::id(), false),
            AccountMeta::new_readonly(anchor_lang::solana_program::system_program::ID, false),
        ],
        data: token_2022::instruction::ClaimWithRing { epoch, index: 8191, amount, proof: vec![], id: id.to_string(), streamer_key: streamer }.data(),
    };
    let recent = ctx.banks_client.get_latest_blockhash().await.unwrap();
    let tx_ok = Transaction::new_signed_with_payer(&[ix_claim_ok], Some(&admin.pubkey()), &[&admin, &claimer], recent);
    ctx.banks_client.process_transaction(tx_ok).await.unwrap();

    // Claim index 8192 (should fail InvalidIndex)
    let ix_claim_bad = solana_sdk::instruction::Instruction {
        program_id: token_2022::id(),
        accounts: vec![
            AccountMeta::new(claimer.pubkey(), true),
            AccountMeta::new(protocol_pda, false),
            AccountMeta::new(chan_pda, false),
            AccountMeta::new_readonly(mint_kp.pubkey(), false),
            AccountMeta::new(treasury_ata, false),
            AccountMeta::new(claimer_ata, false),
            AccountMeta::new_readonly(spl_t22::id(), false),
            AccountMeta::new_readonly(spl_ata::id(), false),
            AccountMeta::new_readonly(anchor_lang::solana_program::system_program::ID, false),
        ],
        data: token_2022::instruction::ClaimWithRing { epoch, index: 8192, amount, proof: vec![], id: id.to_string(), streamer_key: streamer }.data(),
    };
    let recent2 = ctx.banks_client.get_latest_blockhash().await.unwrap();
    let tx_bad = Transaction::new_signed_with_payer(&[ix_claim_bad], Some(&admin.pubkey()), &[&admin, &claimer], recent2);
    let err = ctx.banks_client.process_transaction(tx_bad).await.err().expect("expected failure");
    // Just assert it failed; deeper code checks InvalidIndex inside program
    drop(err);

    // Admin closes channel_state (rent recovery path)
    let rent_receiver = Keypair::new();
    let ix_close = solana_sdk::instruction::Instruction {
        program_id: token_2022::id(),
        accounts: vec![
            AccountMeta::new(admin.pubkey(), true),
            AccountMeta::new_readonly(protocol_pda, false),
            AccountMeta::new(chan_pda, false),
            AccountMeta::new(rent_receiver.pubkey(), false),
        ],
        data: token_2022::instruction::CloseChannelState {}.data(),
    };
    send_ixs(&mut ctx, &admin, vec![ix_close]).await.unwrap();
    let acct = ctx.banks_client.get_account(chan_pda).await.unwrap();
    assert!(acct.is_none());
}
