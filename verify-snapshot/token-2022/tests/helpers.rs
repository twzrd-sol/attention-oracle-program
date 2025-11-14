use anchor_lang::prelude::*;
use anchor_lang::InstructionData;
use solana_program_test::*;
use solana_sdk::{
    instruction::Instruction,
    signature::{Keypair, Signer},
    system_instruction,
    transaction::Transaction,
};
use spl_associated_token_account as spl_ata;
use spl_token_2022 as spl_t22;

pub fn derive_streamer_key(channel: &str) -> Pubkey {
    let lower = channel.to_ascii_lowercase();
    let hash = solana_program::keccak::hashv(&[b"channel:", lower.as_bytes()]);
    Pubkey::new_from_array(hash.0)
}

pub async fn send_ixs(context: &mut ProgramTestContext, payer: &Keypair, ixs: Vec<Instruction>) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let recent = context.banks_client.get_latest_blockhash().await?;
    let tx = Transaction::new_signed_with_payer(&ixs, Some(&payer.pubkey()), &[payer], recent);
    context.banks_client.process_transaction(tx).await?;
    Ok(())
}

pub async fn create_mint_with_transfer_fee(
    context: &mut ProgramTestContext,
    payer: &Keypair,
    mint_kp: &Keypair,
    decimals: u8,
    config_authority: &Pubkey,
    withdraw_authority: &Pubkey,
    fee_bps: u16,
    max_fee: u64,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    use spl_t22::extension::ExtensionType;
    let mint_len = ExtensionType::try_calculate_account_len::<spl_t22::state::Mint>(&[ExtensionType::TransferFeeConfig])
        .unwrap();
    let rent = context.banks_client.get_rent().await?.minimum_balance(mint_len);
    // Create mint account owned by Token-2022 program
    let create = system_instruction::create_account(
        &payer.pubkey(),
        &mint_kp.pubkey(),
        rent,
        mint_len as u64,
        &spl_t22::id(),
    );

    // Initialize transfer-fee config first
    let init_fee = spl_t22::extension::transfer_fee::instruction::initialize_transfer_fee_config(
        &spl_t22::id(),
        &mint_kp.pubkey(),
        Some(config_authority),
        Some(withdraw_authority),
        fee_bps,
        max_fee,
    )?;
    // Initialize mint (decimals + authorities)
    let init_mint = spl_t22::instruction::initialize_mint2(&spl_t22::id(), &mint_kp.pubkey(), &payer.pubkey(), None, decimals)?;

    // Need both payer and mint_kp as signers for create_account
    let recent = context.banks_client.get_latest_blockhash().await?;
    let tx = Transaction::new_signed_with_payer(
        &[create, init_fee, init_mint],
        Some(&payer.pubkey()),
        &[payer, mint_kp], // Both signers required
        recent
    );
    context.banks_client.process_transaction(tx).await?;
    Ok(())
}

pub async fn create_ata(
    context: &mut ProgramTestContext,
    payer: &Keypair,
    owner: &Pubkey,
    mint: &Pubkey,
) -> std::result::Result<Pubkey, Box<dyn std::error::Error>> {
    let ata = spl_ata::get_associated_token_address_with_program_id(owner, mint, &spl_t22::id());
    let ix = spl_ata::instruction::create_associated_token_account(
        &payer.pubkey(),
        owner,
        mint,
        &spl_t22::id(),
    );
    send_ixs(context, payer, vec![ix]).await?;
    Ok(ata)
}

pub async fn mint_to(
    context: &mut ProgramTestContext,
    payer: &Keypair,
    mint: &Pubkey,
    dest: &Pubkey,
    authority: &Keypair,
    amount: u64,
    decimals: u8,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let ix = spl_t22::instruction::mint_to_checked(
        &spl_t22::id(),
        mint,
        dest,
        &authority.pubkey(),
        &[],
        amount,
        decimals,
    )?;
    // Need both payer and authority
    let recent = context.banks_client.get_latest_blockhash().await?;
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &[payer, authority], recent);
    context.banks_client.process_transaction(tx).await?;
    Ok(())
}

pub fn ix_initialize_mint_open(admin: &Pubkey, mint: &Pubkey) -> Instruction {
    use solana_sdk::instruction::AccountMeta;
    let (protocol, _) = Pubkey::find_program_address(&[token_2022::constants::PROTOCOL_SEED, mint.as_ref()], &token_2022::id());
    let (fee_cfg, _) = Pubkey::find_program_address(&[token_2022::constants::PROTOCOL_SEED, mint.as_ref(), b"fee_config"], &token_2022::id());
    let metas = vec![
        AccountMeta::new_readonly(*admin, true),
        AccountMeta::new_readonly(*mint, false),
        AccountMeta::new(protocol, false),
        AccountMeta::new(fee_cfg, false),
        AccountMeta::new_readonly(anchor_lang::system_program::ID, false),
    ];
    Instruction {
        program_id: token_2022::id(),
        accounts: metas,
        data: token_2022::instruction::InitializeMintOpen { fee_basis_points: 0, max_fee: 0 }.data(),
    }
}

pub fn ix_initialize_channel(payer: &Pubkey, mint: &Pubkey, streamer: &Pubkey) -> Instruction {
    use solana_sdk::instruction::AccountMeta;
    let (protocol, _) = Pubkey::find_program_address(&[token_2022::constants::PROTOCOL_SEED, mint.as_ref()], &token_2022::id());
    let (chan, _) = Pubkey::find_program_address(&[token_2022::constants::CHANNEL_STATE_SEED, mint.as_ref(), streamer.as_ref()], &token_2022::id());
    let metas = vec![
        AccountMeta::new(*payer, true),
        AccountMeta::new_readonly(protocol, false),
        AccountMeta::new(chan, false),
        AccountMeta::new_readonly(anchor_lang::system_program::ID, false),
    ];
    Instruction {
        program_id: token_2022::id(),
        accounts: metas,
        data: token_2022::instruction::InitializeChannel { streamer_key: *streamer }.data(),
    }
}
