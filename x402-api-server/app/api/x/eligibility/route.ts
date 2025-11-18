import { NextRequest, NextResponse } from 'next/server';

const TARGET_ID = process.env.X_TARGET_ID || '1883951442932985856'; // @twzrd_xyz by default

export async function GET(req: NextRequest) {
  try {
    const token = req.cookies.get('x_access_token')?.value;
    if (!token) return NextResponse.json({ ok: true, x_follow: false, note: 'no_token' });

    // Get current user
    const meResp = await fetch('https://api.twitter.com/2/users/me', {
      headers: { Authorization: `Bearer ${token}` },
    });
    const me = await meResp.json();
    const uid = me?.data?.id;
    if (!uid) return NextResponse.json({ ok: true, x_follow: false, note: 'no_user' });

    // Check following (paginate up to one page for demo)
    const fResp = await fetch(`https://api.twitter.com/2/users/${uid}/following?max_results=1000`, {
      headers: { Authorization: `Bearer ${token}` },
    });
    const fJson = await fResp.json();
    const arr = fJson?.data || [];
    const x_follow = Array.isArray(arr) && arr.some((u: any) => u?.id === TARGET_ID);
    return NextResponse.json({ ok: true, x_follow });
  } catch (e: any) {
    return NextResponse.json({ ok: false, error: e?.message || String(e) });
  }
}

