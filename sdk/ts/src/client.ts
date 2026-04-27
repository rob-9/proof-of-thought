/**
 * `ProofOfThought` — high-level client wrapping the on-chain `pot_program`.
 *
 * Wraps the program's instruction set behind ergonomic methods. Builds and
 * (optionally) submits Solana transactions via a pluggable `Submitter` so
 * the SDK can be driven against real RPCs, against a local validator, or
 * against an in-memory stub for tests / demos.
 *
 * Typical flow (see `demo/swap-agent` for an end-to-end example):
 *
 * ```ts
 * const pot = new ProofOfThought({
 *   programId,
 *   submitter: rpcSubmitter,
 *   policyId,
 *   modelId,
 * });
 * const seed = await pot.requestVRF(operator);
 * const result = await pot.think(seed, async (ctx) => { ... });
 * const thoughtPda = await pot.submit(result, { actionPda });
 * ```
 *
 * The submitter abstraction keeps the client free of `@solana/web3.js`
 * connection plumbing — that's the consumer's concern.
 */

import { PublicKey } from "@solana/web3.js";

import {
  buildThoughtCommitment,
  type CanonicalInput,
  type CanonicalOutput,
} from "./canonical/index.js";
import { hashCommitment } from "./canonical/hash.js";
import {
  IX,
  agentPda,
  challengePda,
  encodeChallengeArgs,
  encodeRequestVrfArgs,
  encodeSubmitThoughtArgs,
  modelPda,
  policyPda,
  stakeVaultPda,
  thoughtPda,
  vrfRequestPda,
  bondVaultPda,
  type ThoughtRecordArgs as IxThoughtArgs,
} from "./program/index.js";
import { ChallengeClaim } from "./types.js";

// ---------------------------------------------------------------------------
// Submitter abstraction
// ---------------------------------------------------------------------------

export interface AccountMeta {
  pubkey: PublicKey;
  isSigner: boolean;
  isWritable: boolean;
}

export interface InstructionRequest {
  programId: PublicKey;
  data: Uint8Array;
  accounts: AccountMeta[];
}

/** Pluggable submission backend. */
export interface Submitter {
  /** Build, sign, and send a single-instruction transaction. */
  submit(ix: InstructionRequest): Promise<string>;
}

/** In-memory submitter for tests/demos. Records instructions; never errors. */
export class MemorySubmitter implements Submitter {
  public readonly submitted: InstructionRequest[] = [];
  private counter = 0;

  async submit(ix: InstructionRequest): Promise<string> {
    this.submitted.push(ix);
    this.counter += 1;
    return `mem-tx-${this.counter}`;
  }
}

// ---------------------------------------------------------------------------
// Client config + result types
// ---------------------------------------------------------------------------

export interface ProofOfThoughtConfig {
  programId: PublicKey;
  submitter: Submitter;
  policyId: Uint8Array;
  modelId: Uint8Array;
  /** Treasury pubkey from the policy account. Cached for fast tx assembly. */
  treasury?: PublicKey;
  /** Resolver pubkey from the policy account. */
  resolver?: PublicKey;
}

export interface VrfHandle {
  seed: Uint8Array;
  nonceIdx: bigint;
  vrfRequestPda: PublicKey;
}

export interface ThinkContext {
  seed: Uint8Array;
  nonceIdx: bigint;
}

export interface ThinkResult {
  input: CanonicalInput;
  output: CanonicalOutput;
  inputCommitment: Uint8Array;
  outputCommitment: Uint8Array;
  traceManifestHash: Uint8Array;
}

export interface SubmitArgs {
  /** PDA of the downstream action this thought gates. */
  actionPda: PublicKey;
  /** Where the trace bundle lives (e.g. `ar://...`). */
  traceUri: string;
}

export interface SubmitResult {
  thoughtPda: PublicKey;
  signature: string;
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

export class ProofOfThought {
  constructor(public readonly cfg: ProofOfThoughtConfig) {}

  /**
   * Request a fresh VRF seed for this agent.
   *
   * MVP semantics: caller supplies the seed (Pyth Entropy CPI is stubbed
   * in the program). The returned `VrfHandle` is what `think` and `submit`
   * thread through their flows.
   */
  async requestVRF(
    operator: PublicKey,
    nonceIdx: bigint,
    seed: Uint8Array,
  ): Promise<VrfHandle> {
    if (seed.length !== 32) throw new Error("seed must be 32 bytes");
    const [agent] = agentPda(operator, this.cfg.programId);
    const [vrf] = vrfRequestPda(agent, nonceIdx, this.cfg.programId);

    const data = new Uint8Array(8 + 8 + 32);
    data.set(IX.requestVrf, 0);
    data.set(encodeRequestVrfArgs(nonceIdx, seed), 8);

    await this.cfg.submitter.submit({
      programId: this.cfg.programId,
      data,
      accounts: [
        { pubkey: operator, isSigner: true, isWritable: true },
        { pubkey: agent, isSigner: false, isWritable: true },
        { pubkey: vrf, isSigner: false, isWritable: true },
        { pubkey: SYSTEM_PROGRAM_ID, isSigner: false, isWritable: false },
      ],
    });

    return { seed, nonceIdx, vrfRequestPda: vrf };
  }

