const TOKEN_KEY = 'twzrd:twitch_access_token'

const TWITCH_AUTH_BASE = 'https://id.twitch.tv/oauth2/authorize'

const getClientId = () => {
  const id = import.meta.env.VITE_TWITCH_CLIENT_ID
  if (!id) throw new Error('VITE_TWITCH_CLIENT_ID missing')
  return id
}

const getRedirectUri = () => import.meta.env.VITE_TWITCH_REDIRECT_URI || window.location.origin

const getScopes = () => import.meta.env.VITE_TWITCH_SCOPES || 'user:read:email'

export function buildTwitchAuthUrl(state = 'twzrd-binding'): string {
  const url = new URL(TWITCH_AUTH_BASE)
  url.searchParams.set('client_id', getClientId())
  url.searchParams.set('redirect_uri', getRedirectUri())
  url.searchParams.set('response_type', 'token')
  url.searchParams.set('scope', getScopes())
  url.searchParams.set('state', state)
  url.searchParams.set('force_verify', 'true')
  return url.toString()
}

export function extractTokenFromHash(hash: string): string | null {
  if (!hash || !hash.startsWith('#')) return null
  const params = new URLSearchParams(hash.substring(1))
  const token = params.get('access_token')
  return token && token.trim().length > 0 ? token : null
}

export function storeTwitchToken(token: string) {
  if (token) localStorage.setItem(TOKEN_KEY, token)
}

export function getStoredTwitchToken(): string | null {
  return localStorage.getItem(TOKEN_KEY)
}

export function clearTwitchToken() {
  localStorage.removeItem(TOKEN_KEY)
}

export function removeTokenFromUrl() {
  if (window.location.hash.includes('access_token')) {
    window.history.replaceState(null, '', window.location.pathname + window.location.search)
  }
}
