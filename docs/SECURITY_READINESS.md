Security Readiness – Critical Controls

1) Pre-commit secret guard (local)
- Location: .githooks/pre-commit
- Enable: scripts/enable-git-hooks.sh
- Bypass (discouraged): SKIP_SECRET_CHECK=1 git commit ...
- Allowlist false positives: add exact path to .secretsallow

2) .gitignore audit
- Repo already ignores most secrets; keep *.json exceptions in mind (program keypairs live in target/deploy and should never be committed).

3) Deployment guard wrapper
- Location: scripts/guard-deploy.sh
- Ensures: mainnet RPC, keypair outside repo, strict file perms, interactive confirm

4) PR-only main policy (CI)
- Workflow: .github/workflows/protect-main.yml blocks direct pushes to main
- Make it a required status check in branch protection settings

Recommended High-Impact
- Enforce 2FA org-wide, enable secret scanning + push protection
- Require signed commits on main, require code owner reviews