  /**
   * Drive an inference under PoT discipline.
   *
   * The user supplies a callback that takes a `ThinkContext` (containing
   * the fresh seed) and returns a `{ input, output }` pair. The client
   * computes commitments and the trace manifest hash; the caller then
   * uploads the trace bundle off-chain and calls `submit`.
   */
  async think(
    handle: VrfHandle,
    inference: (ctx: ThinkContext) => Promise<{ input: CanonicalInput; output: CanonicalOutput }>,
  ): Promise<ThinkResult> {
    const { input, output } = await inference({ seed: handle.seed, nonceIdx: handle.nonceIdx });
    if (!bytesEqual(input.vrf_seed, handle.seed)) {
      throw new Error(
        "canonical input.vrf_seed must equal the VRF handle's seed — agent is not binding freshness",
      );
    }
    // Trace bundle here is just the canonical input + output; richer bundles
    // (raw provider response, tool I/O, memory proof, attestation) are
    // assembled by the agent and re-hashed by the caller before storage.
    const built = buildThoughtCommitment({
      input,
      output,
      traceParts: [
        { name: "canonical_input.cbor", hash: hashCommitment(input) },
        { name: "canonical_output.cbor", hash: hashCommitment(output) },
      ],
    });
    return {
      input,
      output,
      inputCommitment: built.inputCommitment,
      outputCommitment: built.outputCommitment,
      traceManifestHash: built.traceManifestHash,
    };
  }

  /**
   * Submit the on-chain commitment for a `ThinkResult`.
   *
   * `traceUri` should already be populated — the trace bundle must be
   * uploaded to Arweave / Shadow Drive / etc. before this call so a
   * watcher can fetch it during the challenge window.
   */
  async submit(
    operator: PublicKey,
    handle: VrfHandle,
    result: ThinkResult,
    args: SubmitArgs,
  ): Promise<SubmitResult> {
    const [agent] = agentPda(operator, this.cfg.programId);
    const [model] = modelPda(this.cfg.modelId, this.cfg.programId);
    const [policy] = policyPda(this.cfg.policyId, this.cfg.programId);
    const [thought] = thoughtPda(agent, handle.nonceIdx, this.cfg.programId);

    const ixArgs: IxThoughtArgs = {
      modelId: this.cfg.modelId,
      inputCommitment: result.inputCommitment,
      outputCommitment: result.outputCommitment,
      traceUriHash: hashCommitment(args.traceUri),
      vrfSeed: handle.seed,
      policyId: this.cfg.policyId,
      actionPda: args.actionPda,
      vrfNonceIdx: handle.nonceIdx,
    };

    const argsBytes = encodeSubmitThoughtArgs(ixArgs, args.traceUri);
    const data = new Uint8Array(8 + argsBytes.length);
    data.set(IX.submitThought, 0);
    data.set(argsBytes, 8);

    const sig = await this.cfg.submitter.submit({
      programId: this.cfg.programId,
      data,
      accounts: [
        { pubkey: operator, isSigner: true, isWritable: true },
        { pubkey: agent, isSigner: false, isWritable: true },
        { pubkey: model, isSigner: false, isWritable: false },
        { pubkey: policy, isSigner: false, isWritable: false },
        { pubkey: handle.vrfRequestPda, isSigner: false, isWritable: true },
        { pubkey: thought, isSigner: false, isWritable: true },
        { pubkey: SYSTEM_PROGRAM_ID, isSigner: false, isWritable: false },
      ],
    });

    return { thoughtPda: thought, signature: sig };
  }

  /**
   * File a fraud challenge against a thought. Used by watchers, not agents.
   *
   * The bond is debited from `bondVaultPda(challenge, programId)` — the
   * caller (challenger) must fund that vault separately before this call,
   * or via a CPI in a wrapper program.
   */
  async challenge(
    challenger: PublicKey,
    thought: PublicKey,
    claim: ChallengeClaim,
    bond: bigint,
    evidenceUriHash: Uint8Array,
  ): Promise<{ challengePda: PublicKey; signature: string }> {
    const [challenge] = challengePda(thought, challenger, this.cfg.programId);
    const [bondVault] = bondVaultPda(challenge, this.cfg.programId);

    const argsBytes = encodeChallengeArgs(claim, bond, evidenceUriHash);
    const data = new Uint8Array(8 + argsBytes.length);
    data.set(IX.challenge, 0);
    data.set(argsBytes, 8);

    const sig = await this.cfg.submitter.submit({
      programId: this.cfg.programId,
      data,
      accounts: [
        { pubkey: challenger, isSigner: true, isWritable: true },
        { pubkey: thought, isSigner: false, isWritable: true },
        { pubkey: challenge, isSigner: false, isWritable: true },
        { pubkey: bondVault, isSigner: false, isWritable: true },
        { pubkey: SYSTEM_PROGRAM_ID, isSigner: false, isWritable: false },
      ],
    });

    return { challengePda: challenge, signature: sig };
  }

  /**
   * Compute the PDAs the consumer program needs for `consume_thought` CPI.
   * Returned account metas are shaped for inclusion in the consumer's tx.
   */
  consumeThoughtAccounts(operator: PublicKey, thought: PublicKey, actionPda: PublicKey): {
    accounts: AccountMeta[];
    data: Uint8Array;
  } {
    const data = new Uint8Array(8);
    data.set(IX.consumeThought, 0);
    return {
      data,
      accounts: [
        { pubkey: thought, isSigner: false, isWritable: true },
        { pubkey: actionPda, isSigner: false, isWritable: false },
        { pubkey: operator, isSigner: true, isWritable: false },
      ],
    };
  }
}

// ---------------------------------------------------------------------------
// Internals
// ---------------------------------------------------------------------------

const SYSTEM_PROGRAM_ID = new PublicKey("11111111111111111111111111111111");

function bytesEqual(a: Uint8Array, b: Uint8Array): boolean {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return false;
  return true;
}
