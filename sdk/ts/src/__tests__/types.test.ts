/**
 * Sanity tests for the TS account-type mirrors.
 *
 * No (de)serializer ships in this phase — those land with the Anchor IDL
 * client wrapper. We just verify enum constants are stable and assignable
 * to their type aliases, and that a structurally-correct fixture
 * type-checks against the interface shapes.
 */

import { describe, expect, it } from "vitest";
import {
  ChallengeClaim,
  EquivClass,
  ModelClass,
  ThoughtStatus,
  type AgentProfile,
  type Challenge,
  type ModelRegistry,
  type Policy,
  type ThoughtRecord,
  type VrfRequest,
} from "../types.js";
import { PublicKey } from "@solana/web3.js";

const ZERO_KEY = new PublicKey(new Uint8Array(32));
const ZERO_HASH = new Uint8Array(32);

describe("enum discriminants", () => {
  it("ThoughtStatus values match on-chain u8 representation", () => {
    expect(ThoughtStatus.Pending).toBe(0);
    expect(ThoughtStatus.Challenged).toBe(1);
    expect(ThoughtStatus.Finalized).toBe(2);
    expect(ThoughtStatus.Slashed).toBe(3);
  });

  it("ModelClass values match spec §4.4", () => {
    expect(ModelClass.OpenWeights).toBe(0);
    expect(ModelClass.Hosted).toBe(1);
    expect(ModelClass.TeeFronted).toBe(2);
  });

  it("EquivClass values match spec §6.3", () => {
    expect(EquivClass.Strict).toBe(0);
    expect(EquivClass.StructuralJSON).toBe(1);
    expect(EquivClass.SemanticCommittee).toBe(2);
    expect(EquivClass.AnyOfN).toBe(3);
  });

  it("ChallengeClaim values cover all spec §5.1 cases", () => {
    expect(ChallengeClaim.ModelMismatch).toBe(0);
    expect(ChallengeClaim.OutputMismatch).toBe(1);
    expect(ChallengeClaim.InputOmission).toBe(2);
    expect(ChallengeClaim.Replay).toBe(3);
    expect(ChallengeClaim.StaleVRF).toBe(4);
  });
});

describe("interface shapes", () => {
  it("AgentProfile fixture type-checks", () => {
    const a: AgentProfile = {
      operator: ZERO_KEY,
      stakeVault: ZERO_KEY,
      stakeAmount: 0n,
      reputation: 0n,
      activeThoughts: 0,
      cooldownUntil: 0n,
      bump: 255,
    };
    expect(a.bump).toBe(255);
  });

  it("ThoughtRecord fixture type-checks", () => {
    const t: ThoughtRecord = {
      agent: ZERO_KEY,
      modelId: ZERO_HASH,
      inputCommitment: ZERO_HASH,
      outputCommitment: ZERO_HASH,
      traceUriHash: ZERO_HASH,
      vrfSeed: ZERO_HASH,
      policyId: ZERO_HASH,
      slot: 312000000n,
      actionPda: ZERO_KEY,
      status: ThoughtStatus.Pending,
      attestationVerified: false,
      challengeDeadlineSlot: 312000150n,
      consumedCount: 0,
      vrfNonceIdx: 1n,
      bump: 255,
      pad: new Uint8Array(7),
    };
    expect(t.status).toBe(0);
  });

  it("ModelRegistry / Challenge / Policy / VrfRequest fixtures type-check", () => {
    const m: ModelRegistry = {
      modelId: ZERO_HASH,
      class: ModelClass.OpenWeights,
      verifierPubkey: ZERO_KEY,
      teeRootCa: ZERO_KEY,
      registeredBy: ZERO_KEY,
      bump: 255,
    };
    const c: Challenge = {
      challenger: ZERO_KEY,
      bond: 1_000_000n,
      claim: ChallengeClaim.OutputMismatch,
      evidenceUriHash: ZERO_HASH,
      openedAtSlot: 312000050n,
      resolved: false,
      bump: 255,
    };
    const p: Policy = {
      policyId: ZERO_HASH,
      schemaUriHash: ZERO_HASH,
      equivClass: EquivClass.Strict,
      maxInferenceMs: 30_000,
      challengeWindowSlots: 150n,
      bondMin: 1_000_000n,
      allowedModels: [],
      bump: 255,
    };
    const v: VrfRequest = {
      agent: ZERO_KEY,
      nonceIdx: 0n,
      seed: ZERO_HASH,
      requestedSlot: 311999000n,
      fulfilledSlot: 311999002n,
      consumed: false,
      bump: 255,
    };
    expect([m.bump, c.bump, p.bump, v.bump]).toEqual([255, 255, 255, 255]);
  });
});
