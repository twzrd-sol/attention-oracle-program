module.exports = {
  apps: [
    {
      name: 'irc-collector-justinbieber',
      script: 'npx',
      args: 'tsx clean-hackathon/scripts/twitch-irc-collector.ts',
      cwd: __dirname,
      env: {
        CHANNELS: 'justinbieber',
        DATABASE_HOST: '/var/run/postgresql',
        SNAPSHOT_EVERY_SEC: '60'
        // To persist to DB via TCP instead, set DATABASE_URL here.
      },
      autorestart: true,
      max_restarts: 10,
      restart_delay: 2000,
      watch: false
    }
  ]
}

