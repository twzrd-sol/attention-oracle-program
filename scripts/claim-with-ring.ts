// scripts/claim-with-ring.ts
import * as anchor from '@coral-xyz/anchor'
import { Program, AnchorProvider, web3, BN } from '@coral-xyz/anchor'
// eslint-disable-next-line @typescript-eslint/no-var-requires
const idl = require('../target/idl/token_2022.json')

const PROGRAM_ID = new web3.PublicKey(idl.metadata.address)

// Required ENV/CLI args
const MINT_PUBKEY = new web3.PublicKey(process.env.MINT_PUBKEY || '')
const STREAMER_KEY = new web3.PublicKey(process.env.STREAMER_KEY || '')

const epoch = new BN(process.env.EPOCH || '0')
const index = Number(process.env.INDEX || '0')
const amount = new BN(process.env.AMOUNT || '0')
const idStr = process.env.CLAIM_ID || ''
// Proof passed as comma‑separated hex 32‑byte nodes
const proofCsv = process.env.PROOF || ''

function parseProof(csv: string): number[] | Buffer[] {
  if (!csv) return []
  return csv.split(',').map((hex) => Buffer.from(hex.trim().replace(/^0x/, ''), 'hex'))
}

async function main() {
  if (!MINT_PUBKEY || !STREAMER_KEY) throw new Error('Missing MINT_PUBKEY/STREAMER_KEY env')
  if (epoch.isZero()) throw new Error('Missing EPOCH')
  if (!idStr) throw new Error('Missing CLAIM_ID')

  const provider = AnchorProvider.env()
  anchor.setProvider(provider)
  const program = new Program(idl as anchor.Idl, PROGRAM_ID, provider)

  const [protocolState] = web3.PublicKey.findProgramAddressSync(
    [Buffer.from('protocol'), MINT_PUBKEY.toBuffer()],
    PROGRAM_ID,
  )
  const [channelState] = web3.PublicKey.findProgramAddressSync(
    [Buffer.from('channel_state'), MINT_PUBKEY.toBuffer(), STREAMER_KEY.toBuffer()],
    PROGRAM_ID,
  )

  const proofNodes = parseProof(proofCsv)

  const tx = await program.methods
    .claimWithRing(epoch, index, amount, proofNodes, idStr, STREAMER_KEY)
    .accounts({
      claimer: provider.wallet.publicKey,
      protocolState,
      channelState,
      mint: MINT_PUBKEY,
      tokenProgram: anchor.utils.token.TOKEN_2022_PROGRAM_ID,
      associatedTokenProgram: anchor.utils.token.ASSOCIATED_PROGRAM_ID,
      systemProgram: web3.SystemProgram.programId,
    })
    .rpc()

  console.log('Claim tx:', tx)
}

main().catch((e) => {
  console.error(e)
  process.exit(1)
})

