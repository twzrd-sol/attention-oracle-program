import { NextRequest, NextResponse } from 'next/server';
import crypto from 'crypto';

function base64url(input: Buffer) {
  return input.toString('base64').replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

export async function GET(req: NextRequest) {
  const clientId = process.env.X_CLIENT_ID;
  const redirectUri = process.env.X_REDIRECT_URI || `${req.nextUrl.origin}/api/auth/x/callback`;
  if (!clientId) {
    return NextResponse.json({ ok: false, error: 'X_CLIENT_ID not configured' }, { status: 501 });
  }

  const verifier = base64url(crypto.randomBytes(32));
  const challenge = base64url(crypto.createHash('sha256').update(verifier).digest());
  const state = base64url(crypto.randomBytes(16));

  const authUrl = new URL('https://twitter.com/i/oauth2/authorize');
  authUrl.searchParams.set('response_type', 'code');
  authUrl.searchParams.set('client_id', clientId);
  authUrl.searchParams.set('redirect_uri', redirectUri);
  authUrl.searchParams.set('scope', 'tweet.read users.read follows.read offline.access');
  authUrl.searchParams.set('state', state);
  authUrl.searchParams.set('code_challenge', challenge);
  authUrl.searchParams.set('code_challenge_method', 'S256');

  const res = NextResponse.redirect(authUrl.toString());
  res.cookies.set('x_pkce_verifier', verifier, { httpOnly: true, secure: true, sameSite: 'lax', path: '/' });
  res.cookies.set('x_oauth_state', state, { httpOnly: true, secure: true, sameSite: 'lax', path: '/' });
  return res;
}

