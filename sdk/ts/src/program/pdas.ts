/**
 * PDA derivation helpers for `pot_program`.
 *
 * Every PDA used by the program is enumerated here so the SDK never has to
 * reach into `@coral-xyz/anchor` IDL code that doesn't exist yet. Keep these
 * 1:1 with the program's `seeds = [...]` declarations.
 */

import { PublicKey } from "@solana/web3.js";

/** Encode a u64 as little-endian 8 bytes — matches Solana wire format. */
export function u64Le(n: bigint): Uint8Array {
  const out = new Uint8Array(8);
  const view = new DataView(out.buffer);
  view.setBigUint64(0, n, true);
  return out;
}

export function agentPda(operator: PublicKey, programId: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [new TextEncoder().encode("agent"), operator.toBytes()],
    programId,
  );
}

export function stakeVaultPda(agent: PublicKey, programId: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [new TextEncoder().encode("vault"), agent.toBytes()],
    programId,
  );
}

export function modelPda(modelId: Uint8Array, programId: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [new TextEncoder().encode("model"), modelId],
    programId,
  );
}

export function policyPda(policyId: Uint8Array, programId: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [new TextEncoder().encode("policy"), policyId],
    programId,
  );
}

export function vrfRequestPda(
  agent: PublicKey,
  nonceIdx: bigint,
  programId: PublicKey,
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [new TextEncoder().encode("vrf"), agent.toBytes(), u64Le(nonceIdx)],
    programId,
  );
}

export function thoughtPda(
  agent: PublicKey,
  vrfNonceIdx: bigint,
  programId: PublicKey,
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [new TextEncoder().encode("thought"), agent.toBytes(), u64Le(vrfNonceIdx)],
    programId,
  );
}

export function challengePda(
  thought: PublicKey,
  challenger: PublicKey,
  programId: PublicKey,
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [new TextEncoder().encode("challenge"), thought.toBytes(), challenger.toBytes()],
    programId,
  );
}

export function bondVaultPda(challenge: PublicKey, programId: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [new TextEncoder().encode("bond"), challenge.toBytes()],
    programId,
  );
}
