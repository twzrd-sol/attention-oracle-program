import { NextRequest, NextResponse } from 'next/server';

export async function GET(req: NextRequest) {
  const url = new URL(req.url);
  const code = url.searchParams.get('code');
  const state = url.searchParams.get('state');
  const clientId = process.env.X_CLIENT_ID || '';
  const clientSecret = process.env.X_CLIENT_SECRET || '';
  const redirectUri = process.env.X_REDIRECT_URI || `${url.origin}/api/auth/x/callback`;
  if (!code || !state) {
    return NextResponse.json({ ok: false, error: 'Missing code/state' }, { status: 400 });
  }
  const cookies = req.cookies;
  const expectedState = cookies.get('x_oauth_state')?.value;
  const verifier = cookies.get('x_pkce_verifier')?.value;
  if (!expectedState || expectedState !== state || !verifier) {
    return NextResponse.json({ ok: false, error: 'Invalid state or missing verifier' }, { status: 400 });
  }
  if (!clientId || !clientSecret) {
    // Redirect back with stub status
    const r = NextResponse.redirect(`${url.origin}/public/claim-v2.html?x=stub`);
    r.cookies.delete('x_oauth_state');
    r.cookies.set('x_oauth_stub', '1', { httpOnly: true, path: '/', secure: true, sameSite: 'lax' });
    return r;
  }
  try {
    const body = new URLSearchParams({
      code,
      grant_type: 'authorization_code',
      client_id: clientId,
      redirect_uri: redirectUri,
      code_verifier: verifier,
    });
    const resp = await fetch('https://api.twitter.com/2/oauth2/token', {
      method: 'POST',
      headers: { 'Content-Type': 'application/x-www-form-urlencoded', Authorization: 'Basic ' + Buffer.from(`${clientId}:${clientSecret}`).toString('base64') },
      body,
    });
    const json = await resp.json();
    const r = NextResponse.redirect(`${url.origin}/public/claim-v2.html?x=ok`);
    r.cookies.delete('x_oauth_state');
    r.cookies.set('x_access_token', json.access_token || '', { httpOnly: true, path: '/', secure: true, sameSite: 'lax' });
    return r;
  } catch (e: any) {
    return NextResponse.json({ ok: false, error: e?.message || 'OAuth exchange failed' }, { status: 500 });
  }
}

