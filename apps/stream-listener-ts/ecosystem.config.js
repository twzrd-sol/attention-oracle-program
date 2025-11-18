module.exports = {
  apps: [
    {
      name: 'stream-listener',
      script: 'dist/index.js',
      cwd: __dirname,
      env: { NODE_ENV: 'production' },
      instances: 1,
      autorestart: true,
      watch: false,
      max_memory_restart: '300M'
    }
  ]
};

