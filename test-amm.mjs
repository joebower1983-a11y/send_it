#!/usr/bin/env node
/**
 * test-amm.mjs — Full AMM lifecycle test on devnet
 * 
 * Flow:
 *   1. Init platform (skip if exists)
 *   2. Create token (with Metaplex metadata)
 *   3. Fund SOL vault + buy enough to hit migration threshold
 *   4. Create pool (bonding curve → AMM graduation)
 *   5. Swap SOL → tokens (buy via AMM)
 *   6. Swap tokens → SOL (sell via AMM)
 *   7. Add liquidity
 *   8. Remove liquidity
 */

import {
  Connection, Keypair, PublicKey, SystemProgram, Transaction,
  SYSVAR_RENT_PUBKEY, sendAndConfirmTransaction, LAMPORTS_PER_SOL,
  ComputeBudgetProgram,
} from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID, ASSOCIATED_TOKEN_PROGRAM_ID,
  getAssociatedTokenAddressSync, getOrCreateAssociatedTokenAccount,
} from "@solana/spl-token";
import BN from "bn.js";
import borsh from "borsh";
import fs from "fs";

const RPC = "https://api.devnet.solana.com";
const conn = new Connection(RPC, "confirmed");
const PROGRAM_ID = new PublicKey("HTKq18cATdwCZb6XM66Mhn8JWKCFTrZqH6zU1zip88Zx");
const MPL_TOKEN_METADATA_ID = new PublicKey("metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s");

// Seeds
const PLATFORM_CONFIG_SEED = Buffer.from("platform_config");
const TOKEN_LAUNCH_SEED = Buffer.from("token_launch");
const USER_POSITION_SEED = Buffer.from("user_position");
const PLATFORM_VAULT_SEED = Buffer.from("platform_vault");
const SOL_VAULT_SEED = Buffer.from("sol_vault");
const POOL_SEED = Buffer.from("amm_pool");
const POOL_SOL_VAULT_SEED = Buffer.from("pool_sol_vault");
const LP_MINT_SEED = Buffer.from("lp_mint");

const wallet = Keypair.fromSecretKey(
  Uint8Array.from(JSON.parse(fs.readFileSync("/home/joebower1983/.openclaw/workspace/sendit-5ive/deployer.json","utf8")))
);
console.log("Wallet:", wallet.publicKey.toBase58());

// ── Helpers ──

import { createHash } from "crypto";
function anchorDisc(name) {
  return createHash("sha256").update(`global:${name}`).digest().subarray(0, 8);
}

function findPda(seeds) {
  return PublicKey.findProgramAddressSync(seeds, PROGRAM_ID);
}

const [platformConfig] = findPda([PLATFORM_CONFIG_SEED]);
const [platformVault] = findPda([PLATFORM_VAULT_SEED]);

async function sendTx(tx, signers, label) {
  tx.feePayer = wallet.publicKey;
  tx.recentBlockhash = (await conn.getLatestBlockhash()).blockhash;
  try {
    const sig = await sendAndConfirmTransaction(conn, tx, signers, { commitment: "confirmed" });
    console.log(`  ✅ ${label}: ${sig}`);
    return sig;
  } catch (e) {
    const logs = e?.logs || [];
    console.error(`  ❌ ${label} failed:`, e.message);
    if (logs.length) console.error("  Logs:", logs.slice(-5).join("\n    "));
    throw e;
  }
}

