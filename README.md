# Attention Oracle Program

## Mainnet Deployment
- Program ID: `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop`
- Authority: `2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD`

## Verification
To verify the on-chain bytecode matches this repository:

1. Build: `anchor build --verifiable`
2. Dump: `solana program dump -u m GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop dump.so`
3. Compare: `sha256sum target/verifiable/token_2022.so` vs `sha256sum dump.so`
