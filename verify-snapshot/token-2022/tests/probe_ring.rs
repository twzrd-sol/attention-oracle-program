use anchor_lang::prelude::*;
use anchor_lang::InstructionData;
use solana_program::system_program;
use solana_program_test::*;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk::system_instruction;
use solana_sdk::transaction::Transaction;
use spl_associated_token_account as spl_ata;
use spl_token_2022 as spl_t22;

// Minimal probe: set merkle root, claim once (ok), claim again (AlreadyClaimed), rotate and claim (ok).

#[tokio::test]
async fn probe_claim_with_ring_basic() {
    let mut ctx = ProgramTest::new(
        "token_2022",
        token_2022::id(),
        processor!(token_2022::entry),
    )
    .start_with_context()
    .await;

    // Admin/payer
    let admin = Keypair::new();
    fund(&mut ctx, &admin, 5_000_000_000).await;

    // Create Token-2022 mint with transfer-fee extension (authority = admin)
    let mint_kp = Keypair::new();
    create_mint_t22(&mut ctx, &admin, &mint_kp, 6).await;

    // Initialize protocol (open) for this mint
    send_ixs(
        &mut ctx,
        &admin,
        vec![ix_initialize_mint_open(&admin.pubkey(), &mint_kp.pubkey())],
    )
    .await
    .unwrap();

    // Initialize channel
    let streamer = Pubkey::new_unique();
    send_ixs(
        &mut ctx,
        &admin,
        vec![ix_initialize_channel(
            &admin.pubkey(),
            &mint_kp.pubkey(),
            &streamer,
        )],
    )
    .await
    .unwrap();

    // Protocol PDA (for metas)
    let (protocol_pda, _) = Pubkey::find_program_address(
        &[
            token_2022::constants::PROTOCOL_SEED,
            mint_kp.pubkey().as_ref(),
        ],
        &token_2022::id(),
    );

    // Claimer and ATA
    let claimer = Keypair::new();
    fund(&mut ctx, &claimer, 2_000_000_000).await;
    let claimer_ata = spl_ata::get_associated_token_address_with_program_id(
        &claimer.pubkey(),
        &mint_kp.pubkey(),
        &spl_t22::id(),
    );

    // Publish root for epoch N
    let epoch_n: u64 = 1_000;
    let index: u32 = 0;
    let amount: u64 = 1;
    // Placeholder leaf; current simplified claim impl only checks non-empty proof
    let leaf = [0u8; 32];
    send_ixs(
        &mut ctx,
        &admin,
        vec![ix_set_merkle_root_ring(
            &admin.pubkey(),
            protocol_pda,
            pda_channel(&mint_kp.pubkey(), &streamer),
            epoch_n,
            1,
            streamer,
            leaf,
        )],
    )
    .await
    .unwrap();

    // First claim should succeed
    send_ixs(
        &mut ctx,
        &claimer,
        vec![ix_claim_with_ring(
            &claimer.pubkey(),
            protocol_pda,
            pda_channel(&mint_kp.pubkey(), &streamer),
            epoch_n,
            index,
            amount,
            vec![[1u8; 32]],
            streamer,
        )],
    )
    .await
    .unwrap();

    // Second claim should fail with AlreadyClaimed
    let res = send_ixs(
        &mut ctx,
        &claimer,
        vec![ix_claim_with_ring(
            &claimer.pubkey(),
            protocol_pda,
            pda_channel(&mint_kp.pubkey(), &streamer),
            epoch_n,
            index,
            amount,
            vec![[2u8; 32]],
            streamer,
        )],
    )
    .await;
    assert!(res.is_err(), "double claim must fail");

    // Rotate: publish at N+RING and claim again (ok)
    let epoch_rot = epoch_n + token_2022::constants::CHANNEL_RING_SLOTS as u64;
    send_ixs(
        &mut ctx,
        &admin,
        vec![ix_set_merkle_root_ring(
            &admin.pubkey(),
            protocol_pda,
            pda_channel(&mint_kp.pubkey(), &streamer),
            epoch_rot,
            1,
            streamer,
            leaf,
        )],
    )
    .await
    .unwrap();

    send_ixs(
        &mut ctx,
        &claimer,
        vec![ix_claim_with_ring(
            &claimer.pubkey(),
            protocol_pda,
            pda_channel(&mint_kp.pubkey(), &streamer),
            epoch_rot,
            index,
            amount,
            vec![[3u8; 32]],
            streamer,
        )],
    )
    .await
    .unwrap();
}

fn pda_channel(mint: &Pubkey, streamer: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[
            token_2022::constants::CHANNEL_STATE_SEED,
            mint.as_ref(),
            streamer.as_ref(),
        ],
        &token_2022::id(),
    )
    .0
}

async fn fund(ctx: &mut ProgramTestContext, dest: &Keypair, lamports: u64) {
    let ix = system_instruction::transfer(&ctx.payer.pubkey(), &dest.pubkey(), lamports);
    let bh = ctx.banks_client.get_latest_blockhash().await.unwrap();
    let tx =
        Transaction::new_signed_with_payer(&[ix], Some(&ctx.payer.pubkey()), &[&ctx.payer], bh);
    ctx.banks_client.process_transaction(tx).await.unwrap();
}

async fn send_ixs(
    context: &mut ProgramTestContext,
    payer: &Keypair,
    ixs: Vec<Instruction>,
) -> std::result::Result<(), BanksClientError> {
    let bh = context.banks_client.get_latest_blockhash().await.unwrap();
    let tx = Transaction::new_signed_with_payer(&ixs, Some(&payer.pubkey()), &[payer], bh);
    context.banks_client.process_transaction(tx).await
}