// ── Step 1: Init Platform ──
async function initPlatform() {
  console.log("\n═══ Step 1: Initialize Platform ═══");
  const info = await conn.getAccountInfo(platformConfig);
  if (info) {
    console.log("  Platform already initialized, skipping.");
    return;
  }
  const data = Buffer.concat([
    anchorDisc("initialize_platform"),
    Buffer.from(new BN(500).toArray("le", 2)),   // platform_fee_bps = 5%
    Buffer.from(new BN(0.5 * LAMPORTS_PER_SOL).toArray("le", 8)), // migration_threshold = 0.5 SOL
  ]);
  const tx = new Transaction().add({
    programId: PROGRAM_ID,
    keys: [
      { pubkey: platformConfig, isSigner: false, isWritable: true },
      { pubkey: wallet.publicKey, isSigner: true, isWritable: true },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data,
  });
  await sendTx(tx, [wallet], "init_platform");
}

// ── Step 2: Create Token ──
async function createToken() {
  console.log("\n═══ Step 2: Create Token ═══");
  const mintKp = Keypair.generate();
  const mint = mintKp.publicKey;
  console.log("  Mint:", mint.toBase58());

  const [tokenLaunch] = findPda([TOKEN_LAUNCH_SEED, mint.toBuffer()]);
  const [solVault] = findPda([SOL_VAULT_SEED, mint.toBuffer()]);
  const launchTokenVault = getAssociatedTokenAddressSync(mint, tokenLaunch, true);
  const [metadata] = PublicKey.findProgramAddressSync(
    [Buffer.from("metadata"), MPL_TOKEN_METADATA_ID.toBuffer(), mint.toBuffer()],
    MPL_TOKEN_METADATA_ID
  );

  const name = "AMMTest";
  const symbol = "AMMT";
  const uri = "https://senditsolana.io";
  const creatorFeeBps = 100;

  // Encode: disc + name(borsh string) + symbol + uri + creator_fee_bps(u16)
  const nameBuf = Buffer.from(name);
  const symBuf = Buffer.from(symbol);
  const uriBuf = Buffer.from(uri);
  const data = Buffer.concat([
    anchorDisc("create_token"),
    Buffer.from(new BN(nameBuf.length).toArray("le", 4)), nameBuf,
    Buffer.from(new BN(symBuf.length).toArray("le", 4)), symBuf,
    Buffer.from(new BN(uriBuf.length).toArray("le", 4)), uriBuf,
    Buffer.from(new BN(creatorFeeBps).toArray("le", 2)),
  ]);

  const tx = new Transaction().add(
    ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 }),
    {
      programId: PROGRAM_ID,
      keys: [
        { pubkey: tokenLaunch, isSigner: false, isWritable: true },
        { pubkey: mint, isSigner: true, isWritable: true },
        { pubkey: launchTokenVault, isSigner: false, isWritable: true },
        { pubkey: solVault, isSigner: false, isWritable: true },
        { pubkey: metadata, isSigner: false, isWritable: true },
        { pubkey: platformConfig, isSigner: false, isWritable: true },
        { pubkey: wallet.publicKey, isSigner: true, isWritable: true },
        { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
        { pubkey: ASSOCIATED_TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
        { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
        { pubkey: SYSVAR_RENT_PUBKEY, isSigner: false, isWritable: false },
        { pubkey: MPL_TOKEN_METADATA_ID, isSigner: false, isWritable: false },
      ],
      data,
    }
  );
  await sendTx(tx, [wallet, mintKp], "create_token");
  return mint;
}

// ── Step 3: Buy tokens (enough to hit migration threshold) ──
async function buyTokens(mint, solAmount) {
  console.log(`\n═══ Step 3: Buy ${solAmount / LAMPORTS_PER_SOL} SOL worth ═══`);
  const [tokenLaunch] = findPda([TOKEN_LAUNCH_SEED, mint.toBuffer()]);
  const [solVault] = findPda([SOL_VAULT_SEED, mint.toBuffer()]);
  const [userPos] = findPda([USER_POSITION_SEED, wallet.publicKey.toBuffer(), mint.toBuffer()]);
  const launchTokenVault = getAssociatedTokenAddressSync(mint, tokenLaunch, true);
  const buyerTokenAccount = getAssociatedTokenAddressSync(mint, wallet.publicKey);

  // Fund SOL vault for rent exemption
  const vaultInfo = await conn.getAccountInfo(solVault);
  if (!vaultInfo || vaultInfo.lamports < 890_880) {
    console.log("  Funding SOL vault for rent exemption...");
    const fundTx = new Transaction().add(
      SystemProgram.transfer({
        fromPubkey: wallet.publicKey,
        toPubkey: solVault,
        lamports: 1_000_000,
      })
    );
    await sendTx(fundTx, [wallet], "fund_sol_vault");
  }

  // Fund platform vault
  const pvInfo = await conn.getAccountInfo(platformVault);
  if (!pvInfo || pvInfo.lamports < 890_880) {
    console.log("  Funding platform vault for rent exemption...");
    const fundTx = new Transaction().add(
      SystemProgram.transfer({
        fromPubkey: wallet.publicKey,
        toPubkey: platformVault,
        lamports: 1_000_000,
      })
    );
    await sendTx(fundTx, [wallet], "fund_platform_vault");
  }

  // Find creator from token launch account
  const launchInfo = await conn.getAccountInfo(tokenLaunch);
  // creator is at offset 8 (discriminator), 32 bytes
  const creator = new PublicKey(launchInfo.data.subarray(8, 40));

  const data = Buffer.concat([
    anchorDisc("buy"),
    Buffer.from(new BN(solAmount).toArray("le", 8)),
  ]);

  const tx = new Transaction().add(
    ComputeBudgetProgram.setComputeUnitLimit({ units: 300_000 }),
    {
      programId: PROGRAM_ID,
      keys: [
        { pubkey: tokenLaunch, isSigner: false, isWritable: true },
        { pubkey: mint, isSigner: false, isWritable: false },
        { pubkey: launchTokenVault, isSigner: false, isWritable: true },
        { pubkey: solVault, isSigner: false, isWritable: true },
        { pubkey: buyerTokenAccount, isSigner: false, isWritable: true },
        { pubkey: userPos, isSigner: false, isWritable: true },
        { pubkey: platformVault, isSigner: false, isWritable: true },
        { pubkey: creator, isSigner: false, isWritable: true },
        { pubkey: platformConfig, isSigner: false, isWritable: true },
        { pubkey: wallet.publicKey, isSigner: true, isWritable: true },
        { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
        { pubkey: ASSOCIATED_TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
        { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      ],
      data,
    }
  );
  await sendTx(tx, [wallet], "buy_tokens");
}

// ── Step 4: Create Pool (graduation) ──
async function createPool(mint) {
  console.log("\n═══ Step 4: Create Pool (graduation) ═══");
  const [tokenLaunch] = findPda([TOKEN_LAUNCH_SEED, mint.toBuffer()]);
  const [solVault] = findPda([SOL_VAULT_SEED, mint.toBuffer()]);
  const [ammPool] = findPda([POOL_SEED, mint.toBuffer()]);
  const [poolSolVault] = findPda([POOL_SOL_VAULT_SEED, mint.toBuffer()]);
  const launchTokenVault = getAssociatedTokenAddressSync(mint, tokenLaunch, true);
  const poolTokenVault = getAssociatedTokenAddressSync(mint, ammPool, true);

  // LP mint — init'd by the program, but we use a Keypair? No — it's a PDA-less init.
  // Looking at CreatePool: lp_mint is `init, payer=creator, mint::decimals=6, mint::authority=amm_pool`
  // It's a new keypair generated by the client
  const lpMintKp = Keypair.generate();
  const lpMint = lpMintKp.publicKey;
  console.log("  LP Mint:", lpMint.toBase58());

  const creatorLpAccount = getAssociatedTokenAddressSync(lpMint, wallet.publicKey);

  // Fund pool SOL vault for rent exemption
  const psvInfo = await conn.getAccountInfo(poolSolVault);
  if (!psvInfo || psvInfo.lamports < 890_880) {
    console.log("  Funding pool SOL vault for rent exemption...");
    const fundTx = new Transaction().add(
      SystemProgram.transfer({
        fromPubkey: wallet.publicKey,
        toPubkey: poolSolVault,
        lamports: 1_000_000,
      })
    );
    await sendTx(fundTx, [wallet], "fund_pool_sol_vault");
  }

  const data = Buffer.concat([anchorDisc("create_pool")]);

  const tx = new Transaction().add(
    ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 }),
    {
      programId: PROGRAM_ID,
      keys: [
        { pubkey: ammPool, isSigner: false, isWritable: true },
        { pubkey: tokenLaunch, isSigner: false, isWritable: true },
        { pubkey: mint, isSigner: false, isWritable: false },
        { pubkey: launchTokenVault, isSigner: false, isWritable: true },
        { pubkey: solVault, isSigner: false, isWritable: true },
        { pubkey: poolTokenVault, isSigner: false, isWritable: true },
        { pubkey: poolSolVault, isSigner: false, isWritable: true },
        { pubkey: lpMint, isSigner: true, isWritable: true },
        { pubkey: creatorLpAccount, isSigner: false, isWritable: true },
        { pubkey: platformConfig, isSigner: false, isWritable: false },
        { pubkey: wallet.publicKey, isSigner: true, isWritable: true },
        { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
        { pubkey: ASSOCIATED_TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
        { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
        { pubkey: SYSVAR_RENT_PUBKEY, isSigner: false, isWritable: false },
      ],
      data,
    }
  );
  await sendTx(tx, [wallet, lpMintKp], "create_pool");
  return lpMint;
}

// ── Step 5: Swap SOL → Tokens ──
async function swapBuy(mint, solAmount) {
  console.log(`\n═══ Step 5: Swap ${solAmount / LAMPORTS_PER_SOL} SOL → Tokens ═══`);
  const [ammPool] = findPda([POOL_SEED, mint.toBuffer()]);
  const [poolSolVault] = findPda([POOL_SOL_VAULT_SEED, mint.toBuffer()]);
  const poolTokenVault = getAssociatedTokenAddressSync(mint, ammPool, true);
  const userTokenAccount = getAssociatedTokenAddressSync(mint, wallet.publicKey);

  const data = Buffer.concat([
    anchorDisc("swap"),
    Buffer.from(new BN(solAmount).toArray("le", 8)),  // sol_amount
    Buffer.from(new BN(0).toArray("le", 8)),           // token_amount = 0
  ]);

  const tx = new Transaction().add(
    ComputeBudgetProgram.setComputeUnitLimit({ units: 300_000 }),
    {
      programId: PROGRAM_ID,
      keys: [
        { pubkey: ammPool, isSigner: false, isWritable: true },
        { pubkey: mint, isSigner: false, isWritable: false },
        { pubkey: poolTokenVault, isSigner: false, isWritable: true },
        { pubkey: poolSolVault, isSigner: false, isWritable: true },
        { pubkey: userTokenAccount, isSigner: false, isWritable: true },
        { pubkey: platformVault, isSigner: false, isWritable: true },
        { pubkey: wallet.publicKey, isSigner: true, isWritable: true },
        { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
        { pubkey: ASSOCIATED_TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
        { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      ],
      data,
    }
  );
  await sendTx(tx, [wallet], "swap_buy");
}

// ── Step 6: Swap Tokens → SOL ──
async function swapSell(mint, tokenAmount) {
  console.log(`\n═══ Step 6: Swap ${tokenAmount} tokens → SOL ═══`);
  const [ammPool] = findPda([POOL_SEED, mint.toBuffer()]);
  const [poolSolVault] = findPda([POOL_SOL_VAULT_SEED, mint.toBuffer()]);
  const poolTokenVault = getAssociatedTokenAddressSync(mint, ammPool, true);
  const userTokenAccount = getAssociatedTokenAddressSync(mint, wallet.publicKey);

  const data = Buffer.concat([
    anchorDisc("swap"),
    Buffer.from(new BN(0).toArray("le", 8)),           // sol_amount = 0
    Buffer.from(new BN(tokenAmount).toArray("le", 8)), // token_amount
  ]);

  const tx = new Transaction().add(
    ComputeBudgetProgram.setComputeUnitLimit({ units: 300_000 }),
    {
      programId: PROGRAM_ID,
      keys: [
        { pubkey: ammPool, isSigner: false, isWritable: true },
        { pubkey: mint, isSigner: false, isWritable: false },
        { pubkey: poolTokenVault, isSigner: false, isWritable: true },
        { pubkey: poolSolVault, isSigner: false, isWritable: true },
        { pubkey: userTokenAccount, isSigner: false, isWritable: true },
        { pubkey: platformVault, isSigner: false, isWritable: true },
        { pubkey: wallet.publicKey, isSigner: true, isWritable: true },
        { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
        { pubkey: ASSOCIATED_TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
        { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      ],
      data,
    }
  );
  await sendTx(tx, [wallet], "swap_sell");
}

// ── Step 7: Add Liquidity ──
async function addLiquidity(mint, lpMint, solAmount) {
  console.log(`\n═══ Step 7: Add Liquidity (${solAmount / LAMPORTS_PER_SOL} SOL) ═══`);
  const [ammPool] = findPda([POOL_SEED, mint.toBuffer()]);
  const [poolSolVault] = findPda([POOL_SOL_VAULT_SEED, mint.toBuffer()]);
  const poolTokenVault = getAssociatedTokenAddressSync(mint, ammPool, true);
  const userTokenAccount = getAssociatedTokenAddressSync(mint, wallet.publicKey);
  const userLpAccount = getAssociatedTokenAddressSync(lpMint, wallet.publicKey);

  const data = Buffer.concat([
    anchorDisc("add_liquidity"),
    Buffer.from(new BN(solAmount).toArray("le", 8)),
  ]);

  const tx = new Transaction().add(
    ComputeBudgetProgram.setComputeUnitLimit({ units: 300_000 }),
    {
      programId: PROGRAM_ID,
      keys: [
        { pubkey: ammPool, isSigner: false, isWritable: true },
        { pubkey: mint, isSigner: false, isWritable: false },
        { pubkey: poolTokenVault, isSigner: false, isWritable: true },
        { pubkey: poolSolVault, isSigner: false, isWritable: true },
        { pubkey: lpMint, isSigner: false, isWritable: true },
        { pubkey: userTokenAccount, isSigner: false, isWritable: true },
        { pubkey: userLpAccount, isSigner: false, isWritable: true },
        { pubkey: wallet.publicKey, isSigner: true, isWritable: true },
        { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
        { pubkey: ASSOCIATED_TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
        { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      ],
      data,
    }
  );
  await sendTx(tx, [wallet], "add_liquidity");
}

// ── Step 8: Remove Liquidity ──
async function removeLiquidity(mint, lpMint, lpAmount) {
  console.log(`\n═══ Step 8: Remove Liquidity (${lpAmount} LP tokens) ═══`);
  const [ammPool] = findPda([POOL_SEED, mint.toBuffer()]);
  const [poolSolVault] = findPda([POOL_SOL_VAULT_SEED, mint.toBuffer()]);
  const poolTokenVault = getAssociatedTokenAddressSync(mint, ammPool, true);
  const userTokenAccount = getAssociatedTokenAddressSync(mint, wallet.publicKey);
  const userLpAccount = getAssociatedTokenAddressSync(lpMint, wallet.publicKey);

  const data = Buffer.concat([
    anchorDisc("remove_liquidity"),
    Buffer.from(new BN(lpAmount).toArray("le", 8)),
  ]);

  const tx = new Transaction().add(
    ComputeBudgetProgram.setComputeUnitLimit({ units: 300_000 }),
    {
      programId: PROGRAM_ID,
      keys: [
        { pubkey: ammPool, isSigner: false, isWritable: true },
        { pubkey: mint, isSigner: false, isWritable: false },
        { pubkey: poolTokenVault, isSigner: false, isWritable: true },
        { pubkey: poolSolVault, isSigner: false, isWritable: true },
        { pubkey: lpMint, isSigner: false, isWritable: true },
        { pubkey: userTokenAccount, isSigner: false, isWritable: true },
        { pubkey: userLpAccount, isSigner: false, isWritable: true },
        { pubkey: wallet.publicKey, isSigner: true, isWritable: true },
        { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
        { pubkey: ASSOCIATED_TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
        { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      ],
      data,
    }
  );
  await sendTx(tx, [wallet], "remove_liquidity");
}

// ── Main ──
async function main() {
  console.log("🏊 Send.it AMM Test — Full Lifecycle");
  console.log("Program:", PROGRAM_ID.toBase58());

  const bal = await conn.getBalance(wallet.publicKey);
  console.log(`Wallet balance: ${bal / LAMPORTS_PER_SOL} SOL`);
  if (bal < 2 * LAMPORTS_PER_SOL) {
    console.error("Need at least 2 SOL for full AMM test. Airdropping...");
    const sig = await conn.requestAirdrop(wallet.publicKey, 2 * LAMPORTS_PER_SOL);
    await conn.confirmTransaction(sig, "confirmed");
    console.log("Airdrop confirmed.");
  }

  // Step 1
  await initPlatform();

  // Step 2
  const mint = await createToken();

  // Step 3 — Buy 0.6 SOL worth (threshold is 0.5 SOL)
  await buyTokens(mint, 0.6 * LAMPORTS_PER_SOL);

  // Step 4 — Graduate to AMM pool
  const lpMint = await createPool(mint);

  // Step 5 — Swap buy 0.1 SOL through AMM
  await swapBuy(mint, 0.1 * LAMPORTS_PER_SOL);

  // Step 6 — Swap sell some tokens back
  // Get token balance first
  const userTokenAccount = getAssociatedTokenAddressSync(mint, wallet.publicKey);
  const tokenBal = await conn.getTokenAccountBalance(userTokenAccount);
  const sellAmount = Math.floor(Number(tokenBal.value.amount) / 4); // sell 25%
  console.log(`  Token balance: ${tokenBal.value.uiAmountString}, selling 25% = ${sellAmount} raw`);
  await swapSell(mint, sellAmount);

  // Step 7 — Add liquidity (0.05 SOL)
  await addLiquidity(mint, lpMint, 0.05 * LAMPORTS_PER_SOL);

  // Step 8 — Remove some LP tokens
  const userLpAccount = getAssociatedTokenAddressSync(lpMint, wallet.publicKey);
  const lpBal = await conn.getTokenAccountBalance(userLpAccount);
  const removeAmount = Math.floor(Number(lpBal.value.amount) / 2); // remove 50%
  console.log(`  LP balance: ${lpBal.value.uiAmountString}, removing 50% = ${removeAmount} raw`);
  await removeLiquidity(mint, lpMint, removeAmount);

  console.log("\n🎉 All 8 AMM test steps passed!");
  console.log("  Mint:", mint.toBase58());
  console.log("  LP Mint:", lpMint.toBase58());
}

main().catch(e => { console.error(e); process.exit(1); });
