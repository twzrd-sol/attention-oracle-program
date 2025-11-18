# SDK Publishing Status

**Current Status**: 📦 **NOT YET PUBLISHED** (scaffolded and ready)

---

## ⚠️ Important Note

The SDKs are **scaffolded** but **not yet published** to NPM or crates.io.

Links in documentation (like `npm install @attention-oracle/sdk`) are **forward-looking** - they show where packages will be available once published.

---

## Current Status

| Package | Status | Publish Command | Will Be Available At |
|---------|--------|-----------------|----------------------|
| **@attention-oracle/sdk** (TS) | ⏳ Not Published | `npm publish --access public` | https://www.npmjs.com/package/@attention-oracle/sdk |
| **attention-oracle-sdk** (Rust) | ⏳ Not Published | `cargo publish` | https://crates.io/crates/attention-oracle-sdk |
| **@attention-oracle/cli** | ⏳ Not Published | `npm publish --access public` | https://www.npmjs.com/package/@attention-oracle/cli |

---

## What's Ready

### ✅ Scaffolded and Documented

- File structure created (`sdk/typescript/`, `sdk/rust/`, `cli/`)
- Package manifests configured (`package.json`, `Cargo.toml`)
- Core client code written (`client.ts`, `lib.rs`, `cli.ts`)
- Type definitions created (`types.ts`)
- Utility functions implemented (`utils.ts`)
- Examples written (`sdk/examples/*.ts`)
- READMEs with full API documentation

### ⏳ Needs Before Publishing

#### TypeScript SDK

```bash
cd sdk/typescript

# 1. Install dependencies
npm install
# Expected: Installs @solana/web3.js, @coral-xyz/anchor, etc.

# 2. (Optional) Generate from IDL
# npm run generate
# Requires: programs/target/idl/token_2022.json

# 3. Build
npm run build
# Creates: dist/index.js, dist/index.d.ts

# 4. Test
npm link
# Then in a test project:
# npm link @attention-oracle/sdk

# 5. Verify package contents
npm pack --dry-run
# Shows what will be published

# 6. Publish
npm login  # First time only
npm publish --access public
```

#### Rust SDK

```bash
cd sdk/rust

# 1. Test compilation
cargo build

# 2. Run tests
cargo test

# 3. Check package
cargo package --list
# Shows what will be published

# 4. Verify metadata
cargo publish --dry-run

# 5. Publish
cargo login  # First time only (paste token from crates.io)
cargo publish
```

#### CLI

```bash
cd cli

# 1. Install dependencies
npm install

# 2. Build
npm run build
# Creates: dist/cli.js

# 3. Test locally
npm link
ao --help
# Should show CLI help

# 4. Verify binary works
ao info
# Should show program info

# 5. Publish
npm publish --access public
```

---

## Why Not Published Yet?

The SDKs are **scaffolded** as part of the "three wishes" implementation, but they require:

1. **Dependencies installed** (not committed to git - in `.gitignore`)
2. **Build artifacts generated** (TypeScript → JavaScript compilation)
3. **Testing** (ensure all imports/exports work)
4. **NPM/crates.io accounts** (authentication required)
5. **Package name availability** (may need to adjust names if taken)

---

## Publishing Checklist

Before publishing, ensure:

### TypeScript SDK

- [ ] `npm install` completes without errors
- [ ] `npm run build` creates `dist/` directory
- [ ] `dist/index.js` and `dist/index.d.ts` exist
- [ ] All imports resolve (`@solana/web3.js`, etc.)
- [ ] Package size < 1MB
- [ ] README.md is accurate and helpful
- [ ] Version in package.json is correct (0.2.0)
- [ ] License field is correct (MIT OR Apache-2.0)

### Rust SDK

- [ ] `cargo build` succeeds
- [ ] `cargo test` passes (if tests exist)
- [ ] Dependencies resolve from crates.io
- [ ] README.md and Cargo.toml metadata complete
- [ ] License files present (MIT, Apache-2.0)
- [ ] Version matches program (0.2.0)

### CLI

- [ ] `npm run build` succeeds
- [ ] Binary works: `ao --help`
- [ ] All commands execute (info, passport, pda, etc.)
- [ ] Dependencies installed
- [ ] README has usage examples

---

## Expected Package Names

### If Names Are Available

- `@attention-oracle/sdk` (TypeScript)
- `attention-oracle-sdk` (Rust)
- `@attention-oracle/cli` (CLI)

### If Names Are Taken

Alternative naming:

- `@twzrd/attention-oracle-sdk` (scoped to org)
- `attention-oracle-solana` (Rust)
- `@twzrd/attention-oracle-cli`

Check availability:

```bash
# NPM
npm view @attention-oracle/sdk
# If 404: available
# If shows package info: taken

# Crates.io
cargo search attention-oracle-sdk
# If "no results": available
```

---

## Recommended Publishing Order

1. **Build and test locally first** (all 3 packages)
2. **Publish TypeScript SDK** (developers need this most)
3. **Publish CLI** (depends on TS SDK being available)
4. **Publish Rust SDK** (for on-chain developers)

---

## Local Usage (Before Publishing)

### TypeScript SDK

```bash
# In sdk/typescript
npm install
npm run build
npm link

# In your project
npm link @attention-oracle/sdk

# Use normally
import { AttentionOracleClient } from '@attention-oracle/sdk';
```

### Rust SDK

```bash
# In sdk/rust
cargo build

# In your project's Cargo.toml
[dependencies]
attention-oracle-sdk = { path = "../path/to/sdk/rust" }
```

### CLI

```bash
# In cli
npm install
npm run build
npm link

# Now available globally
ao --help
```

---

## Post-Publishing TODO

Once published, update documentation to:

1. ✅ Confirm package URLs are live
2. ✅ Add installation badges to READMEs
3. ✅ Update version numbers if needed
4. ✅ Announce on Discord/Twitter
5. ✅ Add to main repo README

---

## Questions?

**Before publishing**:
- Check package name availability
- Verify all dependencies are in `package.json` / `Cargo.toml`
- Test installation in a clean directory
- Review NPM/crates.io publishing guides

**Need help**:
- NPM publishing: https://docs.npmjs.com/packages-and-modules/contributing-packages-to-the-registry
- Crates.io publishing: https://doc.rust-lang.org/cargo/reference/publishing.html

---

**Status**: 📦 Scaffolded and documented, not yet published
**Next**: Complete checklist above → Publish → Celebrate 🎉
