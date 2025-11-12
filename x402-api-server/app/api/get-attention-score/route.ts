import { NextRequest, NextResponse } from 'next/server';
import { fetchSwitchboardPrice } from '@/lib/switchboard';

// Mock x402 payment verification
async function verifyX402Payment(request: NextRequest): Promise<boolean> {
  // In production, this would verify the x402 payment proof
  // For demonstration, we simulate the payment verification
  const x402Header = request.headers.get('x-402-payment');
  const authHeader = request.headers.get('authorization');

  // Mock verification: check if payment header exists
  if (x402Header || authHeader?.includes('Bearer x402-')) {
    // Simulate payment verification delay
    await new Promise(resolve => setTimeout(resolve, 100));
    return true;
  }

  return false;
}

// Mock attention score calculation
function calculateMockAttentionScore(creator: string): number {
  // Generate a consistent but varied score based on creator name
  const hash = creator.split('').reduce((acc, char) => acc + char.charCodeAt(0), 0);
  return 1000 + (hash % 9000); // Returns score between 1000-10000
}

export async function GET(request: NextRequest) {
  // Check for x402 payment
  const paymentVerified = await verifyX402Payment(request);

  if (!paymentVerified) {
    // Return 402 Payment Required with x402 payment instructions
    // Optionally include Switchboard price context for clients
    const sb = await fetchSwitchboardPrice().catch(() => null);
    return NextResponse.json(
      {
        error: 'Payment Required',
        message: 'This endpoint requires x402 payment',
        payment_instructions: {
          method: 'x402',
          price: '0.001 SOL',
          recipient: 'GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop',
          description: 'Access to attention score data'
        },
        oracle_context: sb?.ok
          ? { source: 'switchboard', cluster: sb.cluster, feed: sb.feed, sol_usd: sb.price }
          : undefined
      },
      {
        status: 402,
        headers: {
          'X-402-Payment-Required': 'true',
          'X-402-Price': '0.001',
          'X-402-Currency': 'SOL',
          'X-402-Recipient': 'GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop'
        }
      }
    );
  }

  // Payment verified, return mock attention data
  const creator = request.nextUrl.searchParams.get('creator') || 'default_user';
  const timestamp = Math.floor(Date.now() / 1000);

  // Mock response that simulates real oracle data
  const sb = await fetchSwitchboardPrice().catch(() => null);
  const responseData = {
    status: 'success',
    data: {
      creator: creator,
      attention_score: calculateMockAttentionScore(creator),
      timestamp: timestamp,
      epoch: Math.floor(timestamp / 3600), // Hour-based epochs
      merkle_root: '0x' + Buffer.from('mock_merkle_root_' + timestamp).toString('hex'),
      total_participants: Math.floor(Math.random() * 5000) + 100,
      distribution_available: true,
      oracle_context: sb?.ok
        ? { source: 'switchboard', cluster: sb.cluster, feed: sb.feed, sol_usd: sb.price }
        : undefined
    },
    payment: {
      verified: true,
      method: 'x402',
      amount: '0.001 SOL',
      transaction_id: 'mock_tx_' + Date.now()
    },
    _note: 'This is mock data for demonstration purposes.'
  };

  return NextResponse.json(responseData, {
    headers: {
      'X-Powered-By': 'Attention Oracle x402',
      'X-Oracle-Version': 'v2.0'
    }
  });
}

export async function POST(request: NextRequest) {
  // Handle POST requests for submitting payment proofs
  try {
    const body = await request.json();

    // Mock payment proof validation
    if (body.payment_proof) {
      // Simulate processing
      await new Promise(resolve => setTimeout(resolve, 200));

      return NextResponse.json({
        status: 'success',
        message: 'Payment proof accepted',
        access_granted: true,
        expires_at: Date.now() + 3600000, // 1 hour from now
        api_key: 'x402-demo-' + Buffer.from(Math.random().toString()).toString('base64').slice(0, 16)
      });
    }

    return NextResponse.json(
      { error: 'Invalid payment proof' },
      { status: 400 }
    );
  } catch (error) {
    return NextResponse.json(
      { error: 'Invalid request body' },
      { status: 400 }
    );
  }
}
