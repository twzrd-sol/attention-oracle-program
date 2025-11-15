# Integration Guide - Attention Oracle

**Last Updated:** October 30, 2025
**Program ID:** `4rArjoSZKrYkoE7hkvZNBP2Wpxovr78cfkxBnNwFNPn5`
**Difficulty:** Beginner to Intermediate

---

## Table of Contents

1. [Quick Start](#quick-start)
2. [Prerequisites](#prerequisites)
3. [Installation](#installation)
4. [Fetching Merkle Proofs](#fetching-merkle-proofs)
5. [Claiming Rewards](#claiming-rewards)
6. [Account Derivation](#account-derivation)
7. [Error Handling](#error-handling)
8. [Testing](#testing)
9. [Production Considerations](#production-considerations)
10. [Complete Examples](#complete-examples)

---

## Quick Start

### 30-Second Integration

```typescript
import { claimRewards } from '@twzrd/sdk';

// 1. Fetch proof from TWZRD API
const proof = await fetch(`https://api.twzrd.com/proof/${epoch}/${user}`)
  .then(r => r.json());

// 2. Claim rewards
const signature = await claimRewards({
  channelId: "twitch:xqc",
  proof: proof.data,
  amount: proof.amount,
  wallet: walletAdapter,
});

console.log(`✅ Claimed! Signature: ${signature}`);
```

**That's it!** For full control, continue reading.

---

## Prerequisites

### Required Knowledge

- **Solana Basics** - Transactions, accounts, PDAs
- **Anchor Framework** - Program invocation, IDL usage
- **TypeScript/JavaScript** - For client-side integration
- **Merkle Trees** - Basic understanding of proof verification

### Development Environment

**Node.js:**
```bash
node --version  # v18+ required
npm --version   # v9+
```

**Solana CLI:**
```bash
solana --version  # 1.17+
solana config set --url mainnet-beta
```

**Rust (Optional - for program modification):**
```bash
rustc --version  # 1.75+
cargo --version
```

---

## Installation

### Option 1: Use Official SDK (Recommended)

```bash
npm install @twzrd/attention-oracle-sdk
```

```typescript
import { AttentionOracle } from '@twzrd/attention-oracle-sdk';

const oracle = new AttentionOracle({
  programId: "4rArjoSZKrYkoE7hkvZNBP2Wpxovr78cfkxBnNwFNPn5",
  rpcUrl: "https://api.mainnet-beta.solana.com",
});

// Claim rewards
await oracle.claim({
  channelId: "twitch:xqc",
  wallet: myWallet,
});
```

### Option 2: Direct Anchor Integration

```bash
npm install @coral-xyz/anchor @solana/web3.js
```

**Clone Program IDL:**
```bash
curl -o milo_2022.json https://raw.githubusercontent.com/twzrd-sol/attention-oracle/main/target/idl/milo_2022.json
```

**Initialize Program:**
```typescript
import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Milo2022 } from "./milo_2022";
import idl from "./milo_2022.json";

const programId = new anchor.web3.PublicKey("4rArjoSZKrYkoE7hkvZNBP2Wpxovr78cfkxBnNwFNPn5");
const program = new Program<Milo2022>(idl as Milo2022, programId, provider);
```

---

## Fetching Merkle Proofs

### TWZRD Proof API

**Endpoint:** `GET https://api.twzrd.com/proof/:epoch/:user`

**Parameters:**
- `epoch` - Epoch number (e.g., `20251030`)
- `user` - User's Solana public key

**Response:**
```json
{
  "success": true,
  "data": {
    "user": "87d5WsriU5DiGPSFvojQJH525qsHNiHCP4m2Qa19ufdy",
    "epoch": 20251030,
    "channel": "twitch:xqc",
    "amount": 1000000000,
    "proof": [
      "0xabc123...",
      "0xdef456...",
      "0x789abc..."
    ],
    "leaf_index": 42,
    "merkle_root": "0x1a2b3c..."
  }
}
```

**Error Cases:**
```json
{
  "success": false,
  "error": "NO_PROOF_FOUND",
  "message": "User did not participate in epoch 20251030"
}
```

### TypeScript Fetch Example

```typescript
interface MerkleProof {
  user: string;
  epoch: number;
  channel: string;
  amount: number;
  proof: string[];
  leaf_index: number;
  merkle_root: string;
}

async function fetchProof(
  epoch: number,
  user: anchor.web3.PublicKey
): Promise<MerkleProof | null> {
  const response = await fetch(
    `https://api.twzrd.com/proof/${epoch}/${user.toBase58()}`
  );

  const data = await response.json();

  if (!data.success) {
    console.error(`No proof found: ${data.message}`);
    return null;
  }

  return data.data;
}
```

### Local Proof Verification (Optional)

**Before claiming on-chain, verify proof locally:**

```typescript
import { keccak256 } from '@ethersproject/keccak256';

function verifyProof(
  user: anchor.web3.PublicKey,
  amount: number,
  proof: Buffer[],
  root: Buffer
): boolean {
  // Compute leaf hash
  let current = keccak256(
    Buffer.concat([user.toBuffer(), Buffer.from(amount.toString())])
  );

  // Hash up the tree
  for (const sibling of proof) {
    current = current <= sibling
      ? keccak256(Buffer.concat([current, sibling]))
      : keccak256(Buffer.concat([sibling, current]));
  }

  return current.equals(root);
}

// Usage
const isValid = verifyProof(
  userPubkey,
  proof.amount,
  proof.proof.map(p => Buffer.from(p, 'hex')),
  Buffer.from(proof.merkle_root, 'hex')
);

if (!isValid) {
  throw new Error("Proof verification failed - do not submit transaction!");
}
```

---

## Claiming Rewards

### Step-by-Step Claim Process

#### 1. Derive Required Accounts

```typescript
// Protocol State PDA
const [protocolPda] = anchor.web3.PublicKey.findProgramAddressSync(
  [Buffer.from("protocol-state")],
  programId
);

// Channel State PDA
const channelId = "twitch:xqc";
const [channelPda] = anchor.web3.PublicKey.findProgramAddressSync(
  [Buffer.from("channel-state"), Buffer.from(channelId)],
  programId
);

// User Claim PDA
const [claimPda] = anchor.web3.PublicKey.findProgramAddressSync(
  [
    Buffer.from("user-claim"),
    userPubkey.toBuffer(),
    channelPda.toBuffer()
  ],
  programId
);
```

#### 2. Fetch On-Chain State

```typescript
// Get protocol state
const protocol = await program.account.protocolState.fetch(protocolPda);

// Get channel state
const channel = await program.account.channelState.fetch(channelPda);

// Check if user already claimed
const existingClaim = await connection.getAccountInfo(claimPda);
if (existingClaim) {
  throw new Error("Already claimed for this epoch!");
}

// Verify epoch is sealed
if (!channel.sealed) {
  throw new Error("Epoch not yet sealed - cannot claim");
}

// Verify protocol is not paused
if (protocol.paused) {
  throw new Error("Protocol is paused - claims disabled");
}
```

#### 3. Prepare Token Accounts

```typescript
import { getAssociatedTokenAddress, TOKEN_2022_PROGRAM_ID } from "@solana/spl-token";

// Get or create user's token account
const mint = new anchor.web3.PublicKey(protocol.mint); // Fetch from protocol state

const userTokenAccount = await getAssociatedTokenAddress(
  mint,
  userPubkey,
  false,  // allowOwnerOffCurve
  TOKEN_2022_PROGRAM_ID
);

// Check if account exists, create if not
const accountInfo = await connection.getAccountInfo(userTokenAccount);
if (!accountInfo) {
  // Add createAssociatedTokenAccount instruction to transaction
  const createAtaIx = createAssociatedTokenAccountInstruction(
    userPubkey,  // payer
    userTokenAccount,
    userPubkey,  // owner
    mint,
    TOKEN_2022_PROGRAM_ID
  );
  // Add to transaction before claim instruction
}
```

#### 4. Build Claim Transaction

```typescript
const proofBuffers = proof.proof.map(p =>
  Array.from(Buffer.from(p.replace('0x', ''), 'hex'))
);

const tx = await program.methods
  .claimOpen(
    new anchor.BN(proof.amount),
    proofBuffers
  )
  .accounts({
    protocolState: protocolPda,
    channelState: channelPda,
    userClaim: claimPda,
    user: userPubkey,
    userTokenAccount: userTokenAccount,
    mint: mint,
    tokenProgram: TOKEN_2022_PROGRAM_ID,
    systemProgram: anchor.web3.SystemProgram.programId,
  })
  .transaction();

// Add recent blockhash
tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
tx.feePayer = userPubkey;
```

#### 5. Sign and Send Transaction

```typescript
// Sign with wallet adapter
const signedTx = await wallet.signTransaction(tx);

// Send transaction
const signature = await connection.sendRawTransaction(signedTx.serialize());

// Confirm transaction
await connection.confirmTransaction(signature, 'confirmed');

console.log(`✅ Claimed successfully! Signature: ${signature}`);
console.log(`View on explorer: https://explorer.solana.com/tx/${signature}`);
```

---

## Account Derivation

### PDA Derivation Reference

**Protocol State:**
```typescript
const [protocolPda, protocolBump] = anchor.web3.PublicKey.findProgramAddressSync(
  [Buffer.from("protocol-state")],
  programId
);
```

**Channel State:**
```typescript
const [channelPda, channelBump] = anchor.web3.PublicKey.findProgramAddressSync(
  [Buffer.from("channel-state"), Buffer.from(channelId)],
  programId
);
```

**User Claim:**
```typescript
const [claimPda, claimBump] = anchor.web3.PublicKey.findProgramAddressSync(
  [
    Buffer.from("user-claim"),
    userPubkey.toBuffer(),
    channelPda.toBuffer()
  ],
  programId
);
```

### Caching Bump Seeds

**Optimization for repeated derivations:**

```typescript
class PDACache {
  private cache = new Map<string, [anchor.web3.PublicKey, number]>();

  derive(seeds: Buffer[]): [anchor.web3.PublicKey, number] {
    const key = seeds.map(s => s.toString('hex')).join(':');

    if (this.cache.has(key)) {
      return this.cache.get(key)!;
    }

    const result = anchor.web3.PublicKey.findProgramAddressSync(
      seeds,
      programId
    );

    this.cache.set(key, result);
    return result;
  }
}

// Usage
const cache = new PDACache();
const [channelPda] = cache.derive([
  Buffer.from("channel-state"),
  Buffer.from("twitch:xqc")
]);
```

---

## Error Handling

### Common Errors and Solutions

| Error Code | Anchor Error | Cause | Solution |
|------------|--------------|-------|----------|
| `6000` | `Unauthorized` | Signer is not admin/publisher | Check authority account |
| `6001` | `InvalidProof` | Merkle proof verification failed | Verify proof off-chain first |
| `6002` | `EpochNotSealed` | Trying to claim before epoch sealed | Wait for seal_epoch transaction |
| `6003` | `EpochAlreadySealed` | Publisher trying to overwrite | Check epoch status before sealing |
| `6004` | `ProtocolPaused` | Claims disabled by admin | Wait for admin to unpause |
| `6005` | `AlreadyClaimed` | User already claimed this epoch | Check UserClaim PDA exists |
| `6006` | `InvalidAmount` | Amount mismatch in proof | Use exact amount from API |
| `6007` | `ProofTooLong` | Proof exceeds 32 levels | Contact TWZRD support |

### TypeScript Error Handling

```typescript
import { AnchorError } from "@coral-xyz/anchor";

try {
  await program.methods.claimOpen(amount, proof).rpc();
} catch (error) {
  if (error instanceof AnchorError) {
    switch (error.error.errorCode.number) {
      case 6001:
        console.error("Invalid proof - verification failed");
        break;
      case 6002:
        console.error("Epoch not sealed yet - try again later");
        break;
      case 6004:
        console.error("Protocol paused - claims disabled");
        break;
      case 6005:
        console.error("Already claimed for this epoch");
        break;
      default:
        console.error(`Anchor error ${error.error.errorCode.number}: ${error.error.errorMessage}`);
    }
  } else {
    console.error("Transaction failed:", error);
  }
}
```

### Simulation Before Sending

**Always simulate transactions first:**

```typescript
const simulation = await connection.simulateTransaction(tx);

if (simulation.value.err) {
  console.error("Simulation failed:", simulation.value.err);
  console.log("Logs:", simulation.value.logs);
  throw new Error("Transaction would fail - aborting");
}

// Only send if simulation succeeds
const signature = await connection.sendRawTransaction(tx.serialize());
```

---

## Testing

### Devnet Testing

**1. Configure Devnet:**
```bash
solana config set --url devnet
solana airdrop 2  # Get test SOL
```

**2. Use Devnet Program ID:**
```typescript
const DEVNET_PROGRAM_ID = new anchor.web3.PublicKey(
  "DevnetProgramID123..."  // Get from TWZRD team
);
```

**3. Fetch Devnet Proofs:**
```typescript
const proof = await fetch(
  `https://api-dev.twzrd.com/proof/${epoch}/${user}`
).then(r => r.json());
```

### Unit Testing with Anchor

**tests/claim.spec.ts:**
```typescript
import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Milo2022 } from "../target/types/milo_2022";
import { expect } from "chai";

describe("claim_open", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.Milo2022 as Program<Milo2022>;

  it("Claims rewards with valid proof", async () => {
    // Setup: Initialize protocol, channel, seal epoch
    // ...

    // Build merkle tree
    const tree = buildMerkleTree([
      { user: user1.publicKey, amount: 1000 },
      { user: user2.publicKey, amount: 2000 },
    ]);

    const proof = tree.getProof(user1.publicKey);

    // Execute claim
    await program.methods
      .claimOpen(new anchor.BN(1000), proof)
      .accounts({ /* ... */ })
      .signers([user1])
      .rpc();

    // Verify UserClaim was created
    const claimAccount = await program.account.userClaim.fetch(claimPda);
    expect(claimAccount.amount.toNumber()).to.equal(1000);
  });

  it("Rejects invalid proof", async () => {
    const fakeProof = [[1, 2, 3, /* ... */]];

    await expect(
      program.methods
        .claimOpen(new anchor.BN(1000), fakeProof)
        .accounts({ /* ... */ })
        .rpc()
    ).to.be.rejectedWith(/InvalidProof/);
  });

  it("Prevents double-claim", async () => {
    // First claim succeeds
    await program.methods.claimOpen(amount, proof).rpc();

    // Second claim fails
    await expect(
      program.methods.claimOpen(amount, proof).rpc()
    ).to.be.rejectedWith(/AccountAlreadyExists/);
  });
});
```

---

## Production Considerations

### Rate Limiting

**API calls:**
```typescript
import pLimit from 'p-limit';

const limit = pLimit(10); // Max 10 concurrent requests

const proofs = await Promise.all(
  users.map(user =>
    limit(() => fetchProof(epoch, user))
  )
);
```

### Transaction Confirmation

**Wait for finalized confirmation in production:**

```typescript
const signature = await connection.sendRawTransaction(tx.serialize());

// Wait for finalized (not just confirmed)
await connection.confirmTransaction(signature, 'finalized');

// Verify claim account was created
const claimAccount = await program.account.userClaim.fetch(claimPda);
console.log(`Claimed ${claimAccount.amount} tokens at ${claimAccount.claimedAt}`);
```

### RPC Reliability

**Use multiple RPC endpoints:**

```typescript
const endpoints = [
  "https://api.mainnet-beta.solana.com",
  "https://solana-api.projectserum.com",
  "https://rpc.ankr.com/solana",
];

async function sendWithRetry(tx: Transaction, maxRetries = 3) {
  for (let i = 0; i < maxRetries; i++) {
    try {
      const connection = new Connection(endpoints[i % endpoints.length]);
      return await connection.sendRawTransaction(tx.serialize());
    } catch (error) {
      if (i === maxRetries - 1) throw error;
      await new Promise(r => setTimeout(r, 1000 * (i + 1))); // Exponential backoff
    }
  }
}
```

### Fee Handling

**Priority fees for busy periods:**

```typescript
import { ComputeBudgetProgram } from "@solana/web3.js";

// Add priority fee instruction
const priorityFeeIx = ComputeBudgetProgram.setComputeUnitPrice({
  microLamports: 1000,  // Adjust based on network conditions
});

tx.add(priorityFeeIx);
```

---

## Complete Examples

### Example 1: Simple Claim

```typescript
import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Milo2022 } from "./milo_2022";

async function simpleClaim(
  wallet: anchor.Wallet,
  channelId: string
): Promise<string> {
  // 1. Setup
  const provider = new anchor.AnchorProvider(
    connection,
    wallet,
    { commitment: "confirmed" }
  );
  const program = new Program<Milo2022>(idl, programId, provider);

  // 2. Fetch proof
  const proof = await fetch(
    `https://api.twzrd.com/proof/latest/${wallet.publicKey.toBase58()}`
  ).then(r => r.json());

  if (!proof.success) {
    throw new Error("No rewards available");
  }

  // 3. Derive accounts
  const [protocolPda] = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from("protocol-state")],
    programId
  );

  const [channelPda] = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from("channel-state"), Buffer.from(channelId)],
    programId
  );

  const [claimPda] = anchor.web3.PublicKey.findProgramAddressSync(
    [
      Buffer.from("user-claim"),
      wallet.publicKey.toBuffer(),
      channelPda.toBuffer()
    ],
    programId
  );

  // 4. Execute claim
  const signature = await program.methods
    .claimOpen(
      new anchor.BN(proof.data.amount),
      proof.data.proof.map(p => Array.from(Buffer.from(p, 'hex')))
    )
    .accounts({
      protocolState: protocolPda,
      channelState: channelPda,
      userClaim: claimPda,
      user: wallet.publicKey,
      // ... other accounts
    })
    .rpc();

  return signature;
}
```

### Example 2: Batch Claim for Multiple Users

```typescript
async function batchClaim(
  users: anchor.web3.PublicKey[],
  channelId: string
): Promise<string[]> {
  const signatures: string[] = [];

  for (const user of users) {
    try {
      // Fetch proof
      const proof = await fetchProof(currentEpoch, user);
      if (!proof) continue;

      // Build transaction
      const tx = await buildClaimTransaction(user, proof, channelId);

      // Send (user must sign separately)
      const signature = await sendTransactionToUser(user, tx);
      signatures.push(signature);

      console.log(`✅ ${user.toBase58()}: ${signature}`);
    } catch (error) {
      console.error(`❌ ${user.toBase58()}: ${error.message}`);
    }
  }

  return signatures;
}
```

### Example 3: React Integration

```typescript
import { useWallet } from '@solana/wallet-adapter-react';
import { useAnchorWallet } from '@solana/wallet-adapter-react';

