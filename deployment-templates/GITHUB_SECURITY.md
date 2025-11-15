# GitHub Organization Security Hardening

Complete checklist and templates for securing the TWZRD GitHub organization.

## Organization Settings

### 1. Enable Two-Factor Authentication (2FA)
**Path:** Settings → Authentication security → Two-factor authentication

- ✅ Require 2FA for all members
- ✅ Require 2FA for outside collaborators
- ✅ Set grace period to 0 days (immediate enforcement)

### 2. Member Privileges
**Path:** Settings → Member privileges

```
Base permissions: Read
Repository creation: Admins only
Repository forking: Private and internal repos - Disabled
Pages creation: Public repos only
```

### 3. Enable Security Features
**Path:** Settings → Code security and analysis

- ✅ Dependency graph (enabled)
- ✅ Dependabot alerts (enabled for all repos)
- ✅ Dependabot security updates (enabled)
- ✅ Secret scanning (enabled for all repos)
- ✅ Secret scanning push protection (enabled)
- ✅ Code scanning (GitHub Advanced Security if available)

---

## Repository Settings (Per Repo)

### Branch Protection Rules
**Path:** Repo → Settings → Branches → Add rule

**Branch name pattern:** `main`

```yaml
Protection Rules:
  ✅ Require pull request reviews before merging
    - Required approving reviews: 1
    - Dismiss stale PR approvals when new commits are pushed: Yes
    - Require review from Code Owners: Yes
    - Restrict who can dismiss PR reviews: Admins only

  ✅ Require status checks to pass before merging
    - Require branches to be up to date before merging: Yes
    - Status checks (add when CI is set up):
      - tests
      - build
      - lint

  ✅ Require conversation resolution before merging

  ✅ Require signed commits: Yes (optional but recommended)

  ✅ Require linear history: Yes

  ✅ Include administrators: No (allows admins to bypass for emergencies)

  ✅ Restrict who can push to matching branches:
    - Only admins and specific teams

  ✅ Allow force pushes: No

  ✅ Allow deletions: No
```

---

## CODEOWNERS File

Create `.github/CODEOWNERS` in repository root:

```
# TWZRD Code Owners
# Lines starting with '#' are comments.
# Each line is a file pattern followed by one or more owners.

# Default owners for everything in the repo
* @twzrd-core-team

# Solana program code
/programs/ @twzrd-core-team @solana-devs

# Security-critical files
/programs/token-2022/src/instructions/ @twzrd-core-team
/programs/token-2022/src/constants.rs @twzrd-core-team
/scripts/ @twzrd-core-team

# SDK packages
/packages/ @twzrd-sdk-maintainers
/rust-packages/ @twzrd-sdk-maintainers

# Documentation
/docs/ @twzrd-docs-team
*.md @twzrd-docs-team

# CI/CD and infrastructure
/.github/ @twzrd-devops
/deployment-templates/ @twzrd-devops

# Legal and brand assets
/clean-hackathon/public/terms.html @twzrd-legal
/clean-hackathon/public/privacy.html @twzrd-legal
/clean-hackathon/public/brand.html @twzrd-legal
```

---

## Security Policy

Create `SECURITY.md` in repository root:

```markdown
# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

## Reporting a Vulnerability

**DO NOT** create a public GitHub issue for security vulnerabilities.

Instead, report to: **security@twzrd.xyz**

### What to Include

1. Description of the vulnerability
2. Steps to reproduce
3. Potential impact
4. Suggested fix (if any)

### Response Timeline

- **Initial Response:** Within 24 hours
- **Status Update:** Within 7 days
- **Fix Timeline:** Based on severity (critical: <48h, high: <7d, medium: <30d)

### Disclosure Policy

We follow coordinated disclosure:
1. Vulnerability reported to security@twzrd.xyz
2. We confirm and develop fix
3. Fix deployed to production
4. Public disclosure after 90 days or when fix is deployed (whichever is first)

### Bug Bounty

Currently evaluating bug bounty program. Contact security@twzrd.xyz for details.

## Security Contacts

- **Security Team:** security@twzrd.xyz
- **PGP Key:** [Link when available]
- **.well-known/security.txt:** https://twzrd.xyz/.well-known/security.txt

## Known Security Features

- ✅ SPL Token 2022 with Transfer Hooks
- ✅ On-chain verification of Twitch presence
- ✅ Rate limiting on claims
- ✅ Epoch-based token distribution
- ✅ Admin controls for channel management

## Audits

- **Smart Contract Audit:** Pending
- **Penetration Test:** Pending

---

**TWZRD Inc.** · Built in Houston, TX · https://twzrd.xyz
```

