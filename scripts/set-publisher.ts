// scripts/set-publisher.ts
import * as anchor from '@coral-xyz/anchor'
import { Program, AnchorProvider, web3 } from '@coral-xyz/anchor'
// Adjust path after `anchor build`: target/types/token_2022
// eslint-disable-next-line @typescript-eslint/no-var-requires
const idl = require('../target/idl/token_2022.json')

const PROGRAM_ID = new web3.PublicKey(idl.metadata.address)

// ENV
// MINT_PUBKEY is required via env or CLI
const MINT_PUBKEY = new web3.PublicKey(process.env.MINT_PUBKEY || '')
const NEW_PUBLISHER = process.argv[2]

async function main() {
  if (!MINT_PUBKEY) throw new Error('Missing MINT_PUBKEY env')
  if (!NEW_PUBLISHER) throw new Error('Usage: tsx scripts/set-publisher.ts <NEW_PUBLISHER_PUBKEY>')

  const provider = AnchorProvider.env()
  anchor.setProvider(provider)

  const program = new Program(idl as anchor.Idl, PROGRAM_ID, provider)

  const [protocolState] = web3.PublicKey.findProgramAddressSync(
    [Buffer.from('protocol'), MINT_PUBKEY.toBuffer()],
    PROGRAM_ID,
  )

  const tx = await program.methods
    .updatePublisherOpen(new web3.PublicKey(NEW_PUBLISHER))
    .accounts({
      admin: provider.wallet.publicKey,
      protocolState,
    })
    .rpc()

  console.log('ProtocolState:', protocolState.toBase58())
  console.log('New publisher:', NEW_PUBLISHER)
  console.log('Signature:', tx)
}

main().catch((e) => {
  console.error(e)
  process.exit(1)
})

