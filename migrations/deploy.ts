// Anchor migration scaffold. Empty by design — deploy logic lives in CI.
import * as anchor from "@coral-xyz/anchor";

module.exports = async function (provider: anchor.AnchorProvider) {
  anchor.setProvider(provider);
};
