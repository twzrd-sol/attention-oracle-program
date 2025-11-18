# Treasury & Creator Pool PDA Derivation Guide

## Overview

The Attention Oracle program maintains two separate token accounts for each initialized mint:
1. **Treasury**: Collects protocol fees
2. **Creator Pool**: Distributes fees to creators

Both are **Program-Derived Addresses (PDAs)**, not Associated Token Accounts (ATAs).

---

## PDA Derivation Formula

### Treasury PDA
```
Seeds: [b"treasury", mint_pubkey]
Program: GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop

Pseudocode:
address, bump = find_program_address(
    seeds = ["treasury".bytes, mint_pubkey.bytes],
    program_id = GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop
)
```

### Creator Pool PDA
```
Seeds: [b"creator_pool", mint_pubkey]
Program: GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop

Pseudocode:
address, bump = find_program_address(
    seeds = ["creator_pool".bytes, mint_pubkey.bytes],
    program_id = GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop
)
```

---

## Implementation in Different Languages

### Rust (Anchor)
```rust
use anchor_lang::prelude::*;

declare_id!("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop");

pub fn derive_treasury(mint: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[b"treasury", mint.as_ref()],
        &crate::ID,
    )
}

pub fn derive_creator_pool(mint: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[b"creator_pool", mint.as_ref()],
        &crate::ID,
    )
}
```

### TypeScript/JavaScript (with Solana.js)
```typescript
import { PublicKey } from "@solana/web3.js";

const PROGRAM_ID = new PublicKey(
    "GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop"
);

async function deriveTreasury(mint: PublicKey): Promise<[PublicKey, number]> {
    return PublicKey.findProgramAddressSync(
        [Buffer.from("treasury"), mint.toBuffer()],
        PROGRAM_ID
    );
}

async function deriveCreatorPool(mint: PublicKey): Promise<[PublicKey, number]> {
    return PublicKey.findProgramAddressSync(
        [Buffer.from("creator_pool"), mint.toBuffer()],
        PROGRAM_ID
    );
}

// Usage
const mint = new PublicKey("...");
const [treasury, treasuryBump] = await deriveTreasury(mint);
const [creatorPool, creatorPoolBump] = await deriveCreatorPool(mint);
```

### Python (with Solders)
```python
from solders.pubkey import Pubkey

PROGRAM_ID = Pubkey.from_string(
    "GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop"
)

def derive_treasury(mint: Pubkey) -> tuple[Pubkey, int]:
    return Pubkey.find_program_address(
        seeds=[b"treasury", bytes(mint)],
        program_id=PROGRAM_ID
    )

def derive_creator_pool(mint: Pubkey) -> tuple[Pubkey, int]:
    return Pubkey.find_program_address(
        seeds=[b"creator_pool", bytes(mint)],
        program_id=PROGRAM_ID
    )

# Usage
mint = Pubkey.from_string("...")
treasury, treasury_bump = derive_treasury(mint)
creator_pool, creator_pool_bump = derive_creator_pool(mint)
```

---

## Example Derivation

### Given Mint
```
Mint Pubkey: 4zMMC9srt5Ri5X14Gr934XvzzrKZtUS6G3wWZ27G8P8
```

### Derived Addresses
```
Treasury:     HYgDu3DesMHLKneb8qaPxMbNNiZpmQwjpX3W8xU2R6gM (bump: 250)
Creator Pool: FuvfS65VRfacz4ERFs2GaZV5eHqzn1c8MKhQpG88oRtp (bump: 249)
```

✅ **Distinct addresses** — No collision!

---

## On-Chain Verification

### Using Solana CLI
```bash
# Verify treasury exists
solana account HYgDu3DesMHLKneb8qaPxMbNNiZpmQwjpX3W8xU2R6gM --url mainnet-beta

# Verify creator pool exists
solana account FuvfS65VRfacz4ERFs2GaZV5eHqzn1c8MKhQpG88oRtp --url mainnet-beta
```

