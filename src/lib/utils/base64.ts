export function u8ToBase64(u8: Uint8Array): string {
  let bin = '';
  const chunkSize = 0x8000;
  for (let i = 0; i < u8.length; i += chunkSize) {
    const sub = u8.subarray(i, i + chunkSize);
    bin += String.fromCharCode.apply(null, Array.from(sub) as unknown as number[]);
  }
  return typeof btoa !== 'undefined' ? btoa(bin) : Buffer.from(u8).toString('base64');
}

export function base64ToU8(b64: string): Uint8Array {
  if (typeof atob !== 'undefined') {
    const bin = atob(b64);
    const out = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
    return out;
  }
  // Node fallback for SSR/tests
  return Buffer.from(b64, 'base64');
}

