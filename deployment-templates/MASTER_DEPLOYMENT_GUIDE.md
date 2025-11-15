# TWZRD Master Deployment Guide

Complete execution checklist for securing namespace, brand, and infrastructure.

**Status:** All templates ready
**Execution Time:** 4-6 hours total
**Priority Order:** Follow sequence below

---

## 🎯 Phase 1: Namespace Lockdown (Do Today)

### 1A. NPM Scope - @twzrd (~15 min)

**Location:** `/home/twzrd/milo-token/packages/sdk/`

```bash
# Step 1: Login to npm
npm login

# Step 2: Publish placeholder package
cd /home/twzrd/milo-token/packages/sdk
npm publish --access=public

# Step 3: Verify
npm info @twzrd/sdk
```

**Success Criteria:**
- ✅ Package visible at https://npmjs.com/package/@twzrd/sdk
- ✅ Scope @twzrd reserved
- ✅ README displays correctly

---

### 1B. Crates.io - twzrd-sdk & twzrd-cli (~20 min)

**Location:** `/home/twzrd/milo-token/rust-packages/`

```bash
# Step 1: Login to crates.io
cargo login [YOUR_API_TOKEN]

# Step 2: Publish SDK
cd /home/twzrd/milo-token/rust-packages/twzrd-sdk
cargo publish

# Step 3: Publish CLI
cd /home/twzrd/milo-token/rust-packages/twzrd-cli
cargo publish

# Step 4: Verify
cargo search twzrd
```

**Success Criteria:**
- ✅ https://crates.io/crates/twzrd-sdk exists
- ✅ https://crates.io/crates/twzrd-cli exists
- ✅ Proper descriptions and links display

**Troubleshooting:**
- If publish fails with "program ID" error, update placeholder in `lib.rs`
- If missing dependencies, add to workspace `Cargo.toml`

---

### 1C. Docker/Container Registries (~10 min)

**GitHub Container Registry:**
```bash
# Create placeholder Dockerfile
cat > /home/twzrd/milo-token/Dockerfile <<EOF
FROM node:20-alpine
WORKDIR /app
COPY package*.json ./
RUN npm install
COPY . .
CMD ["npm", "start"]
EOF

# Build and push
docker build -t ghcr.io/twzrd-sol/attention-oracle:latest .
docker push ghcr.io/twzrd-sol/attention-oracle:latest
```

**Docker Hub (Optional):**
- Create org: https://hub.docker.com/orgs/twzrd
- Push: `docker tag ... twzrd/attention-oracle:latest`

---

## 🌐 Phase 2: DNS & Email Security (~30 min)

**Reference:** `/home/twzrd/milo-token/deployment-templates/DNS_CONFIGURATION.md`

### 2A. Core DNS Records

Log into your DNS provider and add:

```
# SPF
Type: TXT
Name: @
Value: v=spf1 include:sendgrid.net include:_spf.google.com ~all

# DMARC
Type: TXT
Name: _dmarc
Value: v=DMARC1; p=quarantine; rua=mailto:security@twzrd.xyz; fo=1

# CAA
Type: CAA
Name: @
Value: 0 issue "letsencrypt.org"

# Docs subdomain
Type: CNAME
Name: docs
Value: [your-docs-deployment-url]
```

### 2B. Email Provider Setup

**If using SendGrid:**
1. SendGrid → Settings → Sender Authentication
2. Authenticate domain: twzrd.xyz
3. Copy DKIM records to DNS
4. Verify

**If using Google Workspace:**
1. Admin Console → Apps → Gmail → Authenticate Email
2. Generate DKIM key
3. Add TXT record to DNS
4. Verify

### 2C. Email Aliases

Set up forwards:
- security@twzrd.xyz → [your-email]
- abuse@twzrd.xyz → [your-email]
- dev@twzrd.xyz → [your-email]
- postmaster@twzrd.xyz → [your-email]

### 2D. Verify Setup

```bash
# Check SPF
dig TXT twzrd.xyz | grep spf1

# Check DMARC
dig TXT _dmarc.twzrd.xyz

# Test email
echo "Test" | mail -s "Test from dev@twzrd.xyz" dev@twzrd.xyz
```

**Success Criteria:**
- ✅ All records resolve correctly
- ✅ Email test delivers successfully
- ✅ No delivery warnings/spam flags

---

## 🔐 Phase 3: GitHub Security Hardening (~45 min)

**Reference:** `/home/twzrd/milo-token/deployment-templates/GITHUB_SECURITY.md`

### 3A. Organization Settings

