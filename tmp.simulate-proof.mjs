import { Connection, VersionedTransaction, Transaction } from '@solana/web3.js';

const CHANNEL = process.env.CHANNEL || 'youtube_lofi';
const EPOCH = Number(process.env.EPOCH || 122523);
const WALLET = process.env.WALLET || 'AmMftc4zHgR4yYfv29awV9Q46emo2aGPFW8utP81CsBv';
const API = process.env.API || 'https://api.twzrd.xyz';
const RPC = process.env.RPC || 'https://api.mainnet-beta.solana.com';

async function main() {
  const url = `${API}/v1/proof/${encodeURIComponent(CHANNEL)}/${EPOCH}/${encodeURIComponent(WALLET)}`;
  console.error(`[simulate] Fetching proof: ${url}`);
  const res = await fetch(url, { headers: { 'accept': 'application/json' } });
  if (!res.ok) {
    const text = await res.text();
    throw new Error(`Proof fetch failed: HTTP ${res.status} ${res.statusText}: ${text}`);
  }
  const proof = await res.json();
  if (!proof?.transaction) throw new Error('No transaction field in proof');

  const b64 = proof.transaction;
  const bytes = Buffer.from(b64, 'base64');

  let tx;
  try {
    tx = VersionedTransaction.deserialize(bytes);
    console.error('[simulate] Parsed as VersionedTransaction');
  } catch (e) {
    tx = Transaction.from(bytes);
    console.error('[simulate] Parsed as legacy Transaction');
  }

  const connection = new Connection(RPC, 'confirmed');
  const sim = await connection.simulateTransaction(tx, {
    sigVerify: false,
    replaceRecentBlockhash: true,
    commitment: 'processed',
  });

  const logs = sim?.value?.logs || [];
  const err = sim?.value?.err || null;
  console.log(JSON.stringify({ ok: !err, err, logs }, null, 2));
}

main().catch((e) => {
  console.error('[simulate] ERROR:', e?.message || e);
  process.exit(1);
});
