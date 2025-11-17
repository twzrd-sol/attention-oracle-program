/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_SOLANA_NETWORK?: string;
  readonly VITE_SOLANA_RPC?: string;
  readonly VITE_PROGRAM_ID?: string;
  readonly VITE_GATEWAY_URL?: string;
  readonly VITE_TWITCH_CLIENT_ID?: string;
  readonly VITE_TWITCH_REDIRECT_URI?: string;
  readonly VITE_TWITCH_SCOPES?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