Visit: `https://github.com/organizations/twzrd-sol/settings`

**Security:**
- ✅ Enable 2FA requirement (0 days grace period)
- ✅ Enable Dependabot alerts
- ✅ Enable secret scanning
- ✅ Enable push protection

**Member Privileges:**
- ✅ Base permissions: Read
- ✅ Repository creation: Admins only
- ✅ Repository forking: Disabled for private repos

### 3B. Repository Setup

For each critical repo:

**1. Branch Protection (Settings → Branches → Add Rule)**
```yaml
Branch: main
Rules:
  ✅ Require pull request reviews (1 approval)
  ✅ Require review from CODEOWNERS
  ✅ Require status checks to pass
  ✅ Require conversation resolution
  ✅ Require linear history
  ✅ No force pushes
  ✅ No deletions
```

**2. Add CODEOWNERS**
```bash
mkdir -p .github
cp /home/twzrd/milo-token/deployment-templates/GITHUB_SECURITY.md .github/CODEOWNERS
# Edit with actual team/user handles
git add .github/CODEOWNERS
git commit -m "chore: add CODEOWNERS"
git push
```

**3. Add Security Policy**
```bash
# Extract SECURITY.md from GITHUB_SECURITY.md template
git add SECURITY.md
git commit -m "docs: add security policy"
git push
```

**4. Add Issue/PR Templates**
```bash
mkdir -p .github/ISSUE_TEMPLATE
# Copy templates from GITHUB_SECURITY.md
git add .github/
git commit -m "chore: add issue and PR templates"
git push
```

### 3C. Secrets Management

**Settings → Secrets and Variables → Actions**

Add:
- `SOLANA_MAINNET_RPC_URL`
- `SOLANA_DEVNET_RPC_URL`
- `DEPLOYER_PRIVATE_KEY` (encrypted)
- `NPM_TOKEN`
- `CARGO_REGISTRY_TOKEN`

**Success Criteria:**
- ✅ All team members have 2FA enabled
- ✅ Branch protection active on main
- ✅ CODEOWNERS enforced
- ✅ Security scanning active
- ✅ Secrets configured

---

## 📚 Phase 4: Documentation Site (~30 min)

**Reference:** `/home/twzrd/milo-token/docs-site/README.md`
**Location:** `/home/twzrd/milo-token/docs-site/`

### Option A: Netlify (Fastest)

```bash
# Install Netlify CLI
npm install -g netlify-cli

# Login
netlify login

# Deploy
cd /home/twzrd/milo-token/docs-site
netlify deploy --prod

# Follow prompts:
# - Create new site
# - Build command: (leave empty)
# - Publish directory: .
```

**Then:**
1. Netlify Dashboard → Domain Settings → Add Custom Domain
2. Enter: `docs.twzrd.xyz`
3. Add DNS record:
   ```
   Type: CNAME
   Name: docs
   Value: [your-site].netlify.app
   ```
4. Wait for SSL cert (auto-generated)

### Option B: Vercel

```bash
npm i -g vercel
cd /home/twzrd/milo-token/docs-site
vercel --prod

# Add custom domain in dashboard
```

### Option C: GitHub Pages

```bash
cd /home/twzrd/milo-token
git checkout -b gh-pages
cp -r docs-site/* .
echo "docs.twzrd.xyz" > CNAME
git add .
git commit -m "docs: deploy to GitHub Pages"
git push origin gh-pages

# Enable in repo Settings → Pages
```

**Success Criteria:**
- ✅ Site accessible at https://docs.twzrd.xyz
- ✅ All pages load correctly
- ✅ SSL certificate valid
- ✅ Mobile responsive

---

## 🎨 Phase 5: Social & Web3 Handles (~45 min)

### 5A. Web3 Identities

**Farcaster:**
1. Visit: https://warpcast.com
2. Sign up with wallet
3. Claim username: `twzrd`
4. Add bio: "Official TWZRD Attention Oracle on Solana"
5. Link: https://twzrd.xyz

**Lens Protocol:**
1. Visit: https://claim.lens.xyz
2. Connect wallet
3. Claim: `twzrd.lens`
4. Set profile metadata

**Solana Name Service:**
```bash
# Register twzrd.sol domain
# Visit: https://naming.bonfida.org
# Search: twzrd.sol
# Register if available (~$20/year)
```

### 5B. Social Media

**Reddit:**
1. Create account: u/twzrd_xyz
2. Claim r/TWZRD subreddit
3. Pin official links in description

