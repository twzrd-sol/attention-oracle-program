import { Connection, VersionedTransaction, Transaction } from '@solana/web3.js';

const CHANNEL = process.env.CHANNEL || 'youtube_lofi';
const EPOCH = Number(process.env.EPOCH || 122523);
const API = process.env.API || 'https://api.twzrd.xyz';
const RPC = process.env.RPC || 'https://api.mainnet-beta.solana.com';
const wallets = process.argv.slice(2);

if (wallets.length === 0) {
  console.error('Usage: node tmp.simulate-multi.mjs <WALLET1> [WALLET2 ...]');
  process.exit(2);
}

function short(k){ return k.slice(0,4)+'…'+k.slice(-4); }

async function fetchProof(wallet) {
  const url = `${API}/v1/proof/${encodeURIComponent(CHANNEL)}/${EPOCH}/${encodeURIComponent(wallet)}`;
  const res = await fetch(url, { headers: { 'accept': 'application/json' } });
  if (res.status === 404) return { status: 'ineligible', wallet };
  if (!res.ok) {
    const text = await res.text();
    return { status: 'error', wallet, error: `HTTP ${res.status} ${res.statusText}: ${text}` };
  }
  const proof = await res.json();
  if (!proof?.transaction) return { status: 'error', wallet, error: 'no transaction in proof' };
  return { status: 'eligible', wallet, proof };
}

async function simulateTx(b64) {
  const bytes = Buffer.from(b64, 'base64');
  let tx;
  try { tx = VersionedTransaction.deserialize(bytes); }
  catch { tx = Transaction.from(bytes); }
  const connection = new Connection(RPC, 'confirmed');
  const sim = await connection.simulateTransaction(tx, {
    sigVerify: false,
    replaceRecentBlockhash: true,
    commitment: 'processed',
  });
  const logs = sim?.value?.logs || [];
  const err = sim?.value?.err || null;
  return { err, logs };
}

(async () => {
  for (const w of wallets) {
    try {
      console.error(`\n[simulate] Wallet ${short(w)} — fetching proof…`);
      const r = await fetchProof(w);
      if (r.status === 'ineligible') {
        console.log(JSON.stringify({ wallet: w, status: 'ineligible' }));
        continue;
      }
      if (r.status === 'error') {
        console.log(JSON.stringify({ wallet: w, status: 'error', error: r.error }));
        continue;
      }
      const { proof } = r;
      const { err, logs } = await simulateTx(proof.transaction);
      const already = logs.some((l) => /Already claimed/i.test(l));
      console.log(JSON.stringify({ wallet: w, status: already ? 'already_claimed' : (err ? 'error' : 'ok'), err, logs }));
    } catch (e) {
      console.log(JSON.stringify({ wallet: w, status: 'error', error: e?.message || String(e) }));
    }
  }
})();
