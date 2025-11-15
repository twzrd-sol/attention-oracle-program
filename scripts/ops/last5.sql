-- Recent activity per channel (last 5 minutes)
SELECT
  channel,
  COUNT(*) AS records_last_5min,
  to_timestamp(MAX(first_seen)) AS last_record,
  EXTRACT(EPOCH FROM NOW()) - MAX(first_seen) AS seconds_ago
FROM channel_participation
WHERE first_seen >= EXTRACT(EPOCH FROM NOW()) - 300
GROUP BY channel
ORDER BY last_record DESC;

