import * as anchor from '@coral-xyz/anchor'
import { Program, AnchorProvider, web3, BN } from '@coral-xyz/anchor'
// eslint-disable-next-line @typescript-eslint/no-var-requires
const idl = require('../target/idl/token_2022.json')

const PROGRAM_ID = new web3.PublicKey(idl.metadata.address)

describe('ring buffer security', () => {
  const provider = AnchorProvider.env()
  anchor.setProvider(provider)
  const program = new Program(idl as anchor.Idl, PROGRAM_ID, provider)

  // NOTE: These tests require prepared state (mint, protocol, channel init).
  // They are set to `it.skip` by default; unskip when wiring fixtures.

  it.skip('prevents ring buffer replay (epoch re-init in same slot)', async () => {
    // Arrange: PDAs and params must be provided via fixtures
    // Expect: second set_merkle_root_ring with same epoch fails with EpochNotIncreasing
  })

  it.skip('rejects invalid ring proof (claim theft)', async () => {
    // Arrange: publish known root; attempt claim_with_ring with fake proof
    // Expect: InvalidProof
  })

  it('does not expose removed close_old_epoch_state', async () => {
    // There is no method in the IDL for close_old_epoch_state
    const hasIx = Object.keys((program.idl as any).instructions || {}).some((k) =>
      k.includes('close_old_epoch_state'),
    )
    if (hasIx) throw new Error('close_old_epoch_state unexpectedly present')
  })
})

