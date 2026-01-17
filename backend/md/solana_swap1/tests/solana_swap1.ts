import * as anchor from "@anchor-lang/core";
import { Program } from "@anchor-lang/core";
import { SolanaSwap1 } from "../target/types/solana_swap1";

describe("solana_swap1", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  const program = anchor.workspace.solanaSwap1 as Program<SolanaSwap1>;

  it("Is initialized!", async () => {
    // Add your test here.
    const tx = await program.methods.initialize().rpc();
    console.log("Your transaction signature", tx);
  });
});
