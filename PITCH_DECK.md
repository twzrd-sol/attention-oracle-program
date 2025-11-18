# Attention Oracle — Overview Deck

Structured outline for a public slide deck. Focused on problem, mechanism, Solana fit, and current status. No marketing lore; no internal process notes.

---

## Slide 1 — Title

- **Title:** Attention Oracle
- **Subtitle:** On‑chain settlement layer for creator attention
- **Callout:** Built on Solana (Token‑2022, transfer hooks)

---

## Slide 2 — Problem

- Digital attention is valuable but trapped inside Web2 platforms.
- Creators rent access to their own audiences via opaque algorithms and ad rails.
- Engagement metrics (views, watch time) are not portable and not settled on‑chain.

---

## Slide 3 — Solution

- Headless protocol that turns verified attention into on‑chain state.
- Creators fund treasuries; viewers claim via cryptographic proofs of engagement.
- Works alongside existing platforms via SDK; no migration required.

---

## Slide 4 — Architecture

- **Verification layer (Oracle):** aggregates client‑side signals into attention receipts.
- **Settlement layer (Program):**
  - Solana program using Token‑2022 extensions and transfer hooks.
  - Merkle trees + ring‑buffer state for epochs and channels.
  - Claim bitmaps to prevent double‑spend.
- **Wallet / UX layer:** embedded wallets and gas abstraction for Web2 users.

Diagram suggestion: left (clients) → middle (Oracle) → right (Solana program + treasuries + viewer wallets).

---

## Slide 5 — Oracle Mechanics

- Goal: make it economically irrational to simulate attention with bots.
- Uses multiple signals: interaction entropy, focus patterns, session timing.
- Batches and scores events; only high‑confidence receipts are committed.
- Design exposes clear parameters: precision, false positives/negatives, and red‑team results (to be added as data matures).

---

## Slide 6 — Economics

- Creators allocate a budget for attention rewards instead of paying platforms.
- Viewers receive tokens that represent both value and proof of engagement.
- Protocol charges a low basis‑point fee on flows from creator treasuries to viewers.
- Wallet and gas costs handled via creator‑funded "gas tanks" with explicit caps.

Note: pair this slide with a simple worked example when concrete numbers are available.

---

## Slide 7 — User Value

- Tokens function as **proofs of participation**, not only as payouts.
- Can gate:
  - Access to gated communities or channels.
  - Eligibility for drops or events.
  - Governance weight in creator‑run spaces.
- Aligns incentives for creators and viewers without requiring new platforms.

---

## Slide 8 — Why Solana

- High throughput and low fees enable fine‑grained settlement of attention events.
- State compression keeps on‑chain footprint minimal while tracking many claims.
- Token‑2022 transfer hooks allow protocol logic at the token level (fees, routing, gating).

---

## Slide 9 — Status & Roadmap

- **Today:**
  - Program deployed on Solana mainnet (Token‑2022).
  - SDKs and CLI for creators and integrators.
- **Next:**
  - Harden Oracle implementation with adversarial testing.
  - Expand creator integrations.
  - Additional tooling for analytics and reporting.

This slide should be updated with live metrics (claims, creators, retention) before external use.

---

## Slide 10 — How to Integrate

- Drop‑in TypeScript SDK for web and Node.
- CLI tools for admin operations (treasuries, parameters, verification helpers).
- Simple flow:
  1. Creator configures campaign (budget, token, rules).
  2. Frontend integrates SDK and attention tracking.
  3. Viewers claim on‑chain via Merkle proofs.

Link to SDK and CLI READMEs for detailed examples.

