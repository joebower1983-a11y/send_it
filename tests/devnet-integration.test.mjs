/**
 * Send.it Devnet Integration Tests
 * Run: node --test tests/devnet-integration.test.mjs
 *
 * ⚠️ These run against Solana devnet — they cost SOL (devnet)
 */

import { describe, it, before } from "node:test";
import assert from "node:assert/strict";
import { Connection, Keypair, PublicKey, SystemProgram, Transaction, TransactionInstruction, LAMPORTS_PER_SOL, SYSVAR_RENT_PUBKEY } from "@solana/web3.js";
import { TOKEN_PROGRAM_ID, ASSOCIATED_TOKEN_PROGRAM_ID, getAssociatedTokenAddressSync } from "@solana/spl-token";
import { createHash } from "crypto";
import fs from "fs";

// --- Setup ---
const PROGRAM_ID = new PublicKey("HTKq18cATdwCZb6XM66Mhn8JWKCFTrZqH6zU1zip88Zx");
const connection = new Connection("https://api.devnet.solana.com", "confirmed");

const PLATFORM_CONFIG_SEED = Buffer.from("platform_config");
const TOKEN_LAUNCH_SEED = Buffer.from("token_launch");
const PLATFORM_VAULT_SEED = Buffer.from("platform_vault");
const SOL_VAULT_SEED = Buffer.from("sol_vault");
const USER_POSITION_SEED = Buffer.from("user_position");

const [platformConfig] = PublicKey.findProgramAddressSync([PLATFORM_CONFIG_SEED], PROGRAM_ID);
const [platformVault] = PublicKey.findProgramAddressSync([PLATFORM_VAULT_SEED], PROGRAM_ID);

function disc(name) {
  return createHash("sha256").update(`global:${name}`).digest().subarray(0, 8);
}
function encodeString(s) {
  const buf = Buffer.alloc(4 + s.length);
  buf.writeUInt32LE(s.length, 0);
  buf.write(s, 4);
  return buf;
}

let wallet;
try {
  const paths = [
    "sendit-5ive/deployer.json",
    "../sendit-5ive/deployer.json",
    process.env.HOME + "/.config/solana/id.json",
  ];
  for (const p of paths) {
    try {
      wallet = Keypair.fromSecretKey(Uint8Array.from(JSON.parse(fs.readFileSync(p, "utf8"))));
      break;
    } catch {}
  }
} catch {}

async function sendTx(tx, signers) {
  tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
  tx.feePayer = signers[0].publicKey;
  tx.sign(...signers);
  const sig = await connection.sendRawTransaction(tx.serialize());
  await connection.confirmTransaction(sig, "confirmed");
  return sig;
}

// --- Tests ---

