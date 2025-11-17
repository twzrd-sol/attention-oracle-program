# Wallet Binding Scaffold (Ring Claims)

Goal: bind off-chain user identities (hashed usernames or stable Twitch IDs) to on-chain wallets for ring-claim proofs that require `claimer` in the leaf.

## Table: `user_wallet_bindings`
- `user_hash` TEXT (primary key part)
- `username` TEXT (optional, for diagnostics)
- `wallet` TEXT (primary key part)
- `verified` BOOLEAN DEFAULT FALSE
- `source` TEXT ("manual", "oauth", etc.)
- `created_at` INTEGER (unix seconds)
- `updated_at` INTEGER (unix seconds)

Primary key: `(user_hash, wallet)`

Indexes: `user_hash`, `wallet`

## API Hooks (to wire later)
- `POST /bind-wallet` body: `{ userId?, username?, wallet, verified?, source? }`
  - server calls `db.bindWallet()`; returns latest binding for user.
- `GET /bound-wallet?userId|username` → `{ wallet?, verified }`

## Program Leaf
- Ring leaf: `keccak256( claimer || index || amount || id_bytes )`
- Use `computeClaimLeaf()` from `apps/twzrd-aggregator/src/claims.ts`.

## Canonical Hashing
- Prefer stable Twitch userId: `keccak256("twitchId:" + id)`
- Fallback to login: existing `hashUser(username)`
- Helper: `canonicalUserHash()` in `src/util/hashing.ts`

## Rollout Plan
1. Keep ring endpoints gated by `RING_CLAIMS_ENABLED=false`.
2. Launch wallet-binding endpoints; softly collect bindings.
3. When coverage is sufficient, enable ring tree building by resolving wallets per participant during leaf construction.
4. Publish ring roots on-chain; re-enable `/claim-proof` + `/claim-root`.

