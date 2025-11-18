import type { Connection, PublicKey } from '@solana/web3.js';
import SwitchboardProgram from '@switchboard-xyz/sbv2-lite';

export async function getSwitchboardProgram(connection: Connection) {
  return await SwitchboardProgram.load(connection);
}

export async function decodeLatestAggregatorValue(program: SwitchboardProgram, pubkey: PublicKey) {
  const raw = await program.fetchAggregatorLatestValue(pubkey);
  return { price: raw.value, slot: raw.slot };
}

