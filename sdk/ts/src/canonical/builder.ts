/**
 * Commitment builder — turns validated CanonicalInput / CanonicalOutput
 * into the four artefacts every ThoughtRecord needs:
 *
 *  1. `inputCommitment`    — blake3(canonical_input)   (§4.2)
 *  2. `outputCommitment`   — blake3(canonical_output)  (§4.3)
 *  3. `traceManifest`      — ordered `[name, hash]` pairs of trace bundle parts
 *  4. `traceManifestHash`  — blake3(canonical(traceManifest)); the value that
 *                            gets stored as `trace_uri_hash`'s preimage
 *                            sibling in the bundle. (§4.5)
 *
 * The trace manifest is intentionally NOT a Merkle tree: a flat ordered list
 * is both simpler to verify and matches the spec's `manifest.cbor` layout
 * verbatim (see §4.5). Watchers iterate, recompute each part's hash from
 * the on-disk file, and compare.
 */

import { canonicalEncode } from "./cbor.js";
import { blake3_256, hashCommitment } from "./hash.js";
import {
  validateCanonicalInput,
  validateCanonicalOutput,
  type CanonicalInput,
  type CanonicalOutput,
} from "./schema.js";

/** A single entry in the trace bundle manifest. */
export interface TraceManifestEntry {
  /** Path inside the trace bundle, e.g. `tool_io/00_get_price.req.json`. */
  name: string;
  /** blake3-256 of the file's exact bytes. */
  hash: Uint8Array;
}

/** Inputs to `buildThoughtCommitment`. */
export interface BuildThoughtCommitmentArgs {
  input: CanonicalInput;
  output: CanonicalOutput;
  /**
   * Ordered list of trace bundle parts. Order is significant — watchers
   * must use the same order to reproduce the manifest hash. Typical order:
   *   1. canonical_input.cbor
   *   2. canonical_output.cbor
   *   3. raw_provider_response.json
   *   4. tool_io/* (lexicographic, zero-padded)
   *   5. memory_proof.cbor
   *   6. attestation.bin (if present)
   */
  traceParts: TraceManifestEntry[];
}

/** Output of `buildThoughtCommitment`. */
export interface ThoughtCommitment {
  inputCommitment: Uint8Array;
  outputCommitment: Uint8Array;
  traceManifest: TraceManifestEntry[];
  traceManifestHash: Uint8Array;
}

/**
 * Build the four commitment artefacts for a single thought. Validation is
 * performed up-front; we call the canonical CBOR encoder ONLY through
 * `hashCommitment` so the on-chain bytes and watcher-recomputed bytes are
 * derived from the exact same code path.
 */
export function buildThoughtCommitment(
  args: BuildThoughtCommitmentArgs,
): ThoughtCommitment {
  const validatedInput = validateCanonicalInput(args.input);
  const validatedOutput = validateCanonicalOutput(args.output);

  const inputCommitment = hashCommitment(validatedInput);
  const outputCommitment = hashCommitment(validatedOutput);

  // Defensive copy of trace parts: caller may not realise we hash them in
  // order, and we don't want surprise mutations changing the manifest.
  const traceManifest: TraceManifestEntry[] = args.traceParts.map((p, i) => {
    if (typeof p.name !== "string" || p.name.length === 0) {
      throw new Error(`trace manifest entry ${i}: name must be a non-empty string`);
    }
    if (!(p.hash instanceof Uint8Array) || p.hash.length !== 32) {
      throw new Error(
        `trace manifest entry ${i} (${p.name}): hash must be 32 bytes`,
      );
    }
    return { name: p.name, hash: new Uint8Array(p.hash) };
  });

  // Manifest is encoded as a top-level CBOR array of [name, hash] tuples.
  // Length-first key sort doesn't apply (no maps at the top level), but
  // CDE still gives us deterministic length encodings + smallest ints.
  const manifestEncoded = canonicalEncode(
    traceManifest.map((p) => [p.name, p.hash] as const),
  );
  const traceManifestHash = blake3_256(manifestEncoded);

  return {
    inputCommitment,
    outputCommitment,
    traceManifest,
    traceManifestHash,
  };
}