---

## GitHub Actions Security

### Secrets Management
**Path:** Repo → Settings → Secrets and variables → Actions

```
Required Secrets:
- SOLANA_MAINNET_RPC_URL
- SOLANA_DEVNET_RPC_URL
- DEPLOYER_PRIVATE_KEY (encrypted, restricted to protected branches)
- NPM_TOKEN (for package publishing)
- CARGO_REGISTRY_TOKEN (for crates.io publishing)
```

### Workflow Permissions
**Path:** Repo → Settings → Actions → General → Workflow permissions

```
✅ Read repository contents and packages permissions
❌ Read and write permissions (only enable for specific trusted workflows)

✅ Allow GitHub Actions to create and approve pull requests: No
```

---

## Issue and PR Templates

### Bug Report Template
Create `.github/ISSUE_TEMPLATE/bug_report.md`:

```markdown
---
name: Bug report
about: Create a report to help us improve
title: '[BUG] '
labels: bug
assignees: ''
---

**Describe the bug**
A clear description of what the bug is.

**To Reproduce**
Steps to reproduce:
1.
2.
3.

**Expected behavior**
What you expected to happen.

**Environment:**
- OS: [e.g. Ubuntu 22.04]
- Solana CLI: [e.g. 1.18.26]
- Node.js: [e.g. 20.x]

**Additional context**
Any other relevant information.
```

### Feature Request Template
Create `.github/ISSUE_TEMPLATE/feature_request.md`:

```markdown
---
name: Feature request
about: Suggest an idea for TWZRD
title: '[FEATURE] '
labels: enhancement
assignees: ''
---

**Problem Statement**
What problem does this solve?

**Proposed Solution**
How should this work?

**Alternatives Considered**
What other approaches did you consider?

**Additional Context**
Links, mockups, or examples.
```

### Pull Request Template
Create `.github/PULL_REQUEST_TEMPLATE.md`:

```markdown
## Description
Brief description of changes.

## Type of Change
- [ ] Bug fix
- [ ] New feature
- [ ] Breaking change
- [ ] Documentation update

## Checklist
- [ ] Code follows style guidelines
- [ ] Self-review completed
- [ ] Comments added for complex code
- [ ] Documentation updated
- [ ] No new warnings
- [ ] Tests added/updated
- [ ] All tests pass
- [ ] Dependent changes merged

## Testing
How was this tested?

## Related Issues
Fixes #(issue)
```

---

## Organization Webhooks (Optional)

### Discord/Slack Integration
**Path:** Org Settings → Webhooks

```
Events to Monitor:
✅ Repository creation/deletion
✅ Member added/removed
✅ Security alerts
✅ Release published
```

---

## Deployment Instructions

1. **Enable Org-Level Settings:**
   ```bash
   # Navigate to: https://github.com/organizations/twzrd-sol/settings/security
   # Enable all security features
   ```

2. **Add CODEOWNERS:**
   ```bash
   mkdir -p .github
   # Copy CODEOWNERS content above to .github/CODEOWNERS
   git add .github/CODEOWNERS
   git commit -m "chore: add CODEOWNERS for code review requirements"
   ```

3. **Add Security Policy:**
   ```bash
   # Copy SECURITY.md content above to root
   git add SECURITY.md
   git commit -m "docs: add security policy"
   ```

4. **Configure Branch Protection:**
   - Go to each critical repo → Settings → Branches
   - Click "Add rule"
   - Copy settings from "Branch Protection Rules" section above

5. **Create Issue Templates:**
   ```bash
   mkdir -p .github/ISSUE_TEMPLATE
   # Copy templates above
   git add .github/ISSUE_TEMPLATE/
   git commit -m "chore: add issue and PR templates"
   ```

---

**Status:** Ready to deploy
**Last Updated:** 2025-11-11
**Priority:** Critical (do before public launch)
