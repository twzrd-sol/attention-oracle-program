# Attention Oracle — Open Core (Token‑2022 Program)

Brand‑neutral, production‑grade Anchor program implementing Token‑2022 claim verification and a transfer‑hook entrypoint. No secrets or third‑party API keys in this repository.

## CLS Protocol Overview
Brand‑neutral, production‑grade system for streaming viewer loyalty rewards via Solana Token‑2022. Viewers earn tokens through merkle‑proof claims. Fixed‑size ring buffer (~9.5KB per channel) ensures bounded storage.

### Key On‑Chain Structures
```rust
#[zero_copy]
#[repr(C, packed)]
pub struct ChannelSlot {
    pub epoch: u64,
    pub root: [u8; 32],
    pub claim_count: u16,
    pub claimed_bitmap: [u8; 1024],
} // 1,066 bytes

#[account(zero_copy)]
pub struct ChannelState {
    pub version: u8,
    pub bump: u8,
    pub mint: Pubkey,
    pub streamer: Pubkey,
    pub latest_epoch: u64,
    pub slots: [ChannelSlot; 9],
} // ~9.5KB total
```

Ring buffer publish logic:
```rust
let slot_idx = (epoch as usize) % 9;
channel.slots[slot_idx].reset(epoch, root);
channel.latest_epoch = epoch;
```

### Merkle Proof Verification (O(log n))
```rust
fn verify_proof(proof: &[[u8; 32]], root: [u8; 32], leaf: [u8; 32]) -> bool {
    let mut computed = leaf;
    for sibling in proof {
        computed = if computed <= *sibling { keccak256(&[computed, *sibling]) } else { keccak256(&[*sibling, computed]) };
    }
    computed == root
}
```

### Bitmap Claim Check (O(1))
```rust
let byte = (index / 8) as usize;
let bit = (index % 8) as u8;
let claimed = (slot.claimed_bitmap[byte] & (1 << bit)) != 0;
```

## Deployment (2025‑11‑07)
- Program ID: `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop`
- Publisher: set via `update_publisher_open`
- Channel init cost: ~0.04002 SOL; publish fee: ~0.000005 SOL

## Test Suite (v2 Ring Path, Token‑2022)

End‑to‑end ProgramTest coverage with real Token‑2022 mint (TransferFeeConfig) and CPIs:

- Initialize mint (open, mint‑keyed ProtocolState)
- Initialize channel_state ring
- set_merkle_root_ring (monotonic guard)
- claim_with_ring (transfer_checked from treasury to claimer)
- close_channel_state (rent recovery)

Run:
```bash
cargo test -p token-2022 --tests -- --nocapture
```

## License
MIT — see LICENSE.
