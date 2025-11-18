# Attention Oracle — Pitch Deck

**Settling Attention as On-Chain State on Solana**

A thesis-driven presentation for investors and builders. Focuses on market structure, mechanism design, Solana fit, and traction. No marketing lore; no internal process notes.

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

## Slide 7: Traction to Date

### Mainnet Metrics (Last 30 Days)

**Deployment:**
- Program: `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop` (Solana mainnet)
- Token-2022 hooks with audited transfer logic
- Live and processing claims

**Pilot Creators:**
- 3 integration partners (gaming, DeFi education)
- ~120k combined audience
- Real, organic integrations

**On-Chain Activity:**
- **15,400 verified attention claims** (30 days)
- **513 claims/day average**
- **14% bot rejection rate** (Oracle functioning)
- **<$2 total gas spent** (compression proven)
- **28% 7-day return rate** (early retention signal)

**Observations:**
- Core mechanism works at scale
- Anti-bot oracle is functional
- Infrastructure is gas-efficient
- Retention needs product iteration (expected at alpha)

---

## Slide 8: Competitive Positioning

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

**Moat (Today):**
1. **First to operationalize defensible anti-bot oracle on Solana**
2. **Integration depth and creator support**
3. **Multi-creator wallet accumulation** (switching cost grows with token count)

---

## Slide 9: Risks & Mitigations

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
- Impact: Users claim once and churn
- Mitigation: Status/access model should improve retention; product iteration Q1
- IC Checkpoint: Target 40%+ 7-day retention by month 6 (currently 28%)

---

## Slide 10: Use of Proceeds

### $2.5M Seed Allocation

| Category | % | $ | Purpose |
|----------|---|---|---------|
| Creator Integrations (BD) | 50% | $1.25M | Onboard 50-100 creators, DevRel, SDK support |
| Product & Engineering | 20% | $500k | Hook hardening, dashboard, analytics |
| Security & Audits | 15% | $375k | Program audit, oracle red-team |
| Legal & Operations | 15% | $375k | Token structure, compliance, jurisdictions |

**Burn Rate:** ~$180k/month (14-month runway)

**Milestones:**
- M1 (3mo): 25 creators, 100k claims
- M2 (6mo): 50 creators, 1M claims, 40%+ retention
- M3 (9mo): 100 creators, 10M claims, Series A clarity

---

## Slide 11: Team

### Execution Credibility

**Tech Lead**
- Ex-Solana Labs core contributor
- Optimized SPL-Token library
- Deep Rust/SVM expertise
- Shipped mainnet code

**Product Lead**
- Former Ad-Tech Engineer (The Trade Desk)
- Understands DSP and ad stack inefficiencies
- 7+ years in real-time bidding

**Mechanism Design**
- DeFi protocol designer
- Prior 2021 cycle exit
- Governance and tokenomics experience

**Team Transparency:**
- Pseudonymous publicly; doxxed to IC
- Some LPs may require public identification
- Valuation reflects anonymity trade-off

---

## Slide 12: The Ask

### Investment Terms

**Amount:** $2.5M Seed
**Runway:** 14 months to Series A or profitability inflection

**Key Checkpoints Before Close:**
1. Oracle precision/recall data (precision >95%)
2. LOIs from 5+ creators
3. Team legal/corporate finalized
4. Public vs. pseudonymous decision

**Success Metrics (Year 1):**
- 50+ integrated creators
- 1M+ on-chain claims
- 40%+ 7-day retention
- Multi-creator token accumulation driving moat

---

## Slide 13: Why Fund This

### The Investment Thesis

**Thesis Strength: 9/10**
- Attention as on-chain asset is correct
- Solana timing is perfect (not 2 years ago, not 10 years from now)
- Settlement cost is the unlock

**Execution Risk: 7/10**
- Oracle operationalized but needs red-team validation
- Creator adoption is normal seed risk (strong BD allocation mitigates)
- Pseudonymous team is VC-fundable but impacts valuation

**Category Potential: High**
- $100B+ addressable market (creator economy)
- First protocol to settle attention at scale
- Network effects compound (more creators → better data → better oracle)

**Recommendation:**
This is a **category-defining bet on the financialization of engagement**. Fund it.

---

## Appendices (For Discussion)

**Appendix A: Oracle Performance Data**
- Precision/recall on adversarial test set
- False positive/negative rates
- Attack vs. reward cost breakdown

**Appendix B: Creator Integration Playbook**
- SDK deployment checklist
- Metrics dashboard setup
- Common integration issues + fixes

**Appendix C: Token Design & Compliance**
- Token classification (utility vs. security)
- Transfer hook mechanics
- Gas subsidy model (detailed)
- Regulatory pathway

**Appendix D: 18-Month Roadmap**
- Q1: Product iteration, 25 creators
- Q2: 50 creators, multi-root epochs, mobile SDK
- Q3: Partner integrations, data graph exploration, Series A prep

---

**Questions?**
dev@twzrd.xyz | [GitHub](https://github.com/twzrd-sol/attention-oracle-program) | [Mainnet](https://solscan.io/account/GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop)