### Using Solscan
- Treasury: https://solscan.io/account/HYgDu3DesMHLKneb8qaPxMbNNiZpmQwjpX3W8xU2R6gM
- Creator Pool: https://solscan.io/account/FuvfS65VRfacz4ERFs2GaZV5eHqzn1c8MKhQpG88oRtp

---

## Why PDAs Instead of ATAs?

| Aspect | ATA | PDA |
|--------|-----|-----|
| **Derivation** | Owner + Mint + Token Program | Custom seeds + Program ID |
| **Control** | Owner's wallet authority | Program authority (PDA signer) |
| **Multiple per mint** | ❌ One per owner | ✅ Multiple via distinct seeds |
| **Use case** | User token accounts | Program-owned vaults |
| **Treasury support** | ❌ Not ideal | ✅ Perfect |

In this case:
- ❌ ATAs would create a collision (both treasury & creator_pool tried to use same owner + mint)
- ✅ PDAs allow distinct seeds, eliminating collisions

---

## Constants in Code

Located in `/clean-hackathon/verify-snapshot/token-2022/src/constants.rs`:

```rust
pub const PROTOCOL_SEED: &[u8] = b"protocol";
pub const TREASURY_SEED: &[u8] = b"treasury";           // ← Treasury PDA seed
pub const CREATOR_POOL_SEED: &[u8] = b"creator_pool";  // ← Creator Pool PDA seed
pub const EPOCH_STATE_SEED: &[u8] = b"epoch_state";
pub const LIQUIDITY_ENGINE_SEED: &[u8] = b"liquidity_engine";
pub const CHANNEL_STATE_SEED: &[u8] = b"channel_state";
```

---

## Fee Flow with Proper PDAs

```
1. User transfers tokens
   └─→ Transfer hook observes
       └─→ Withholds fees per Token-2022 extension

2. Keeper invokes harvest_fees()
   └─→ Treasury PDA receives 50% of withheld amount
   └─→ Creator Pool PDA receives 50% of withheld amount
       └─→ Both controlled by protocol_state PDA authority

3. Creator retrieves fees from Creator Pool PDA
   └─→ Via governance instruction or keeper distribution
```

---

## Common Pitfalls

❌ **Don't** try to use ATAs for both accounts — they'll collide
❌ **Don't** use the same seed for both PDAs — they'll collide
❌ **Don't** forget the mint key in the seed — PDAs won't be unique per mint
✅ **Do** use distinct seeds (`b"treasury"` vs `b"creator_pool"`)
✅ **Do** include the mint in the seed for uniqueness
✅ **Do** verify PDA addresses match after initialization

---

## Testing the Derivation

### Unit Test (Rust)
```rust
#[test]
fn test_pda_derivation() {
    let mint = Pubkey::new_unique();
    let (treasury, _) = Pubkey::find_program_address(
        &[b"treasury", mint.as_ref()],
        &crate::ID,
    );
    let (creator_pool, _) = Pubkey::find_program_address(
        &[b"creator_pool", mint.as_ref()],
        &crate::ID,
    );

    // ✅ Must be different
    assert_ne!(treasury, creator_pool);
}
```

### Integration Test (TypeScript)
```typescript
const mint = new PublicKey("4zMMC9srt5Ri5X14Gr934XvzzrKZtUS6G3wWZ27G8P8");
const [treasury] = PublicKey.findProgramAddressSync(
    [Buffer.from("treasury"), mint.toBuffer()],
    PROGRAM_ID
);
const [creatorPool] = PublicKey.findProgramAddressSync(
    [Buffer.from("creator_pool"), mint.toBuffer()],
    PROGRAM_ID
);

console.assert(
    !treasury.equals(creatorPool),
    "Treasury and Creator Pool must be distinct!"
);
```

---

## References

- **Solana Docs**: https://docs.solana.com/developing/programming-model/calling-between-programs#program-derived-addresses
- **Anchor Book**: https://book.anchor-lang.com/
- **Program Source**: https://github.com/twzrd-sol/attention-oracle-program

---

**Last Updated**: November 18, 2025
**Version**: 1.0
**Status**: ✅ Production
