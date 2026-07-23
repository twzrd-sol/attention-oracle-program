# Security Policy

Report security issues privately to **security@twzrd.xyz**. Please include the
affected program address, relevant transaction signatures, and reproduction
steps. We aim to acknowledge reports within 72 hours.

## Deployed programs in scope

This repository hosts the on-chain Solana programs for the Liquid Attention
Protocol:

| Program | Address | Upgradeability |
|---------|---------|----------------|
| Attention Oracle | `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop` | Immutable (upgrade authority revoked) |
| wzrd-markets | `7YZZxQC9JrWwoo1W1fYgxZs6rnbW17iF72mi65vU93sy` | Upgradeable |
| wzrd-rails | `BdSv824hvYeGAWQZUcypRzAor8yJit2qeqCHty3CSZy9` | Upgradeable |

The Attention Oracle program is immutable; reports against it are accepted for
transparency but cannot be patched on-chain. wzrd-markets and wzrd-rails are
upgradeable and are the priority scope for actionable reports.

There is no paid bug-bounty program at this time. Coordinated private disclosure
to the address above is the supported channel.
