module.exports = {
  apps: [{
    name: 'gateway',
    script: 'dist/index.js',
    args: '--port 8082',
    cwd: '/home/twzrd/milo-token/apps/gateway',
    env: {
      NODE_ENV: 'production',
      PORT: '8082',
      // Explicit DB for gateway proof routes (read-only access)
      GATEWAY_DATABASE_URL: 'postgresql://doadmin:AVNS_7OLyCRhJkIPcAKrZMoi@twzrd-prod-postgres-do-user-21113270-0.f.db.ondigitalocean.com:25061/twzrd-oracle-pool?sslmode=require',
      EPOCH_SECONDS: '60'
    }
  }]
};
