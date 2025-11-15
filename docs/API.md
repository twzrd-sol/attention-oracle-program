# API Reference - Attention Oracle

**Last Updated:** October 30, 2025
**Program ID:** `4rArjoSZKrYkoE7hkvZNBP2Wpxovr78cfkxBnNwFNPn5`
**Anchor Version:** 0.30.1

---

## Table of Contents

1. [Overview](#overview)
2. [Instruction Categories](#instruction-categories)
3. [Initialization Instructions](#initialization-instructions)
4. [Claim Instructions](#claim-instructions)
5. [Admin Instructions](#admin-instructions)
6. [Merkle Root Management](#merkle-root-management)
7. [Channel Management](#channel-management)
8. [Passport System](#passport-system)
9. [Points System](#points-system)
10. [Cleanup Instructions](#cleanup-instructions)
11. [Type Definitions](#type-definitions)
12. [Error Codes](#error-codes)

---

## Overview

The Attention Oracle program exposes **25+ instructions** organized into logical categories. Most instructions have two variants:

- **Singleton** - Admin-gated, single protocol instance
- **Open** - Permissionless, mint-keyed protocol instances

**For hackathon/production:** Focus on **`claim_channel_open`** and **`set_channel_merkle_root`** (ring buffer system).

---

## Instruction Categories

| Category | Instructions | Access Level |
|----------|--------------|--------------|
| **Initialization** | `initialize_mint`, `initialize_channel` | Admin (one-time) |
| **Claims** | `claim_open`, `claim_channel_open`, `claim_with_ring` | Permissionless |
| **Admin** | `update_admin`, `update_publisher`, `set_paused`, `set_policy` | Admin only |
| **Merkle Roots** | `set_merkle_root_open`, `set_channel_merkle_root`, `set_merkle_root_ring` | Publisher only |
| **Passport** | `mint_passport`, `upgrade_passport`, `revoke_passport` | Publisher only |
| **Points** | `claim_points_open`, `require_points_ge` | Permissionless |
| **Cleanup** | `close_epoch_state`, `close_old_epoch_state` | Admin only |

---

## Initialization Instructions

### initialize_mint_open

**Description:** Initialize a new protocol instance keyed by Token-2022 mint.

**Authority:** Admin (one-time setup)

**Parameters:**
```rust
pub fn initialize_mint_open(
    ctx: Context<InitializeMintOpen>,
    fee_basis_points: u16,  // Transfer fee (e.g., 100 = 1%)
    max_fee: u64,           // Maximum fee per transfer (in lamports)
) -> Result<()>
```

**Accounts:**
```rust
#[derive(Accounts)]
pub struct InitializeMintOpen<'info> {
    #[account(
        init,
        payer = admin,
        space = 8 + ProtocolState::LEN,
        seeds = [b"protocol-state", mint.key().as_ref()],
        bump
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    pub mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut)]
    pub admin: Signer<'info>,

    pub system_program: Program<'info, System>,
}
```

**TypeScript Example:**
```typescript
const [protocolPda] = anchor.web3.PublicKey.findProgramAddressSync(
  [Buffer.from("protocol-state"), mint.toBuffer()],
  programId
);

await program.methods
  .initializeMintOpen(
    100,  // 1% fee
    new anchor.BN(100_000_000)  // 0.1 token max fee
  )
  .accounts({
    protocolState: protocolPda,
    mint: mintPubkey,
    admin: adminKeypair.publicKey,
    systemProgram: anchor.web3.SystemProgram.programId,
  })
  .signers([adminKeypair])
  .rpc();
```

**Events Emitted:**
```rust
pub struct ProtocolInitialized {
    pub admin: Pubkey,
    pub mint: Pubkey,
    pub fee_basis_points: u16,
}
```

---

### initialize_channel

**Description:** Initialize a channel ring buffer for a specific streamer.

**Authority:** Publisher

**Parameters:**
```rust
pub fn initialize_channel(
    ctx: Context<InitializeChannel>,
    streamer_key: Pubkey,  // Streamer/channel identifier
) -> Result<()>
```

**Accounts:**
```rust
#[derive(Accounts)]
pub struct InitializeChannel<'info> {
    #[account(
        init,
        payer = publisher,
        space = 8 + ChannelState::LEN,
        seeds = [
            b"channel-ring",
            protocol_state.key().as_ref(),
            streamer_key.as_ref()
        ],
        bump
    )]
    pub channel_state: Account<'info, ChannelState>,

    #[account(seeds = [b"protocol-state", mint.key().as_ref()], bump)]
    pub protocol_state: Account<'info, ProtocolState>,

    #[account(mut)]
    pub publisher: Signer<'info>,

    pub mint: Box<InterfaceAccount<'info, Mint>>,
    pub system_program: Program<'info, System>,
}
```

**TypeScript Example:**
```typescript
const streamerKey = new anchor.web3.PublicKey("...");

const [channelPda] = anchor.web3.PublicKey.findProgramAddressSync(
  [
    Buffer.from("channel-ring"),
    protocolPda.toBuffer(),
    streamerKey.toBuffer()
  ],
  programId
);

await program.methods
  .initializeChannel(streamerKey)
  .accounts({
    channelState: channelPda,
    protocolState: protocolPda,
    publisher: publisherKeypair.publicKey,
    mint: mintPubkey,
    systemProgram: anchor.web3.SystemProgram.programId,
  })
  .signers([publisherKeypair])
  .rpc();
```

---

## Claim Instructions

### claim_channel_open

**Description:** Claim rewards using channel ring buffer and merkle proof.

**Authority:** Permissionless (any user with valid proof)

**Parameters:**
```rust
pub fn claim_channel_open(
    ctx: Context<ClaimChannel>,
    channel: String,       // Channel identifier (e.g., "twitch:xqc")
    epoch: u64,           // Epoch number
    index: u32,           // Leaf index in merkle tree
    amount: u64,          // Claim amount (in token units)
    id: String,           // User identifier (e.g., Twitch username)
    proof: Vec<[u8; 32]>, // Merkle proof path
) -> Result<()>
```

**Accounts:**
```rust
#[derive(Accounts)]
pub struct ClaimChannel<'info> {
    #[account(seeds = [b"protocol-state", mint.key().as_ref()], bump)]
    pub protocol_state: Account<'info, ProtocolState>,

    #[account(
        mut,
        seeds = [
            b"channel-state",
            protocol_state.key().as_ref(),
            channel.as_bytes()
        ],
        bump
    )]
    pub channel_state: Account<'info, ChannelState>,

    #[account(
        init,
        payer = user,
        space = 8 + UserClaim::LEN,
        seeds = [
            b"user-claim",
            user.key().as_ref(),
            channel_state.key().as_ref(),
            &epoch.to_le_bytes()
        ],
        bump
    )]
    pub user_claim: Account<'info, UserClaim>,

    #[account(mut)]
    pub user_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}
```

**TypeScript Example:**
```typescript
const channelId = "twitch:xqc";

const [channelPda] = anchor.web3.PublicKey.findProgramAddressSync(
  [
    Buffer.from("channel-state"),
    protocolPda.toBuffer(),
    Buffer.from(channelId)
  ],
  programId
);

const [claimPda] = anchor.web3.PublicKey.findProgramAddressSync(
  [
    Buffer.from("user-claim"),
    userPubkey.toBuffer(),
    channelPda.toBuffer(),
    Buffer.from(epoch.toString('le', 8))  // u64 little-endian
  ],
  programId
);

// Fetch proof from API
const proof = await fetch(`https://api.twzrd.com/proof/${epoch}/${user}`)
  .then(r => r.json());

await program.methods
  .claimChannelOpen(
    channelId,
    new anchor.BN(epoch),
    index,
    new anchor.BN(proof.amount),
    userId,  // e.g., "xqcL"
    proof.proof.map(p => Array.from(Buffer.from(p, 'hex')))
  )
  .accounts({
    protocolState: protocolPda,
    channelState: channelPda,
    userClaim: claimPda,
    userTokenAccount: userAta,
    mint: mintPubkey,
    user: userKeypair.publicKey,
    tokenProgram: TOKEN_2022_PROGRAM_ID,
    systemProgram: anchor.web3.SystemProgram.programId,
  })
  .signers([userKeypair])
  .rpc();
```

**Events Emitted:**
```rust
pub struct TokensClaimed {
    pub user: Pubkey,
    pub channel: String,
    pub epoch: u64,
    pub amount: u64,
    pub id: String,
}
```

**Validation Checks:**
- ✅ Protocol not paused
- ✅ Epoch is sealed
- ✅ Merkle proof valid
- ✅ User hasn't claimed this epoch
- ✅ Amount matches proof

---

### claim_channel_open_with_receipt

**Description:** Claim rewards with optional cNFT receipt minting.

**Authority:** Permissionless

**Parameters:**
```rust
pub fn claim_channel_open_with_receipt(
    ctx: Context<ClaimChannelWithReceipt>,
    channel: String,
    epoch: u64,
    index: u32,
    amount: u64,
    id: String,
    proof: Vec<[u8; 32]>,
    mint_receipt: bool,  // Whether to mint cNFT receipt
) -> Result<()>
```

**Additional Accounts (vs claim_channel_open):**
```rust
pub bubblegum_program: Option<Program<'info, Bubblegum>>,
pub compression_program: Option<Program<'info, Compression>>,
pub merkle_tree: Option<UncheckedAccount<'info>>,  // cNFT tree
```

**TypeScript Example:**
```typescript
await program.methods
  .claimChannelOpenWithReceipt(
    channelId,
    epoch,
    index,
    amount,
    userId,
    proof,
    true  // mint_receipt = true
  )
  .accounts({
    // ... same as claim_channel_open
    bubblegumProgram: BUBBLEGUM_PROGRAM_ID,
    compressionProgram: COMPRESSION_PROGRAM_ID,
    merkleTree: merkleTreePubkey,
  })
  .rpc();
```

**Receipt Format:**
```json
{
  "name": "TWZRD Claim Receipt #42",
  "symbol": "TWZRD",
  "uri": "https://api.twzrd.com/receipt/20251030/42",
  "collection": "TWZRD Receipts",
  "attributes": [
    {"trait_type": "Epoch", "value": "20251030"},
    {"trait_type": "Amount", "value": "1000000000"},
    {"trait_type": "Channel", "value": "twitch:xqc"}
  ]
}
```

---

### claim_open

**Description:** Legacy claim instruction (single-epoch, no ring buffer).

**Authority:** Permissionless

**Parameters:**
```rust
pub fn claim_open(
    ctx: Context<ClaimOpen>,
    streamer_index: u8,
    index: u32,
    amount: u64,
    id: String,
    proof: Vec<[u8; 32]>,
    channel: Option<String>,
    twzrd_epoch: Option<u64>,
    receipt_proof: Option<CnftReceiptProof>,
) -> Result<()>
```

**Note:** Prefer `claim_channel_open` for production use (ring buffer support).

---

## Admin Instructions

### update_admin_open

**Description:** Transfer admin authority to a new address.

**Authority:** Current admin only

**Parameters:**
```rust
pub fn update_admin_open(
    ctx: Context<UpdateAdminOpen>,
    new_admin: Pubkey,
) -> Result<()>
```

**Accounts:**
```rust
#[derive(Accounts)]
pub struct UpdateAdminOpen<'info> {
    #[account(
        mut,
        seeds = [b"protocol-state", mint.key().as_ref()],
        bump,
        constraint = protocol_state.admin == admin.key() @ ErrorCode::Unauthorized
    )]
    pub protocol_state: Account<'info, ProtocolState>,

    pub admin: Signer<'info>,
    pub mint: Box<InterfaceAccount<'info, Mint>>,
}
```

**TypeScript Example:**
```typescript
await program.methods
  .updateAdminOpen(newAdminPubkey)
  .accounts({
    protocolState: protocolPda,
    admin: currentAdminKeypair.publicKey,
    mint: mintPubkey,
  })
  .signers([currentAdminKeypair])
  .rpc();
```

**⚠️ Security Warning:**
- Single-step transfer (no accept mechanism)
- Typo in `new_admin` = permanent loss of control
- **Planned fix:** Two-step transfer pattern

---

### update_publisher_open

**Description:** Rotate publisher authority (who can seal epochs).

**Authority:** Admin only

**Parameters:**
```rust
pub fn update_publisher_open(
    ctx: Context<UpdatePublisherOpen>,
    new_publisher: Pubkey,
) -> Result<()>
```

**TypeScript Example:**
```typescript
await program.methods
  .updatePublisherOpen(newPublisherPubkey)
  .accounts({
    protocolState: protocolPda,
    admin: adminKeypair.publicKey,
    mint: mintPubkey,
  })
  .signers([adminKeypair])
  .rpc();
```

**Use Cases:**
- Weekly publisher key rotation
- Compromised publisher recovery
- Migrate to HSM-backed key

---

### set_paused_open

**Description:** Emergency circuit breaker (disable claims).

**Authority:** Admin only

**Parameters:**
```rust
pub fn set_paused_open(
    ctx: Context<SetPausedOpen>,
    paused: bool,  // true = pause, false = unpause
) -> Result<()>
```

**TypeScript Example:**
```typescript
// Pause protocol
await program.methods
  .setPausedOpen(true)
  .accounts({
    protocolState: protocolPda,
    admin: adminKeypair.publicKey,
    mint: mintPubkey,
  })
  .signers([adminKeypair])
  .rpc();

console.log("⛔ Protocol paused - claims disabled");

// Unpause protocol
await program.methods
  .setPausedOpen(false)
  .accounts({ /* same */ })
  .signers([adminKeypair])
  .rpc();

console.log("✅ Protocol resumed - claims enabled");
```

**Effects When Paused:**
- ❌ `claim_open` fails with `ProtocolPaused` error
- ❌ `claim_channel_open` fails
- ✅ Admin operations still work
- ✅ Publisher can still seal epochs

---

### set_policy_open

**Description:** Enable/disable cNFT receipt requirement.

**Authority:** Admin only

**Parameters:**
```rust
pub fn set_policy_open(
    ctx: Context<SetPolicyOpen>,
    require_receipt: bool,
) -> Result<()>
```

**TypeScript Example:**
```typescript
await program.methods
  .setPolicyOpen(true)  // Require receipts for all claims
  .accounts({
    protocolState: protocolPda,
    admin: adminKeypair.publicKey,
    mint: mintPubkey,
  })
  .signers([adminKeypair])
  .rpc();
```

**Effects:**
- When `true`: Claims must provide valid cNFT receipt proof
- When `false`: Claims work without receipts (current default)

---

## Merkle Root Management

### set_channel_merkle_root

**Description:** Seal an epoch by publishing merkle root to channel state.

**Authority:** Publisher only

**Parameters:**
```rust
pub fn set_channel_merkle_root(
    ctx: Context<SetChannelMerkleRoot>,
    channel: String,
    epoch: u64,
    root: [u8; 32],
) -> Result<()>
```

**Accounts:**
```rust
#[derive(Accounts)]
pub struct SetChannelMerkleRoot<'info> {
    #[account(seeds = [b"protocol-state", mint.key().as_ref()], bump)]
    pub protocol_state: Account<'info, ProtocolState>,

    #[account(
        mut,
        seeds = [
            b"channel-state",
            protocol_state.key().as_ref(),
            channel.as_bytes()
        ],
        bump,
        constraint = !channel_state.sealed @ ErrorCode::EpochAlreadySealed
    )]
    pub channel_state: Account<'info, ChannelState>,

    #[account(constraint = publisher.key() == protocol_state.publisher @ ErrorCode::Unauthorized)]
    pub publisher: Signer<'info>,

    pub mint: Box<InterfaceAccount<'info, Mint>>,
}
```

**TypeScript Example:**
```typescript
// Build merkle tree off-chain
const tree = buildMerkleTree(participants);
const root = tree.getRootHash();

await program.methods
  .setChannelMerkleRoot(
    channelId,
    new anchor.BN(epoch),
    Array.from(root)
  )
  .accounts({
    protocolState: protocolPda,
    channelState: channelPda,
    publisher: publisherKeypair.publicKey,
    mint: mintPubkey,
  })
  .signers([publisherKeypair])
  .rpc();

console.log(`✅ Epoch ${epoch} sealed with root: ${root.toString('hex')}`);
```

**Events Emitted:**
```rust
pub struct EpochSealed {
    pub channel: String,
    pub epoch: u64,
    pub merkle_root: [u8; 32],
    pub sealed_at: i64,
}
```

**Validation Checks:**
- ✅ Signer is authorized publisher
- ✅ Epoch not already sealed
- ✅ Root is non-zero

---

### set_merkle_root_ring

**Description:** Set merkle root using ring buffer (10-slot circular buffer).

**Authority:** Publisher only

**Parameters:**
```rust
pub fn set_merkle_root_ring(
    ctx: Context<SetMerkleRootRing>,
    root: [u8; 32],
    epoch: u64,
    claim_count: u16,
    streamer_key: Pubkey,
) -> Result<()>
```

**Ring Buffer Behavior:**
- Stores last 10 epochs in circular buffer
- Oldest epoch overwritten when buffer full
- Claims must use epochs within last 10

**TypeScript Example:**
```typescript
await program.methods
  .setMerkleRootRing(
    Array.from(root),
    new anchor.BN(epoch),
    expectedClaimCount,
    streamerKey
  )
  .accounts({ /* ... */ })
  .rpc();
```

---

## Channel Management

### Channel State Structure

```rust
#[account]
pub struct ChannelState {
    pub channel_id: String,       // e.g., "twitch:xqc"
    pub current_epoch: u64,
    pub merkle_root: [u8; 32],
    pub total_amount: u64,
    pub total_claims: u64,
    pub ring_buffer: [u8; 32],    // Circular claim history
    pub sealed: bool,
    pub bump: u8,
}
```

**PDA Derivation:**
```typescript
const [channelPda] = anchor.web3.PublicKey.findProgramAddressSync(
  [
    Buffer.from("channel-state"),
    protocolPda.toBuffer(),
    Buffer.from(channelId)
  ],
  programId
);
```

---

## Passport System

### mint_passport_open

**Description:** Mint a new passport (identity attestation).

**Authority:** Publisher only

**Parameters:**
```rust
pub fn mint_passport_open(
    ctx: Context<MintPassportOpen>,
    user_hash: [u8; 32],  // keccak256(user_id)
    owner: Pubkey,        // Wallet owner
    tier: u8,             // Verification tier (0-5)
    score: u64,           // Trust score
) -> Result<()>
```

**Passport Structure:**
```rust
#[account]
pub struct Passport {
    pub user_hash: [u8; 32],
    pub owner: Pubkey,
    pub tier: u8,             // 0=unverified, 5=KYC
    pub score: u64,
    pub epoch_count: u32,
    pub weighted_presence: u64,
    pub badges: u32,
    pub revoked: bool,
    pub issued_at: i64,
    pub updated_at: i64,
}
```

**TypeScript Example:**
```typescript
const userHash = keccak256(Buffer.from(userId));

await program.methods
  .mintPassportOpen(
    Array.from(userHash),
    ownerPubkey,
    3,  // Tier 3 verification
    new anchor.BN(1000)  // Initial score
  )
  .accounts({ /* ... */ })
  .signers([publisherKeypair])
  .rpc();
```

---

### upgrade_passport_open

**Description:** Upgrade passport tier or score.

**Authority:** Publisher only

**Parameters:**
```rust
pub fn upgrade_passport_open(
    ctx: Context<UpgradePassportOpen>,
    user_hash: [u8; 32],
    new_tier: u8,
    new_score: u64,
    epoch_count: u32,
    weighted_presence: u64,
    badges: u32,
    leaf_hash: Option<[u8; 32]>,  // Future: merkle proof validation
) -> Result<()>
```

**TypeScript Example:**
```typescript
await program.methods
  .upgradePassportOpen(
    Array.from(userHash),
    5,  // Upgrade to Tier 5 (KYC)
    new anchor.BN(5000),  // New score
    100,  // Participated in 100 epochs
    new anchor.BN(1_000_000),  // Weighted presence
    7,  // 7 badges earned
    null  // No leaf hash (not using merkle proof yet)
  )
  .accounts({ /* ... */ })
  .rpc();
```

---

### revoke_passport_open

**Description:** Revoke a passport (ban user).

**Authority:** Publisher only

**Parameters:**
```rust
pub fn revoke_passport_open(
    ctx: Context<RevokePassportOpen>,
    user_hash: [u8; 32],
) -> Result<()>
```

**Effects:**
- Sets `passport.revoked = true`
- Future claims may reject revoked users (policy-dependent)

---

## Points System

### claim_points_open

**Description:** Claim non-transferable points using merkle proof.

**Authority:** Permissionless

**Parameters:**
```rust
pub fn claim_points_open(
    ctx: Context<ClaimPointsOpen>,
    index: u32,
    amount: u64,
    id: String,
    proof: Vec<[u8; 32]>,
) -> Result<()>
```

**Points Structure:**
```rust
#[account]
pub struct PointsAccount {
    pub owner: Pubkey,
    pub balance: u64,
    pub bump: u8,
}
```

**TypeScript Example:**
```typescript
const [pointsPda] = anchor.web3.PublicKey.findProgramAddressSync(
  [Buffer.from("points"), ownerPubkey.toBuffer()],
  programId
);

await program.methods
  .claimPointsOpen(
    index,
    new anchor.BN(pointsAmount),
    userId,
    proof
  )
  .accounts({
    pointsAccount: pointsPda,
    owner: userPubkey,
    // ...
  })
  .rpc();
```

**Key Difference from Tokens:**
- ❌ Non-transferable (no token account)
- ✅ On-chain reputation/score
- ✅ Can gate features via `require_points_ge`

---

### require_points_ge

**Description:** Gate instruction on minimum points balance.

**Authority:** Anyone (composable check)

**Parameters:**
```rust
pub fn require_points_ge(
    ctx: Context<RequirePoints>,
    min: u64,
) -> Result<()>
```

**Usage Pattern:**
```typescript
// Check user has 1000+ points before proceeding
await program.methods
  .requirePointsGe(new anchor.BN(1000))
  .accounts({
    pointsAccount: userPointsPda,
  })
  .rpc();

// If succeeds, user has sufficient points
// If fails, user does not have 1000 points
```

**Composable Example:**
```rust
// In another program:
pub fn premium_claim(ctx: Context<PremiumClaim>) -> Result<()> {
    // First check points requirement
    require_points_ge(ctx.accounts.to_require_points_context(), 1000)?;

    // Then execute premium claim logic
    // ...
}
```

---

## Cleanup Instructions

### close_epoch_state

**Description:** Close old epoch state account and reclaim rent.

**Authority:** Admin only

**Parameters:**
```rust
pub fn close_epoch_state(
    ctx: Context<CloseEpochState>,
    epoch: u64,
    streamer_key: Pubkey,
) -> Result<()>
```

**TypeScript Example:**
```typescript
// Close epoch from 30 days ago
const oldEpoch = currentEpoch - 30;

await program.methods
  .closeEpochState(
    new anchor.BN(oldEpoch),
    streamerKey
  )
  .accounts({
    epochState: oldEpochPda,
    admin: adminKeypair.publicKey,
    // Rent refunded to admin
  })
  .signers([adminKeypair])
  .rpc();

console.log(`🧹 Reclaimed rent from epoch ${oldEpoch}`);
```

**Rent Savings:**
- Each EpochState = ~0.002 SOL rent
- 365 epochs/year = ~0.73 SOL saved annually

---

### close_old_epoch_state

**Description:** Close epoch state from ring buffer.

**Authority:** Anyone (if epoch expired from buffer)

**Parameters:**
```rust
pub fn close_old_epoch_state(
    ctx: Context<CloseOldEpochState>,
) -> Result<()>
```

**Validation:**
- Epoch must be outside 10-slot ring buffer window
- Prevents closing recent epochs still in use

---

## Type Definitions

### CnftReceiptProof

```rust
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct CnftReceiptProof {
    pub leaf_hash: [u8; 32],
    pub proof_nodes: Vec<[u8; 32]>,
    pub leaf_index: u32,
    pub tree_authority: Pubkey,
}
```

### FeeSplit

```rust
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy)]
pub enum FeeSplit {
    Treasury,    // 100% to treasury
    Stakers,     // 100% to stakers
    Split5050,   // 50/50 split
    Split7030,   // 70 treasury, 30 stakers
}
```

### UserClaim

```rust
#[account]
pub struct UserClaim {
    pub user: Pubkey,
    pub channel: Pubkey,
    pub epoch: u64,
    pub amount: u64,
    pub claimed_at: i64,
    pub bump: u8,
}
```

---

## Error Codes

| Code | Name | Description |
|------|------|-------------|
| `6000` | `Unauthorized` | Signer is not authorized for this operation |
| `6001` | `InvalidProof` | Merkle proof verification failed |
| `6002` | `EpochNotSealed` | Cannot claim from unsealed epoch |
| `6003` | `EpochAlreadySealed` | Cannot overwrite sealed epoch |
| `6004` | `ProtocolPaused` | Protocol is paused by admin |
| `6005` | `AlreadyClaimed` | User already claimed this epoch |
| `6006` | `InvalidAmount` | Amount doesn't match merkle leaf |
| `6007` | `ProofTooLong` | Proof exceeds 32 levels |
| `6008` | `InvalidReceipt` | cNFT receipt verification failed |
| `6009` | `InsufficientPoints` | User doesn't have required points |
| `6010` | `PassportRevoked` | User's passport has been revoked |
| `6011` | `InvalidTier` | Invalid passport tier value |
| `6012` | `RingBufferFull` | Ring buffer exhausted (should never happen) |

**Error Handling Example:**
```typescript
try {
  await program.methods.claimOpen(/* ... */).rpc();
} catch (err) {
  if (err.code === 6001) {
    console.error("Invalid merkle proof");
  } else if (err.code === 6004) {
    console.error("Protocol paused - try again later");
  } else {
    throw err;
  }
}
```

---

## Complete Workflow Example

### End-to-End Claim Flow

```typescript
import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Milo2022 } from "./milo_2022";

async function completeClaimWorkflow() {
  // 1. Setup
  const provider = anchor.AnchorProvider.env();
  const program = new Program<Milo2022>(IDL, PROGRAM_ID, provider);

  // 2. Derive accounts
  const [protocolPda] = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from("protocol-state"), MINT.toBuffer()],
    PROGRAM_ID
  );

  const [channelPda] = anchor.web3.PublicKey.findProgramAddressSync(
    [
      Buffer.from("channel-state"),
      protocolPda.toBuffer(),
      Buffer.from(CHANNEL_ID)
    ],
    PROGRAM_ID
  );

  // 3. Fetch on-chain state
  const protocol = await program.account.protocolState.fetch(protocolPda);
  const channel = await program.account.channelState.fetch(channelPda);

  console.log(`Protocol paused: ${protocol.paused}`);
  console.log(`Epoch sealed: ${channel.sealed}`);
  console.log(`Current epoch: ${channel.currentEpoch}`);

  if (protocol.paused) {
    throw new Error("Protocol is paused");
  }

  if (!channel.sealed) {
    throw new Error("Epoch not yet sealed");
  }

  // 4. Fetch merkle proof from API
  const proofResponse = await fetch(
    `https://api.twzrd.com/proof/${channel.currentEpoch}/${USER_PUBKEY}`
  );

  if (!proofResponse.ok) {
    throw new Error("No proof found for user");
  }

  const proofData = await proofResponse.json();

  // 5. Verify proof locally (optional but recommended)
  const isValid = verifyMerkleProof(
    USER_PUBKEY,
    proofData.amount,
    proofData.proof,
    channel.merkleRoot
  );

  if (!isValid) {
    throw new Error("Proof verification failed locally");
  }

  // 6. Prepare claim PDA
  const [claimPda] = anchor.web3.PublicKey.findProgramAddressSync(
    [
      Buffer.from("user-claim"),
      USER_PUBKEY.toBuffer(),
      channelPda.toBuffer(),
      Buffer.from(channel.currentEpoch.toString('le', 8))
    ],
    PROGRAM_ID
  );

  // Check if already claimed
  const existingClaim = await provider.connection.getAccountInfo(claimPda);
  if (existingClaim) {
    throw new Error("Already claimed for this epoch");
  }

  // 7. Prepare token account
  const userAta = await getAssociatedTokenAddress(
    MINT,
    USER_PUBKEY,
    false,
    TOKEN_2022_PROGRAM_ID
  );

  // 8. Build and send transaction
  const signature = await program.methods
    .claimChannelOpen(
      CHANNEL_ID,
      new anchor.BN(channel.currentEpoch),
      proofData.index,
      new anchor.BN(proofData.amount),
      proofData.id,
      proofData.proof.map(p => Array.from(Buffer.from(p, 'hex')))
    )
    .accounts({
      protocolState: protocolPda,
      channelState: channelPda,
      userClaim: claimPda,
      userTokenAccount: userAta,
      mint: MINT,
      user: USER_PUBKEY,
      tokenProgram: TOKEN_2022_PROGRAM_ID,
      systemProgram: anchor.web3.SystemProgram.programId,
    })
    .rpc();

  // 9. Confirm transaction
  await provider.connection.confirmTransaction(signature, 'confirmed');

  // 10. Verify claim
  const claimAccount = await program.account.userClaim.fetch(claimPda);

  console.log(`✅ Claimed ${claimAccount.amount} tokens`);
  console.log(`📝 Signature: ${signature}`);
  console.log(`🔗 Explorer: https://explorer.solana.com/tx/${signature}`);

  return signature;
}
```

---

## Additional Resources

- **GitHub:** https://github.com/twzrd-sol/attention-oracle
- **Solana Explorer:** https://explorer.solana.com/address/4rArjoSZKrYkoE7hkvZNBP2Wpxovr78cfkxBnNwFNPn5
- **Integration Guide:** [INTEGRATION.md](INTEGRATION.md)
- **Architecture:** [ARCHITECTURE.md](ARCHITECTURE.md)
- **Security:** [SECURITY.md](SECURITY.md)

---

*For support, open an issue on GitHub or contact dev@twzrd.com*
