import { Pool } from 'pg';

const pool = new Pool({
  connectionString: process.env.DATABASE_URL,
  ssl: { rejectUnauthorized: false }
});

(async () => {
  try {
    const res = await pool.query(`
      SELECT 
        se.epoch,
        se.channel,
        COUNT(sp.user_hash)::int as participant_count
      FROM sealed_epochs se
      LEFT JOIN sealed_participants sp ON se.epoch = sp.epoch AND se.channel = sp.channel
      WHERE se.sealed_at > $1
      GROUP BY se.epoch, se.channel
      ORDER BY se.epoch DESC
      LIMIT 15
    `, [Math.floor(Date.now() / 1000) - 7200]);
    
    console.log("Participant counts (last 2 hours):");
    res.rows.forEach(r => {
      console.log(r.channel.padEnd(20), "participants:", r.participant_count);
    });
    
    const avg = res.rows.reduce((sum, r) => sum + parseInt(r.participant_count), 0) / res.rows.length;
    console.log("");
    console.log("Average participants per channel:", Math.round(avg));
    
    await pool.end();
  } catch (e) {
    console.error("ERROR:", e.message);
  }
})();
