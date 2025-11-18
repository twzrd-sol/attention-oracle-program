import http from 'node:http';
import { clusterApiUrl, Connection, PublicKey } from '@solana/web3.js';
import { getSwitchboardProgram, decodeLatestAggregatorValue } from './switchboard.js';

const PORT = Number(process.env.PORT || 3000);
const CLUSTER = (process.env.SB_CLUSTER || 'devnet') as 'devnet' | 'mainnet-beta' | 'testnet';
const FEED = process.env.SB_FEED;

if (!FEED) {
  console.error('Missing SB_FEED aggregator pubkey');
  process.exit(1);
}

const server = http.createServer(async (req, res) => {
  try {
    if (!req.url) return void res.end();
    if (req.url.startsWith('/price')) {
      const rpc = clusterApiUrl(CLUSTER);
      const conn = new Connection(rpc, 'confirmed');
      const prog = await getSwitchboardProgram(conn);
      const { price, slot } = await decodeLatestAggregatorValue(prog, new PublicKey(FEED));
      res.writeHead(200, { 'content-type': 'application/json' });
      return void res.end(JSON.stringify({ ok: true, cluster: CLUSTER, feed: FEED, price, slot }));
    }
    if (req.url.startsWith('/protected')) {
      const paid = req.headers['x-402-payment'] === 'true';
      if (!paid) {
        res.writeHead(402, { 'content-type': 'application/json' });
        return void res.end(JSON.stringify({ ok: false, code: 402, message: 'Payment Required', method: 'x402' }));
      }
      res.writeHead(200, { 'content-type': 'application/json' });
      return void res.end(JSON.stringify({ ok: true }));
    }
    res.writeHead(404);
    res.end();
  } catch (e: any) {
    res.writeHead(500, { 'content-type': 'application/json' });
    res.end(JSON.stringify({ ok: false, error: e?.message || String(e) }));
  }
});

server.listen(PORT, () => {
  console.log(`x402+Switchboard example listening on :${PORT}`);
});

