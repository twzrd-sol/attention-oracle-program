# Attention Oracle — Pitch Deck

**Settling attention as on‑chain state on Solana**

Presentation for investors and builders covering market structure, mechanism design, Solana fit, and current status.

---

## Slide 1: The Problem

### Web2 Attention is a Trapped Asset

**Current State:**
- User attention is finite and competitive
- Platforms (Google, Meta, Twitch, TikTok) capture and resell it via ads
- Creators and viewers receive near-zero direct value
- Measurements (views, watch time) are closed, non-portable, non-settled

**The Market Gap:**
- $200B+ ad-tech market globally
- 50% of value is lost to platform intermediaries
- Creators pay $20 CPM to platforms to reach audiences they own
- No on-chain settlement mechanism exists

**Why It Matters:**
Attention is the scarcest resource in the digital economy, yet the only asset class with no transparent, verifiable settlement layer.

---

## Slide 2: Market Opportunity

### Three Converging Trends

**1. Creator Economy at Scale**
- 200M+ creators worldwide; $100B+ addressable market
- Platforms extract 30-50% take rates (YouTube, TikTok, Twitch)
- Creators actively seeking alternative distribution and monetization rails

**2. Solana Infrastructure Maturity**
- SVM (Sealevel) enables atomic composability at scale
- State Compression reduces cost per event 100x
- Token-2022 hooks allow logic at token level
- Settlement cost now <$0.000005 per transaction

**3. Embedded Wallets + Standards**
- Web2 users can hold crypto without seed phrases (Privy, Magic)
- Gas abstraction is now standardized
- Account abstraction removes UX friction

**Result:** For the first time, settling individual attention events is economically rational.

---

## Slide 3: The Solution

### Headless Settlement Protocol

**Three Components:**

**1. Verification Layer (Oracle)**
- Ingests client-side telemetry: cursor movement, scroll patterns, focus/blur, session timing
- Produces "Proof of Entropy" with confidence score
- Distinguishes humans from bots before on-chain commitment

**2. Settlement Layer (Program)**
- Batches attention receipts into Merkle trees
- Commits roots to Solana via Token-2022
- Ring-buffered state for epochs/channels
- Claim bitmaps prevent double-spend

**3. UX Layer (Invisible Wallet)**
- Embedded wallet for Web2 users (no seed phrases)
- Gas abstraction via creator-funded "Gas Tank"
- Automatic subsidy for first 5 claims

**Key Design:** No new platform. Twitch, Substack, games, or any surface integrate via SDK without migrating users.

---

## Slide 4: Oracle Mechanics — Proof-of-Entropy

### How We Stop Bots

**The Risk:**
If bots can cheaply simulate watching, the economy becomes an extractable faucet.

**Our Defense:**

**Signal Collection:**
- Micro-movements (mouse position, velocity)
- Scroll velocity variance
- Interaction timing patterns
- Active tab focus depth
- Session duration distribution

**Entropy Model:**
- Generates confidence score (human vs. bot)
- Thresholds tunable as adversaries adapt
- Multi-layered signals (statistical + behavioral + hardware)

**Economic Calibration:**
- Attack cost (realistic bot farm): ~$0.004/min
- Reward value per minute: ~$0.001/min
- **Result:** Attack is 4x unprofitable at scale

**Validator Economics:**
- Oracle nodes stake; slashed for fraudulent patterns
- Honest nodes rewarded proportional to volume + quality

**Key Metric:** Oracle precision >95% on red-team testing (required before seed close).

---

## Slide 5: Unit Economics

### Why Creators Integrate + Protocol Revenue

**Creator Math (Status Quo vs. Attention Oracle):**

| Metric | Web2 | Attention Oracle |
|--------|------|------------------|
| CPM paid | $20 | $10 |
| Platform take | 50% | 0% |
| Effective cost | $20 | $10 |
| User receives | $0 | Value + proof |
| **Creator savings** | — | **50%** |

**Protocol Revenue ("Tribute"):**
- 20 basis points on value flowing creator → viewer
- Example: Creator funds $100 → 99.8 units to viewers, 0.2 to protocol

**Gas & Wallet Economics:**

| Item | Cost | Solution |
|------|------|----------|
| Embedded wallet | $0.50/user/month | Creator pre-funds |
| Gas subsidy (claims 1-5) | $2.50/user | Amortized over retention |
| After 5 claims | Self-sustaining | Tokens pay via hooks |

**Worked Example (50k follower creator):**
- Budget: $5k/month attention distribution
- Users subsidized: 2,000 (at $2.50 each)
- Payback: 40% return in week 2 → subsidy amortized
- Protocol take: ~$10/month (0.2%)
- Creator CAC: $2.50 → $0.25 if >10x ROI over 90 days

---

## Slide 6: User Value — Status, Access, Composability

### Why Users Care (Beyond Income)

**The Problem with "Click-to-Earn":**
- Pure yield ($0.05/hour) doesn't move the needle
- This was BAT's failure mode

**The Solution: Status & Access**

Tokens function as **Proof-of-Fandom** unlocking:
- **Community Access:** Gated Discord channels, exclusive streams
- **Future Drops:** Early access to NFTs, merch, gated content
- **Governance:** Votes in creator DAOs, proposal weight
- **Multi-Creator Loyalty:** Tokens work across creators (switching cost)

**Why This Works:**
- Same mechanic powering Fortnite cosmetics ($billions/year)
- Perceived value exceeds nominal yield when tied to status
- Speculative floor created by genuine utility

---

## Slide 7: Competitive Positioning

### Why We Win

**vs. Web2 Platforms (Google/Meta/Twitch):**
- They own distribution but are extractive
- We are a settlement layer, not a replacement
- Creators adopt without leaving existing platforms

**vs. Earlier Web3 Attempts (BAT/Rally/Roll):**

| Factor | BAT | Rally/Roll | Attention Oracle |
|--------|-----|-----------|------------------|
| Settlement Cost | $5+ | $2+ | $0.0005 |
| UX Friction | Browser migration | New wallet | Embedded, gasless |
| Composability | Closed | Platform-specific | Headless SDK |
| Anti-Bot | Weak | Weak | Entropy-based |
| Status/Access | No | Weak | Native |

**vs. Social Tokens (Farcaster/Lens/Friend.tech):**
- They focus on speculation or identity
- We focus on engagement settlement (fundamental primitive)
- Speculation can layer on top

**Moat (Design Goals):**
1. Operational, economically defensible anti-bot oracle on Solana.
2. Deep integrations with creators and existing distribution surfaces.
3. Multi-creator wallet accumulation increasing switching costs over time.

These are targets, not current claims; they should only be presented as achieved once supported by data.

---

## Slide 8: Risks & Mitigations

### Key Execution Risks

**Risk 1: Oracle Robustness**
- Impact: Bots get through → economy breaks
- Mitigation: Red-team testing, slashing mechanism, continuous updates
- IC Checkpoint: Precision/recall data required before close

**Risk 2: Creator Adoption**
- Impact: No creators → no viewers → no settlement
- Mitigation: 50% of seed to BD, target 50-100 creators in year 1
- IC Checkpoint: LOIs from 10+ creators

**Risk 3: Regulatory Clarity**
- Impact: "Attention rewards" classified as financial instruments
- Mitigation: Structured as utility tokens for content access; legal review in progress
- IC Checkpoint: Legal opinion on token classification (US-first)

**Risk 4: Retention**
- Impact: Users claim once and churn.
- Mitigation: Design focuses on status/access and cross‑creator utility; iteration will be driven by observed cohorts.

**Questions?**
Contact: dev@twzrd.xyz
