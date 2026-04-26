/**
 * Public surface for canonicalization + commitment.
 *
 * Consumers should `import { ... } from "@canteen/pot/canonical"` rather
 * than reaching into individual modules — internal layout may change.
 */

export {
  canonicalEncode,
  canonicalDecode,
  CanonicalEncodingError,
} from "./cbor.js";

export {
  blake3_256,
  hashCommitment,
  hashHex,
  toHex,
  fromHex,
  COMMITMENT_BYTES,
} from "./hash.js";

export {
  CanonicalSchemaError,
  validateCanonicalInput,
  validateCanonicalOutput,
  validateMessage,
  validateToolCall,
  validateToolIntent,
  validateSampling,
  type CanonicalInput,
  type CanonicalOutput,
  type Message,
  type ToolCall,
  type ToolIntent,
  type Sampling,
} from "./schema.js";

export {
  buildThoughtCommitment,
  type BuildThoughtCommitmentArgs,
  type ThoughtCommitment,
  type TraceManifestEntry,
} from "./builder.js";