**Bluesky:**
1. Join: https://bsky.app
2. Register: @twzrd.xyz (custom domain handle)
3. Follow setup: https://bsky.social/about/blog/4-28-2023-domain-handle-tutorial

**Discord:**
1. Create server: TWZRD
2. Claim vanity URL: /twzrd (requires boost level)
3. Set up roles, channels, verification

**Telegram:**
1. Create channel: @twzrd_xyz
2. Create group: @twzrd_community
3. Link bot for announcements

### 5C. Pin Official Links

Create pinned post on X/Twitter:

```
🚀 Official TWZRD Links

Website: https://twzrd.xyz
Docs: https://docs.twzrd.xyz
GitHub: https://github.com/twzrd-sol/attention-oracle-program
NPM: https://npmjs.com/package/@twzrd/sdk
Crates: https://crates.io/crates/twzrd-sdk

Verify all links before connecting!

#Solana #Web3 #AttentionEconomy
```

**Success Criteria:**
- ✅ All handles secured
- ✅ Consistent branding across platforms
- ✅ Official links pinned/bio'd
- ✅ Cross-links between platforms

---

## 🏛️ Phase 6: Trademark Filing (~3-4 hours)

**Reference:** `/home/twzrd/milo-token/deployment-templates/USPTO_TRADEMARK_FILING.md`

### 6A. Pre-Filing Search

1. Visit: https://tmsearch.uspto.gov
2. Search: "TWZRD", "WIZARD"
3. Check Classes 9, 42 for conflicts
4. Document: No conflicts found

### 6B. Gather Materials

**Specimens:**
- Class 9: Screenshot of https://npmjs.com/package/@twzrd/sdk
- Class 42: Screenshot of https://twzrd.xyz

**Dates:**
- First use: [Check git log for first commit]
  ```bash
  cd /home/twzrd/milo-token
  git log --reverse --oneline | head -1
  ```
- First commercial use: [Domain registration or first mainnet deploy]

**Owner Info:**
- Entity: TWZRD Inc.
- Address: [Your business address]
- Email: dev@twzrd.xyz
- Phone: [Your phone]

### 6C. File TEAS Plus Application

1. Visit: https://www.uspto.gov/trademarks/apply
2. Select: TEAS Plus ($250/class)
3. Create USPTO account
4. Fill form:
   - Mark: TWZRD
   - Type: Standard Character
   - Classes: 9, 42 (copy descriptions from guide)
   - Upload specimens
   - Enter dates
5. Review 3 times
6. Pay $500 (2 classes × $250)
7. Submit
8. **Save serial number**

### 6D. Set Monitoring Reminders

```
Calendar reminders:
- 3 months: Check application status
- 5-6 years: File Section 8 & 15 ($425)
- 10 years: File Section 9 renewal ($300)
```

**Success Criteria:**
- ✅ Application filed (serial number received)
- ✅ Specimens accepted
- ✅ Payment processed
- ✅ Reminders set for maintenance
- ✅ Using TWZRD™ (with ™ symbol) everywhere

---

## 📊 Phase 7: Monitoring & Analytics (~30 min)

### 7A. Uptime Monitoring

**UptimeRobot (Free):**
1. Visit: https://uptimerobot.com
2. Add monitors:
   - https://twzrd.xyz
   - https://docs.twzrd.xyz
   - https://twzrd.xyz/claim.html
3. Set alert email: dev@twzrd.xyz
4. Check interval: 5 minutes

### 7B. Security Monitoring

**GitHub Security:**
- Already enabled (Phase 3)
- Check: Settings → Security → Overview

**Domain Monitoring:**
- Google Search Console: https://search.google.com/search-console
- Submit sitemaps:
  - https://twzrd.xyz/sitemap.xml
  - https://docs.twzrd.xyz/sitemap.xml (create one)

**SSL Monitoring:**
- Certificate expires: Check via browser or:
  ```bash
  echo | openssl s_client -servername twzrd.xyz -connect twzrd.xyz:443 2>/dev/null | openssl x509 -noout -dates
  ```

### 7C. Analytics (Optional)

**Privacy-Friendly (Recommended):**
```html
<!-- Add to all pages -->
<script defer data-domain="twzrd.xyz" src="https://plausible.io/js/script.js"></script>
```

**Or Google Analytics:**
- Create GA4 property
- Add tracking code to pages
- Set up conversion goals

**Success Criteria:**
- ✅ Uptime monitoring active
- ✅ SSL expiry notifications set
- ✅ Search Console configured
- ✅ Analytics (optional) deployed

---

## ✅ Final Verification Checklist

Print this and verify:

