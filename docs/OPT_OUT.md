# Opt-Out Guide

## Overview

The TWZRD/MILO/CLS protocol respects user privacy and autonomy. If you do not want your Twitch chat participation tracked for reward eligibility, you can opt out at any time.

**Key Points:**
- ✅ Opt-out is **immediate and permanent**
- ✅ No personal data is stored (only pseudonymous hashes)
- ✅ You can check your opt-out status anytime
- ⚠️ Past epochs remain sealed (cryptographic integrity), but you won't be eligible for **future** rewards

---

## For Viewers

### How to Opt Out

**Method 1: API Request (Recommended)**

```bash
curl -X POST https://api.twzrd.xyz/opt-out \
  -H "Content-Type: application/json" \
  -d '{"username": "your_twitch_username", "reason": "Optional reason"}'
```

**Response:**
```json
{
  "success": true,
  "message": "Opt-out request recorded. Your data will not be collected going forward.",
  "username": "your_twitch_username",
  "effective_immediately": true
}
```

**Method 2: Contact Form** (Coming Soon)
- Visit https://twzrd.xyz/opt-out
- Submit your Twitch username
- Receive confirmation email

### Check Your Status

```bash
curl "https://api.twzrd.xyz/opt-out/status?username=your_twitch_username"
```

**Response:**
```json
{
  "suppressed": true,
  "requested_at": 1761870003
}
```

### What Happens After Opt-Out?

1. **Immediate Effect:** Your participation in any tracked channel will no longer be recorded starting from the next epoch (within 60 minutes).

2. **No Future Eligibility:** You will not appear in merkle trees for future epochs, meaning you cannot claim rewards for activity after your opt-out timestamp.

3. **Past Data:** Historical epochs remain sealed for cryptographic integrity. However, you will not be able to claim tokens from those epochs either (suppression is enforced at claim time).

4. **Reversible:** Contact the team if you wish to opt back in. We can remove your suppression entry, but eligibility resumes from that point forward only.

---

## For Streamers

### How Opt-Out Affects Your Community

When a viewer opts out:
- They **disappear** from your participation counts for future epochs
- Their engagement signals (subs, bits, raids) are **not recorded**
- Your merkle root proofs will **exclude** them going forward
- Past drops remain valid, but they cannot claim

### Moderator Guidance

If viewers ask about opt-out:
1. Direct them to https://twzrd.xyz/opt-out or this documentation
2. Clarify that opt-out is **immediate** and affects **future** drops only
3. Explain that pseudonymous hashing means we never stored their real identity—only a hash of their Twitch username

### Streamer Opt-Out

If you want your **entire channel** removed from the protocol:
- Contact the team at support@twzrd.xyz or via Discord
- Provide your Twitch channel name and reason
- We will add your channel to the permanent blocklist within 24 hours

**Note:** Streamer opt-out is more complex because it affects both MILO (if you're a FAZE streamer) and CLS (if you're in the crypto category). We'll work with you to ensure clean removal.

---

## Technical Details

### Data Storage

**What We Store:**
- `user_hash`: SHA3-256 hash of your lowercase Twitch username (pseudonymous)
- `username`: Your Twitch username (for lookup/verification)
- `requested_at`: Unix timestamp of opt-out request
- `reason`: Optional reason you provided
- `ip_hash`: Partial hash of your IP address (first 16 chars, for audit trail)

**What We Don't Store:**
- Email addresses
- Personal information
- Chat message contents (only presence/signals)

### Enforcement

Opt-out is enforced at **ingestion time** in the aggregator:
1. Worker sends participation events to `/ingest` endpoint
2. Aggregator checks `suppression_list` table for each user_hash
3. If suppressed, event is **dropped** before database write
4. Merkle trees built from non-suppressed participants only

### Cryptographic Integrity

Past epochs cannot be retroactively altered because:
- Merkle roots are **sealed** and published on-chain (Solana)
- Changing a single participant would invalidate the entire tree
- On-chain roots are immutable (program state)

**However**, suppressed users **cannot claim** from past epochs because the claim endpoint also checks suppression status before generating proofs.

---

## Privacy & Compliance

### GDPR / CCPA

The protocol operates in a **pseudonymous** mode:
- User identities are hashed immediately upon ingestion
- We store Twitch usernames for opt-out lookup, but not for reward distribution
- Opt-out constitutes a "right to be forgotten" request under GDPR Article 17

### Audit Trail

All opt-out requests are logged in `suppression_log` with:
- Timestamp
- Username
- Action (opted_out / opted_in)
- IP hash (privacy-preserving)

This ensures accountability and allows us to provide proof of compliance if requested by regulators.

---

## FAQ

**Q: Can I opt back in?**
A: Yes, contact the team. We'll remove your entry from the suppression list, and you'll be eligible for future epochs.

**Q: Will I lose past rewards?**
A: If you already claimed tokens from past epochs, those remain yours. However, unclaimed epochs become ineligible once you opt out.

**Q: How long does opt-out take?**
A: Immediate (within the current epoch, max 60 minutes).

**Q: Do streamers know I opted out?**
A: No. Opt-out is private. Streamers only see aggregate participation counts decrease.

**Q: Can I opt out of just CLS but stay in MILO?**
A: Not currently. Opt-out applies to all protocol layers (TWZRD L1, MILO L2, CLS L2). Contact us if you need granular control.

---

## Contact

- **Email:** support@twzrd.xyz
- **Discord:** https://discord.gg/twzrd (coming soon)
- **GitHub Issues:** https://github.com/twzrd/milo-token/issues

For urgent opt-out requests or compliance inquiries, email us directly.
