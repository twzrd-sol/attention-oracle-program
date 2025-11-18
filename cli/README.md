# Attention Oracle CLI

Command-line interface for Attention Oracle admin operations and utilities.

## Installation

```bash
npm install -g @attention-oracle/cli
```

Or use directly with `npx`:

```bash
npx @attention-oracle/cli info
```

## Commands

### Program Info

```bash
# Show program information
ao info

# Use custom RPC
ao info --url https://api.mainnet-beta.solana.com
```

### Passport Management

```bash
# Check passport tier for a wallet
ao passport <WALLET_ADDRESS>

# Example
ao passport 2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD
```

Output:
```
🎫 Passport
══════════════════════════════════════════════════
Wallet: 2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD
Tier: Elite
PDA: J4...xyz
```

### Fee Harvesting

```bash
# Harvest withheld fees (requires admin authority)
ao harvest

# Custom mint
ao harvest --mint AAHd7u22jCMgmbF7ATkiY3BhkifD4MN3Vbsy4eYQGWN5
```

### Merkle Proof Verification

```bash
# Verify a merkle proof (off-chain)
ao verify-proof --proof proof.json
```

Proof JSON format:
```json
{
  "index": 42,
  "amount": 1000000000,
  "id": "claim_001",
  "proof": ["0xabc...", "0xdef..."],
  "epochIndex": 12345
}
```

### Claim Receipts

```bash
# Export claim receipts for a channel
ao receipts kaicenat --output receipts.json

# Specific epoch
ao receipts kaicenat --epoch 12345
```

### PDA Derivation

```bash
# Derive treasury PDA
ao pda --type treasury

# Derive passport PDA
ao pda --type passport --user <WALLET>

# Derive channel PDA
ao pda --type channel --channel kaicenat

# Derive epoch PDA
ao pda --type epoch --channel kaicenat --epoch 12345
```

## Global Options

```bash
-u, --url <url>         RPC URL (default: mainnet)
-k, --keypair <path>    Path to keypair (default: ~/.config/solana/id.json)
```

## Development

```bash
# Install dependencies
npm install

# Build
npm run build

# Link locally for testing
npm link

# Now available as `ao` command
ao --help
```

## Examples

### Check Multiple Passports

```bash
#!/bin/bash
for wallet in $(cat wallets.txt); do
  ao passport $wallet
done
```

### Monitor Fee Harvesting

```bash
# Run hourly via cron
0 * * * * ao harvest --mint AAHd7u22jCMgmbF7ATkiY3BhkifD4MN3Vbsy4eYQGWN5
```

### Batch PDA Derivation

```bash
# Derive PDAs for all channels
for channel in kaicenat lacy adapt silky; do
  ao pda --type channel --channel $channel
done
```

## License

Dual MIT/Apache-2.0

## Links

- **Program**: https://github.com/twzrd-sol/attention-oracle-program
- **SDK**: https://www.npmjs.com/package/@attention-oracle/sdk
