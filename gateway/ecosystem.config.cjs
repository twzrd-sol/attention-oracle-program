module.exports = {
  apps: [{
    name: 'gateway',
    script: 'dist/index.js',
    cwd: '/home/twzrd/milo-token/gateway',
    env: {
      NODE_ENV: 'production',
      PORT: '5000'
    },
    // Auto-restart on crashes
    autorestart: true,
    // Max memory before restart
    max_memory_restart: '500M',
    // Logging
    error_file: '~/.pm2/logs/gateway-error.log',
    out_file: '~/.pm2/logs/gateway-out.log',
    log_date_format: 'YYYY-MM-DD HH:mm:ss Z'
  }]
};
