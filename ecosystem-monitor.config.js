const fs = require('fs');

// Load DATABASE_URL from .env
let databaseUrl = '';
const envContent = fs.readFileSync('.env', 'utf8');
const match = envContent.match(/^DATABASE_URL=(.+)$/m);
if (match) {
  databaseUrl = match[1].trim();
}

module.exports = {
  apps: [
    {
      name: 'off-chain-monitor',
      script: 'npx',
      args: 'tsx monitor-off-chain.ts',
      exec_mode: 'fork',
      instances: 1,
      env: {
        NODE_ENV: 'production',
        NODE_TLS_REJECT_UNAUTHORIZED: '0',
        DATABASE_URL: databaseUrl
      },
      error_file: './logs/off-chain-monitor-error.log',
      out_file: './logs/off-chain-monitor-out.log',
      log_file: './logs/off-chain-monitor.log',
      time: true,
      merge_logs: false
    }
  ]
};