async fn create_mint_t22(
    ctx: &mut ProgramTestContext,
    payer: &Keypair,
    mint_kp: &Keypair,
    decimals: u8,
) {
    use spl_t22::extension::ExtensionType;
    let mint_len = ExtensionType::try_calculate_account_len::<spl_t22::state::Mint>(&[
        ExtensionType::TransferFeeConfig,
    ])
    .unwrap();
    let rent = ctx
        .banks_client
        .get_rent()
        .await
        .unwrap()
        .minimum_balance(mint_len);
    let create = system_instruction::create_account(
        &payer.pubkey(),
        &mint_kp.pubkey(),
        rent,
        mint_len as u64,
        &spl_t22::id(),
    );
    let init_fee = spl_t22::extension::transfer_fee::instruction::initialize_transfer_fee_config(
        &spl_t22::id(),
        &mint_kp.pubkey(),
        Some(&payer.pubkey()),
        Some(&payer.pubkey()),
        0,
        0,
    )
    .unwrap();
    let init_mint = spl_t22::instruction::initialize_mint2(
        &spl_t22::id(),
        &mint_kp.pubkey(),
        &payer.pubkey(),
        None,
        decimals,
    )
    .unwrap();

    let bh = ctx.banks_client.get_latest_blockhash().await.unwrap();
    let tx = Transaction::new_signed_with_payer(
        &[create, init_fee, init_mint],
        Some(&payer.pubkey()),
        &[payer, mint_kp],
        bh,
    );
    ctx.banks_client.process_transaction(tx).await.unwrap();
}

async fn create_ata(
    ctx: &mut ProgramTestContext,
    payer: &Keypair,
    owner: &Pubkey,
    mint: &Pubkey,
) -> Pubkey {
    let ata = spl_ata::get_associated_token_address_with_program_id(owner, mint, &spl_t22::id());
    let ix = spl_ata::instruction::create_associated_token_account(
        &payer.pubkey(),
        owner,
        mint,
        &spl_t22::id(),
    );
    send_ixs(ctx, payer, vec![ix]).await.unwrap();
    ata
}

async fn mint_to(
    ctx: &mut ProgramTestContext,
    payer: &Keypair,
    mint: &Pubkey,
    dest: &Pubkey,
    authority: &Keypair,
    amount: u64,
    decimals: u8,
) {
    let ix = spl_t22::instruction::mint_to_checked(
        &spl_t22::id(),
        mint,
        dest,
        &authority.pubkey(),
        &[],
        amount,
        decimals,
    )
    .unwrap();
    let bh = ctx.banks_client.get_latest_blockhash().await.unwrap();
    let tx =
        Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &[payer, authority], bh);
    ctx.banks_client.process_transaction(tx).await.unwrap();
}

fn ix_initialize_mint_open(admin: &Pubkey, mint: &Pubkey) -> Instruction {
    let (protocol, _) = Pubkey::find_program_address(
        &[token_2022::constants::PROTOCOL_SEED, mint.as_ref()],
        &token_2022::id(),
    );
    let (fee_cfg, _) = Pubkey::find_program_address(
        &[
            token_2022::constants::PROTOCOL_SEED,
            mint.as_ref(),
            b"fee_config",
        ],
        &token_2022::id(),
    );
    Instruction {
        program_id: token_2022::id(),
        accounts: vec![
            AccountMeta::new_readonly(*admin, true),
            AccountMeta::new_readonly(*mint, false),
            AccountMeta::new(protocol, false),
            AccountMeta::new(fee_cfg, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: token_2022::instruction::InitializeMintOpen {
            fee_basis_points: 0,
            max_fee: 0,
        }
        .data(),
    }
}

fn ix_initialize_channel(payer: &Pubkey, mint: &Pubkey, streamer: &Pubkey) -> Instruction {
    let (protocol, _) = Pubkey::find_program_address(
        &[token_2022::constants::PROTOCOL_SEED, mint.as_ref()],
        &token_2022::id(),
    );
    let (chan, _) = Pubkey::find_program_address(
        &[
            token_2022::constants::CHANNEL_STATE_SEED,
            mint.as_ref(),
            streamer.as_ref(),
        ],
        &token_2022::id(),
    );
    Instruction {
        program_id: token_2022::id(),
        accounts: vec![
            AccountMeta::new(*payer, true),
            AccountMeta::new_readonly(protocol, false),
            AccountMeta::new(chan, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: token_2022::instruction::InitializeChannel {
            streamer_key: *streamer,
        }
        .data(),
    }
}

fn ix_set_merkle_root_ring(
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
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: token_2022::instruction::SetMerkleRootRing {
            root,
            epoch,
            claim_count,
            streamer_key: streamer,
        }
        .data(),
    }
}

fn ix_claim_with_ring(
    claimer: &Pubkey,
    protocol_pda: Pubkey,
    channel_pda: Pubkey,
    epoch: u64,
    index: u32,
    amount: u64,
    proof: Vec<[u8; 32]>,
    streamer_key: Pubkey,
) -> Instruction {
    Instruction {
        program_id: token_2022::id(),
        accounts: vec![
            AccountMeta::new(*claimer, true),
            AccountMeta::new(protocol_pda, false),
            AccountMeta::new(channel_pda, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: token_2022::instruction::ClaimWithRing {
            epoch,
            index,
            amount,
            proof,
            streamer_key,
        }
        .data(),
    }
}
