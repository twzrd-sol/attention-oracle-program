import { NextResponse } from 'next/server';
import { fetchSwitchboardPrice } from '@/lib/switchboard';

export async function GET() {
  const result = await fetchSwitchboardPrice();
  const status = result.ok ? 200 : 503;
  return NextResponse.json(
    {
      source: 'switchboard',
      cluster: result.cluster,
      feed: result.feed,
      ok: result.ok,
      price: result.price,
      updatedRecently: result.updatedRecently ?? false,
      error: result.error,
    },
    {
      status,
      headers: {
        'X-Oracle-Source': 'Switchboard',
        'X-Oracle-Cluster': result.cluster,
      },
    }
  );
}

