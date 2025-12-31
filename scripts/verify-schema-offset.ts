
import { PublicKey } from '@solana/web3.js';

// Anchor Account Discriminator (8 bytes)
const DISCRIMINATOR_SIZE = 8;

// ChannelConfigV2 Struct Layout
// pub struct ChannelConfigV2 {
//     pub version: u8,
//     pub bump: u8,
//     pub mint: Pubkey,
//     pub subject: Pubkey,
//     pub authority: Pubkey,
//     pub latest_root_seq: u64,
//     ...
// }

const LAYOUT = [
    { name: 'discriminator', size: 8 },
    { name: 'version', size: 1 },
    { name: 'bump', size: 1 },
    { name: 'mint', size: 32 },
    { name: 'subject', size: 32 },
    { name: 'authority', size: 32 },
    { name: 'latest_root_seq', size: 8 }, // We want the offset OF this field
];

function calculateOffset(fieldName: string): number {
    let offset = 0;
    for (const field of LAYOUT) {
        if (field.name === fieldName) {
            return offset;
        }
        offset += field.size;
    }
    throw new Error(`Field ${fieldName} not found in layout`);
}

function main() {
    const targetField = 'latest_root_seq';
    const offset = calculateOffset(targetField);
    
    console.log(`--- Schema Offset Verification ---`);
    console.log(`Target Field: ${targetField}`);
    console.log(`Calculated Offset: ${offset}`);
    
    // Hardcoded check against the fix we deployed
    const EXPECTED_OFFSET = 106;
    
    if (offset === EXPECTED_OFFSET) {
        console.log(`✅ Offset verified! Matches production fix (${EXPECTED_OFFSET}).`);
        process.exit(0);
    } else {
        console.error(`❌ Offset MISMATCH! Expected ${EXPECTED_OFFSET}, got ${offset}.`);
        console.error(`Did the struct layout change? Update the aggregator immediately.`);
        process.exit(1);
    }
}

main();
