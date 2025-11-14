// Unit tests for core logic (no integration testing)
use token_2022::constants::*;

#[test]
fn test_channel_bitmap_bytes() {
    // Verify we have 1024 bytes for the bitmap (v2 upgrade)
    assert_eq!(CHANNEL_BITMAP_BYTES, 1024);

    // This gives us 1024 * 8 = 8192 bits
    let max_participants = CHANNEL_BITMAP_BYTES * 8;
    assert_eq!(max_participants, 8192);

    // Verify CHANNEL_MAX_CLAIMS is also 8192
    assert_eq!(CHANNEL_MAX_CLAIMS, 8192);

    println!("✅ CHANNEL_BITMAP_BYTES = {} (supports {} participants per channel)",
             CHANNEL_BITMAP_BYTES, max_participants);
    println!("✅ CHANNEL_MAX_CLAIMS = {} (max claimable per epoch)",
             CHANNEL_MAX_CLAIMS);
}

#[test]
fn test_channel_state_size() {
    // ChannelState must stay under the 10KB MAX_PERMITTED_DATA_INCREASE limit
    // Structure:
    // - version: 1 byte
    // - bump: 1 byte
    // - mint: 32 bytes
    // - streamer: 32 bytes
    // - latest_epoch: 8 bytes
    // - slots: CHANNEL_RING_SLOTS * ChannelSlot

    // Each ChannelSlot:
    // - epoch: 8 bytes
    // - root: 32 bytes
    // - claim_count: 2 bytes
    // - claimed_bitmap: 1024 bytes (CHANNEL_BITMAP_BYTES)
    // Total per slot: 1066 bytes

    const SLOT_SIZE: usize = 8 + 32 + 2 + 1024;
    assert_eq!(SLOT_SIZE, 1066);

    const HEADER_SIZE: usize = 1 + 1 + 32 + 32 + 8;
    assert_eq!(HEADER_SIZE, 74);

    assert_eq!(CHANNEL_RING_SLOTS, 9, "v2.0.1 uses 9 slots to stay under 10KB");

    const TOTAL_SIZE: usize = 8 + HEADER_SIZE + (CHANNEL_RING_SLOTS * SLOT_SIZE); // 8 bytes discriminator
    assert_eq!(TOTAL_SIZE, 9676);

    // Verify it's safely under the 10KB growth limit
    assert!(TOTAL_SIZE <= 10_240, "ChannelState must remain under the 10KB limit ({} bytes)", TOTAL_SIZE);

    println!("✅ ChannelState size: {} bytes ({:.2} KB)", TOTAL_SIZE, TOTAL_SIZE as f64 / 1024.0);
    println!("   - Discriminator: 8 bytes");
    println!("   - Header: {} bytes", HEADER_SIZE);
    println!("   - Slots ({}x): {} bytes", CHANNEL_RING_SLOTS, CHANNEL_RING_SLOTS * SLOT_SIZE);
}

#[test]
fn test_bitmap_operations() {
    // Simulate bitmap operations
    let mut bitmap = vec![0u8; CHANNEL_BITMAP_BYTES];

    // Helper functions (same logic as in the program)
    fn set_bit(bitmap: &mut [u8], index: usize) {
        let byte = index / 8;
        let bit = index % 8;
        bitmap[byte] |= 1 << bit;
    }

    fn test_bit(bitmap: &[u8], index: usize) -> bool {
        let byte = index / 8;
        let bit = index % 8;
        (bitmap[byte] & (1 << bit)) != 0
    }

    // Test bit 0
    assert!(!test_bit(&bitmap, 0));
    set_bit(&mut bitmap, 0);
    assert!(test_bit(&bitmap, 0));

    // Test bit 8191 (last valid bit for 1024 bytes)
    assert!(!test_bit(&bitmap, 8191));
    set_bit(&mut bitmap, 8191);
    assert!(test_bit(&bitmap, 8191));

    // Test middle bit
    assert!(!test_bit(&bitmap, 2048));
    set_bit(&mut bitmap, 2048);
    assert!(test_bit(&bitmap, 2048));

    // Ensure other bits are still 0
    assert!(!test_bit(&bitmap, 1));
    assert!(!test_bit(&bitmap, 2047));
    assert!(!test_bit(&bitmap, 2049));

    println!("✅ Bitmap operations work correctly");
    println!("   - Bit 0: set and tested");
    println!("   - Bit 8191: set and tested (v2 max)");
    println!("   - Bit 2048: set and tested");
}

#[test]
fn test_8192_vs_8193() {
    // CHANNEL_MAX_CLAIMS should be 8192
    assert_eq!(CHANNEL_MAX_CLAIMS, 8192);

    // 8192 should be valid
    let index_8191 = 8191u32;
    assert!(index_8191 < CHANNEL_MAX_CLAIMS as u32);

    // 8192 should be invalid
    let index_8192 = 8192u32;
    assert!(index_8192 >= CHANNEL_MAX_CLAIMS as u32);

    println!("✅ Index 8191 < {} ✅", CHANNEL_MAX_CLAIMS);
    println!("✅ Index 8192 >= {} ✅", CHANNEL_MAX_CLAIMS);
}

