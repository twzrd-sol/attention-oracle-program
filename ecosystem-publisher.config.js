module.exports = {
  apps: [{
    name: 'publisher',
    script: 'tsx',
    args: 'scripts/publisher/publish-category-root.ts',
    cwd: '/home/twzrd/milo-token',
    // Run shortly after the top of the hour (after AUTO_FINALIZE seals N-1)
    cron_restart: '5 * * * *',
    autorestart: false,
    env: {
      NODE_ENV: 'production',
      // DB settings (kept for parity; publisher fetches from aggregator)
      DATABASE_TYPE: 'postgres',
      DATABASE_URL: 'postgresql://twzrd:twzrd_password_2025@localhost:5432/twzrd_oracle',
      PROGRAM_ID: 'GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop',
      // Publish CLS roots only (MILO remains private)
      CLS_MINT: 'FZnUPK6eRWSQFEini3Go11JmVEqRNAQZgDP7q1DhyaKo',
      // MINT_PUBKEY left for backward-compat but unused when CLS_MINT is set
      MINT_PUBKEY: 'AAHd7u22jCMgmbF7ATkiY3BhkifD4MN3Vbsy4eYQGWN5',
      WALLET_PATH: '/home/twzrd/.config/solana/oracle-authority.json',
      PUBLISHER_RPC_URLS: process.env.PUBLISHER_RPC_URLS || process.env.RPC_URL || 'https://api.mainnet.solana.com',
      // Point publisher at the running aggregator
      AGGREGATOR_PORT: '8080',
      AGGREGATOR_URL: 'http://127.0.0.1:8080'
    }
  }]
};
