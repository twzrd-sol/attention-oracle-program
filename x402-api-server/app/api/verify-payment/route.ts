import { NextRequest, NextResponse } from 'next/server';
import { clusterApiUrl, Connection, PublicKey } from '@solana/web3.js';

export async function GET(req: NextRequest) {
  const txSig = req.nextUrl.searchParams.get('tx');
  if (!txSig) {
    return NextResponse.json({ error: 'Missing tx signature (?tx=...)' }, { status: 400 });
  }

  const cluster = (process.env.SB_CLUSTER as 'devnet' | 'mainnet-beta') || 'devnet';
  const rpc = process.env.SOLANA_RPC_URL || clusterApiUrl(cluster);
  const connection = new Connection(rpc, 'confirmed');

  const recipientStr = process.env.X402_RECIPIENT || 'GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop';
  const minLamports = Number(process.env.X402_MIN_LAMPORTS || 0);

  try {
    const tx = await connection.getTransaction(txSig, { maxSupportedTransactionVersion: 0, commitment: 'confirmed' });
    if (!tx) {
      return NextResponse.json({ verified: false, error: 'Transaction not found' }, { status: 404 });
    }

    const recipient = new PublicKey(recipientStr);

    // Simple system transfer verification
    const log = tx.meta?.logMessages?.join('\n') || '';
    const preBalances = tx.meta?.preBalances || [];
    const postBalances = tx.meta?.postBalances || [];
    const keys = tx.transaction.message.getAccountKeys().keySegments().flat();
    const recipientIndex = keys.findIndex((k) => k.equals(recipient));

    let lamportsReceived = 0;
    if (recipientIndex >= 0 && preBalances[recipientIndex] !== undefined && postBalances[recipientIndex] !== undefined) {
      lamportsReceived = postBalances[recipientIndex] - preBalances[recipientIndex];
    }

    const systemTransfer = log.includes('Program 11111111111111111111111111111111 invoke');

    const verified = systemTransfer && lamportsReceived >= minLamports;

    return NextResponse.json({
      verified,
      cluster,
      recipient: recipientStr,
      lamportsReceived,
      minLamports,
      signature: txSig,
    });
  } catch (e: any) {
    return NextResponse.json({ verified: false, error: e?.message || String(e) }, { status: 500 });
  }
}