function ClaimButton() {
  const wallet = useAnchorWallet();
  const [claiming, setClaiming] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleClaim = async () => {
    if (!wallet) {
      setError("Please connect wallet");
      return;
    }

    setClaiming(true);
    setError(null);

    try {
      const signature = await simpleClaim(wallet, "twitch:xqc");
      alert(`Claimed! Signature: ${signature}`);
    } catch (err) {
      setError(err.message);
    } finally {
      setClaiming(false);
    }
  };

  return (
    <div>
      <button onClick={handleClaim} disabled={claiming}>
        {claiming ? "Claiming..." : "Claim Rewards"}
      </button>
      {error && <p style={{ color: 'red' }}>{error}</p>}
    </div>
  );
}
```

---

## Next Steps

1. **Read API Reference** - See [API.md](API.md) for complete instruction documentation
2. **Review Architecture** - See [ARCHITECTURE.md](ARCHITECTURE.md) for deep technical dive
3. **Check Security** - See [SECURITY.md](SECURITY.md) for threat model and best practices
4. **Join Community** - Discord: coming soon post-hackathon

---

## Support

**Issues:** https://github.com/twzrd-sol/attention-oracle/issues
**Email:** dev@twzrd.com
**Documentation:** https://docs.twzrd.com

---

*Happy Building! 🚀*