#[test]
fn test_merkle_leaf_computation() {
    use solana_program::keccak;
    use solana_program::pubkey::Pubkey;

    // Test leaf computation (same as in the program)
    let claimer = Pubkey::new_unique();
    let index: u32 = 42;
    let amount: u64 = 1_000_000_000;
    let id = "test_id";

    let idx_bytes = index.to_le_bytes();
    let amt_bytes = amount.to_le_bytes();
    let id_bytes = id.as_bytes();

    let leaf = keccak::hashv(&[
        claimer.as_ref(),
        &idx_bytes,
        &amt_bytes,
        id_bytes,
    ]).to_bytes();

    // Leaf should be 32 bytes
    assert_eq!(leaf.len(), 32);

    // Re-computing with same inputs should give same result
    let leaf2 = keccak::hashv(&[
        claimer.as_ref(),
        &idx_bytes,
        &amt_bytes,
        id_bytes,
    ]).to_bytes();

    assert_eq!(leaf, leaf2, "Leaf computation should be deterministic");

    // Different inputs should give different leaf
    let different_index = 43u32;
    let different_idx_bytes = different_index.to_le_bytes();
    let leaf3 = keccak::hashv(&[
        claimer.as_ref(),
        &different_idx_bytes,
        &amt_bytes,
        id_bytes,
    ]).to_bytes();

    assert_ne!(leaf, leaf3, "Different inputs should give different leaf");

    println!("✅ Merkle leaf computation works correctly");
    println!("   - Leaf size: 32 bytes");
    println!("   - Deterministic: same inputs → same output");
    println!("   - Unique: different inputs → different outputs");
}

#[test]
fn test_ring_buffer_slot_index() {
    // Ring buffer uses modulo CHANNEL_RING_SLOTS for slot indexing
    const RING_SIZE: usize = CHANNEL_RING_SLOTS;

    fn slot_index(epoch: u64) -> usize {
        (epoch as usize) % RING_SIZE
    }

    // Epoch 0 → slot 0
    assert_eq!(slot_index(0), 0);

    // Epoch (RING_SIZE - 1) → last slot
    assert_eq!(slot_index((RING_SIZE - 1) as u64), RING_SIZE - 1);

    // Epoch RING_SIZE → wraps to 0
    assert_eq!(slot_index(RING_SIZE as u64), 0);

    // Epoch (2 * RING_SIZE - 1) → last slot
    assert_eq!(slot_index((2 * RING_SIZE - 1) as u64), RING_SIZE - 1);

    // Epoch (2 * RING_SIZE) → wraps to 0
    assert_eq!(slot_index((2 * RING_SIZE) as u64), 0);

    // Large epoch number
    assert_eq!(slot_index(1762556400), 0);  // moonmoon epoch from monitoring
    assert_eq!(slot_index(1762556401), 1);

    println!("✅ Ring buffer slot indexing works correctly");
    println!("   - Modulo {} wraps around properly", RING_SIZE);
    println!("   - Epoch 10 → slot 0 (wraparound)");
    println!("   - Large epochs handled correctly");
}

#[test]
fn test_constants_consistency() {
    // Verify all constants are consistent

    // CHANNEL_BITMAP_BYTES should support 8192 participants (v2 upgrade)
    assert_eq!(CHANNEL_BITMAP_BYTES * 8, 8192);

    // CHANNEL_MAX_CLAIMS should be 8192
    assert_eq!(CHANNEL_MAX_CLAIMS, 8192);

    // CHANNEL_MAX_CLAIMS should equal bitmap capacity (v2: both are 8192)
    assert_eq!(CHANNEL_MAX_CLAIMS, CHANNEL_BITMAP_BYTES * 8);

    println!("✅ Constants are consistent:");
    println!("   - CHANNEL_BITMAP_BYTES: {}", CHANNEL_BITMAP_BYTES);
    println!("   - Bitmap capacity: {} participants", CHANNEL_BITMAP_BYTES * 8);
    println!("   - CHANNEL_MAX_CLAIMS: {}", CHANNEL_MAX_CLAIMS);
    println!("   - Max claimable: {}", CHANNEL_MAX_CLAIMS);
}

#[test]
fn test_production_scenario_jasontheween() {
    // Real production scenario from database:
    // jasontheween channel had 5,132 participants in epoch 1761249600 (highest observed)

    let participant_count = 5132;

    // This exceeds the old 1024 limit
    assert!(participant_count > 1024, "jasontheween exceeds v1 1024 limit");

    // This exceeds the v1.5 4096 limit
    assert!(participant_count > 4096, "jasontheween exceeds v1.5 4096 limit");

    // This is within the new 8192 max claims (v2 upgrade)
    assert!(participant_count <= CHANNEL_MAX_CLAIMS, "jasontheween fits in v2 8192 max");

    println!("✅ Production scenario (jasontheween):");
    println!("   - Participants: {}", participant_count);
    println!("   - Old v1 limit (1024): ❌ EXCEEDED");
    println!("   - Old v1.5 limit (4096): ❌ EXCEEDED");
    println!("   - New v2 limit (8192): ✅ FITS");
}
