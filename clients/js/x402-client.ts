export type X402Options = {
  baseUrl?: string; // default http://localhost:3000
  bearer?: string;  // simulated x402 token for demo
};

export async function getAttentionScore(creator: string, opts: X402Options = {}) {
  const baseUrl = opts.baseUrl || 'http://localhost:3000';
  const url = `${baseUrl}/api/get-attention-score?creator=${encodeURIComponent(creator)}`;

  // First attempt without payment
  let res = await fetch(url);
  if (res.status === 402) {
    // In production, perform x402 payment per returned invoice
    // For demo, include a mock Bearer token to pass the gate
    res = await fetch(url, {
      headers: {
        Authorization: `Bearer ${opts.bearer || 'x402-mock-payment-token'}`,
        'X-402-Payment': 'mock-payment-proof',
      },
    });
  }
  if (!res.ok) {
    throw new Error(`Request failed: ${res.status}`);
  }
  return res.json();
}