### Namespace
- [ ] @twzrd/sdk published on npm
- [ ] twzrd-sdk published on crates.io
- [ ] twzrd-cli published on crates.io
- [ ] GitHub org: twzrd-sol active

### DNS & Email
- [ ] SPF record resolves
- [ ] DMARC record resolves
- [ ] DKIM configured and verified
- [ ] Email aliases working (test each)
- [ ] CAA records active

### GitHub Security
- [ ] 2FA required for all members
- [ ] Branch protection on main
- [ ] CODEOWNERS file active
- [ ] Secret scanning enabled
- [ ] SECURITY.md published
- [ ] Issue/PR templates added

### Documentation
- [ ] docs.twzrd.xyz live and loading
- [ ] All pages accessible
- [ ] SSL certificate valid
- [ ] Mobile responsive test passed

### Social/Web3
- [ ] Farcaster: @twzrd claimed
- [ ] Lens: twzrd.lens claimed
- [ ] Reddit: u/twzrd_xyz active
- [ ] Bluesky: @twzrd.xyz claimed
- [ ] Discord server created
- [ ] Telegram channel active
- [ ] Official links pinned on X

### Trademark
- [ ] USPTO application filed
- [ ] Serial number saved
- [ ] Specimens accepted
- [ ] Using TWZRD™ consistently
- [ ] Maintenance reminders set

### Monitoring
- [ ] Uptime monitoring active
- [ ] SSL expiry alerts set
- [ ] Search Console configured
- [ ] Security scanning active

---

## 🚀 Post-Launch Actions

### Week 1
- [ ] Monitor trademark application status
- [ ] Check DNS propagation (24-48 hours)
- [ ] Test all email addresses
- [ ] Cross-link all social accounts
- [ ] Submit docs site to search engines

### Month 1
- [ ] Respond to any USPTO office actions
- [ ] Review analytics (if enabled)
- [ ] Update docs based on user feedback
- [ ] Announce SDK availability on social

### Month 3
- [ ] Check trademark status (should be approved or in opposition)
- [ ] Review security scan results
- [ ] Update package versions
- [ ] Plan feature roadmap

### Years 5-6
- [ ] **CRITICAL:** File USPTO Section 8 & 15 ($425)
- [ ] Don't miss this or trademark cancels!

### Year 10
- [ ] **CRITICAL:** File USPTO Section 9 renewal ($300)

---

## 📞 Support Contacts

**Domain/DNS Issues:**
- Your DNS provider support
- Cloudflare: https://support.cloudflare.com

**Package Registries:**
- npm support: https://npmjs.com/support
- crates.io: https://users.rust-lang.org

**Trademark Questions:**
- USPTO: 1-800-786-9199
- Or hire trademark attorney

**Security Issues:**
- security@twzrd.xyz
- GitHub Security: https://github.com/twzrd-sol/attention-oracle-program/security

---

## 💰 Total Cost Summary

| Item | Cost | Frequency |
|------|------|-----------|
| NPM Packages | Free | One-time |
| Crates.io Packages | Free | One-time |
| DNS Records | Free | Ongoing |
| Email (SendGrid/Workspace) | $0-$12/mo | Monthly |
| Docs Hosting (Netlify) | Free | Ongoing |
| Social/Web3 Handles | Free-$50 | One-time |
| USPTO Trademark | $500 | One-time |
| Uptime Monitoring | Free | Ongoing |
| **Total Year 1** | **$500-$700** | - |
| **Maintenance Years 2-4** | **$0-$144/yr** | Annually |
| **Trademark Maintenance (Year 5)** | **$425** | One-time |
| **Trademark Renewal (Year 10)** | **$300** | Every 10yr |

**Total 10-Year Cost:** ~$1,500-$2,500 (excluding optional services)

---

## 🎯 Execution Priority

If limited on time, execute in this order:

**Priority 1 (Critical - Do Today):**
1. Reserve npm/crates namespaces
2. File USPTO trademark
3. Configure DNS records
4. Enable GitHub 2FA

**Priority 2 (High - This Week):**
1. Deploy docs site
2. Claim social handles
3. Set up email aliases
4. Configure monitoring

**Priority 3 (Medium - This Month):**
1. Add GitHub templates
2. Cross-link social accounts
3. Set up analytics
4. Monitor trademark application

---

**All assets ready. Execute at will.**

**Total Execution Time:** 4-6 hours (DIY) or 2-3 hours (with help)
**Last Updated:** 2025-11-11
**Status:** ✅ Ready for deployment
**Contact:** dev@twzrd.xyz
