# DNS Configuration for twzrd.xyz

Copy-paste these records into your DNS provider (Cloudflare, Namecheap, Route53, etc.)

## Email Security Records

### SPF Record
**Type:** TXT
**Name:** `@` or `twzrd.xyz`
**Value:**
```
v=spf1 include:sendgrid.net include:_spf.google.com ~all
```

**Purpose:** Prevents email spoofing by specifying authorized mail servers.

---

### DMARC Record
**Type:** TXT
**Name:** `_dmarc` or `_dmarc.twzrd.xyz`
**Value:**
```
v=DMARC1; p=quarantine; rua=mailto:security@twzrd.xyz; ruf=mailto:security@twzrd.xyz; fo=1; adkim=s; aspf=s
```

**Purpose:** Email authentication policy; instructs receivers on how to handle failures.

---

### DKIM Record
**Type:** TXT
**Name:** `default._domainkey` (or provider-specific selector)
**Value:**
```
[GET THIS FROM YOUR EMAIL PROVIDER]
```

**For SendGrid:**
1. Log into SendGrid
2. Go to Settings → Sender Authentication → Authenticate Your Domain
3. Follow wizard to generate DKIM records
4. Copy the generated CNAME records

**For Google Workspace:**
1. Admin Console → Apps → Google Workspace → Gmail → Authenticate email
2. Generate new DKIM key
3. Copy TXT record value

---

## Security & Monitoring

### CAA Record (Certificate Authority Authorization)
**Type:** CAA
**Name:** `@` or `twzrd.xyz`
**Value:**
```
0 issue "letsencrypt.org"
0 issuewild "letsencrypt.org"
0 iodef "mailto:security@twzrd.xyz"
```

**Purpose:** Restricts which CAs can issue certificates for your domain.

---

### MTA-STS (Mail Transfer Agent Strict Transport Security)
**Type:** TXT
**Name:** `_mta-sts`
**Value:**
```
v=STSv1; id=20251111T000000
```

**Type:** CNAME
**Name:** `mta-sts`
**Value:** `your-hosting-provider.com` (or create A record pointing to your server)

Then create file at `https://mta-sts.twzrd.xyz/.well-known/mta-sts.txt`:
```
version: STSv1
mode: enforce
mx: mail.google.com
mx: aspmx.l.google.com
max_age: 604800
```

---

## Email Aliases

### Required Aliases (RFC 2142)
Set up these email forwards/aliases:

- `security@twzrd.xyz` → your-main-email@domain.com
- `abuse@twzrd.xyz` → your-main-email@domain.com
- `dev@twzrd.xyz` → your-main-email@domain.com
- `postmaster@twzrd.xyz` → your-main-email@domain.com
- `hostmaster@twzrd.xyz` → your-main-email@domain.com

---

## DNSSEC (Optional but Recommended)

Enable DNSSEC in your DNS provider's dashboard:
- **Cloudflare:** DNS → Settings → Enable DNSSEC
- **Namecheap:** Domain List → Manage → Advanced DNS → DNSSEC
- **Route53:** Hosted zones → Enable DNSSEC signing

---

## Verification Commands

After adding records, verify with:

```bash
# SPF
dig TXT twzrd.xyz | grep spf1

# DMARC
dig TXT _dmarc.twzrd.xyz

# DKIM (replace 'default' with your selector)
dig TXT default._domainkey.twzrd.xyz

# CAA
dig CAA twzrd.xyz

# MTA-STS
dig TXT _mta-sts.twzrd.xyz
curl https://mta-sts.twzrd.xyz/.well-known/mta-sts.txt
```

---

## SSL/TLS Configuration

### HSTS Preload Header
Add to your web server config (Nginx):

```nginx
add_header Strict-Transport-Security "max-age=63072000; includeSubDomains; preload" always;
```

Then submit to: https://hstspreload.org

---

## DNS Propagation

After adding records, allow 24-48 hours for full propagation.
Check status: https://dnschecker.org

---

**Status:** Ready to deploy
**Last Updated:** 2025-11-11
**Contact:** dev@twzrd.xyz