describe("Send.it Devnet Integration", () => {
  before(() => {
    assert.ok(wallet, "Wallet not found — need deployer.json or ~/.config/solana/id.json");
  });

  it("should have the program deployed", async () => {
    const info = await connection.getAccountInfo(PROGRAM_ID);
    assert.ok(info, "Program not found on devnet");
    assert.ok(info.executable, "Program should be executable");
  });

  it("should have platform config initialized", async () => {
    const info = await connection.getAccountInfo(platformConfig);
    assert.ok(info, "Platform config PDA not found");
    assert.ok(info.data.length > 0, "Platform config should have data");
  });

  describe("Token Lifecycle", () => {
    let mintKeypair, mint, tokenLaunch, solVault, launchVault;

    before(async () => {
      // Check balance
      const bal = await connection.getBalance(wallet.publicKey);
      assert.ok(bal > 0.1 * LAMPORTS_PER_SOL, `Need devnet SOL. Balance: ${bal / LAMPORTS_PER_SOL}`);
    });

    it("should create a token", async () => {
      mintKeypair = Keypair.generate();
      mint = mintKeypair.publicKey;
      [tokenLaunch] = PublicKey.findProgramAddressSync([TOKEN_LAUNCH_SEED, mint.toBuffer()], PROGRAM_ID);
      [solVault] = PublicKey.findProgramAddressSync([SOL_VAULT_SEED, mint.toBuffer()], PROGRAM_ID);
      launchVault = getAssociatedTokenAddressSync(mint, tokenLaunch, true);

      const data = Buffer.concat([
        disc("create_token"),
        encodeString("Test Token " + Date.now()),
        encodeString("TEST"),
        encodeString("https://send-it-seven-sigma.vercel.app"),
        Buffer.from([0xF4, 0x01]),
      ]);
      const tx = new Transaction().add(new TransactionInstruction({
        keys: [
          { pubkey: tokenLaunch, isSigner: false, isWritable: true },
          { pubkey: mint, isSigner: true, isWritable: true },
          { pubkey: launchVault, isSigner: false, isWritable: true },
          { pubkey: solVault, isSigner: false, isWritable: true },
          { pubkey: platformConfig, isSigner: false, isWritable: true },
          { pubkey: wallet.publicKey, isSigner: true, isWritable: true },
          { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
          { pubkey: ASSOCIATED_TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
          { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
          { pubkey: SYSVAR_RENT_PUBKEY, isSigner: false, isWritable: false },
        ],
        programId: PROGRAM_ID, data,
      }));
      const sig = await sendTx(tx, [wallet, mintKeypair]);
      assert.ok(sig, "Should return tx signature");

      const launchInfo = await connection.getAccountInfo(tokenLaunch);
      assert.ok(launchInfo, "Token launch PDA should exist");
    });

    it("should buy tokens with SOL", async () => {
      // Pre-fund vaults
      const rentExempt = await connection.getMinimumBalanceForRentExemption(0);
      const preTx = new Transaction();
      if (!(await connection.getAccountInfo(solVault)))
        preTx.add(SystemProgram.transfer({ fromPubkey: wallet.publicKey, toPubkey: solVault, lamports: rentExempt }));
      if (!(await connection.getAccountInfo(platformVault)))
        preTx.add(SystemProgram.transfer({ fromPubkey: wallet.publicKey, toPubkey: platformVault, lamports: rentExempt }));
      if (preTx.instructions.length > 0) await sendTx(preTx, [wallet]);

      const buyerAta = getAssociatedTokenAddressSync(mint, wallet.publicKey);
      const [userPosition] = PublicKey.findProgramAddressSync([USER_POSITION_SEED, wallet.publicKey.toBuffer(), mint.toBuffer()], PROGRAM_ID);

      const buyData = Buffer.alloc(16);
      disc("buy").copy(buyData, 0);
      buyData.writeBigUInt64LE(10_000_000n, 8); // 0.01 SOL

      const tx = new Transaction().add(new TransactionInstruction({
        keys: [
          { pubkey: tokenLaunch, isSigner: false, isWritable: true },
          { pubkey: mint, isSigner: false, isWritable: false },
          { pubkey: launchVault, isSigner: false, isWritable: true },
          { pubkey: solVault, isSigner: false, isWritable: true },
          { pubkey: buyerAta, isSigner: false, isWritable: true },
          { pubkey: userPosition, isSigner: false, isWritable: true },
          { pubkey: platformVault, isSigner: false, isWritable: true },
          { pubkey: wallet.publicKey, isSigner: false, isWritable: true },
          { pubkey: platformConfig, isSigner: false, isWritable: true },
          { pubkey: wallet.publicKey, isSigner: true, isWritable: true },
          { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
          { pubkey: ASSOCIATED_TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
          { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
        ],
        programId: PROGRAM_ID, data: buyData,
      }));
      const sig = await sendTx(tx, [wallet]);
      assert.ok(sig, "Buy tx should succeed");

      const tokenBal = await connection.getTokenAccountBalance(buyerAta);
      assert.ok(Number(tokenBal.value.amount) > 0, "Should have received tokens");
    });

    it("should sell tokens for SOL", async () => {
      const buyerAta = getAssociatedTokenAddressSync(mint, wallet.publicKey);
      const [userPosition] = PublicKey.findProgramAddressSync([USER_POSITION_SEED, wallet.publicKey.toBuffer(), mint.toBuffer()], PROGRAM_ID);

      const balBefore = await connection.getBalance(wallet.publicKey);

      const sellData = Buffer.alloc(16);
      disc("sell").copy(sellData, 0);
      sellData.writeBigUInt64LE(1_000_000n, 8); // sell 1M tokens

      const tx = new Transaction().add(new TransactionInstruction({
        keys: [
          { pubkey: tokenLaunch, isSigner: false, isWritable: true },
          { pubkey: mint, isSigner: false, isWritable: false },
          { pubkey: launchVault, isSigner: false, isWritable: true },
          { pubkey: solVault, isSigner: false, isWritable: true },
          { pubkey: buyerAta, isSigner: false, isWritable: true },
          { pubkey: userPosition, isSigner: false, isWritable: true },
          { pubkey: platformVault, isSigner: false, isWritable: true },
          { pubkey: wallet.publicKey, isSigner: false, isWritable: true },
          { pubkey: platformConfig, isSigner: false, isWritable: true },
          { pubkey: wallet.publicKey, isSigner: true, isWritable: true },
          { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
          { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
        ],
        programId: PROGRAM_ID, data: sellData,
      }));
      const sig = await sendTx(tx, [wallet]);
      assert.ok(sig, "Sell tx should succeed");
    });

    it("should have correct token launch state after trades", async () => {
      const info = await connection.getAccountInfo(tokenLaunch);
      assert.ok(info, "Token launch should still exist");
      const data = info.data;
      const creator = new PublicKey(data.subarray(8, 40));
      assert.equal(creator.toBase58(), wallet.publicKey.toBase58(), "Creator should be wallet");
      const tokensSold = data.readBigUInt64LE(48);
      assert.ok(tokensSold > 0n, "Should have tokens sold recorded");
    });
  });

  describe("Error Cases", () => {
    it("should reject re-initialization", async () => {
      const data = Buffer.alloc(8 + 2 + 8);
      disc("initialize_platform").copy(data, 0);
      data.writeUInt16LE(250, 8);
      data.writeBigUInt64LE(100_000_000_000n, 10);
      const tx = new Transaction().add(new TransactionInstruction({
        keys: [
          { pubkey: platformConfig, isSigner: false, isWritable: true },
          { pubkey: wallet.publicKey, isSigner: true, isWritable: true },
          { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
        ],
        programId: PROGRAM_ID, data,
      }));
      tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
      tx.feePayer = wallet.publicKey;
      tx.sign(wallet);
      await assert.rejects(
        () => connection.sendRawTransaction(tx.serialize()),
        "Re-initialization should fail"
      );
    });

    it("should reject buy with 0 SOL", async () => {
      const mintKeypair = Keypair.generate();
      const mint = mintKeypair.publicKey;
      // Buy without creating — should fail
      const [tokenLaunch] = PublicKey.findProgramAddressSync([TOKEN_LAUNCH_SEED, mint.toBuffer()], PROGRAM_ID);
      const [solVault] = PublicKey.findProgramAddressSync([SOL_VAULT_SEED, mint.toBuffer()], PROGRAM_ID);
      const launchVault = getAssociatedTokenAddressSync(mint, tokenLaunch, true);
      const buyerAta = getAssociatedTokenAddressSync(mint, wallet.publicKey);
      const [userPosition] = PublicKey.findProgramAddressSync([USER_POSITION_SEED, wallet.publicKey.toBuffer(), mint.toBuffer()], PROGRAM_ID);

      const buyData = Buffer.alloc(16);
      disc("buy").copy(buyData, 0);
      buyData.writeBigUInt64LE(0n, 8);

      const tx = new Transaction().add(new TransactionInstruction({
        keys: [
          { pubkey: tokenLaunch, isSigner: false, isWritable: true },
          { pubkey: mint, isSigner: false, isWritable: false },
          { pubkey: launchVault, isSigner: false, isWritable: true },
          { pubkey: solVault, isSigner: false, isWritable: true },
          { pubkey: buyerAta, isSigner: false, isWritable: true },
          { pubkey: userPosition, isSigner: false, isWritable: true },
          { pubkey: platformVault, isSigner: false, isWritable: true },
          { pubkey: wallet.publicKey, isSigner: false, isWritable: true },
          { pubkey: platformConfig, isSigner: false, isWritable: true },
          { pubkey: wallet.publicKey, isSigner: true, isWritable: true },
          { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
          { pubkey: ASSOCIATED_TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
          { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
        ],
        programId: PROGRAM_ID, data: buyData,
      }));
      tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
      tx.feePayer = wallet.publicKey;
      tx.sign(wallet);
      await assert.rejects(
        () => connection.sendRawTransaction(tx.serialize()),
        "Buy on non-existent token should fail"
      );
    });
  });
});
