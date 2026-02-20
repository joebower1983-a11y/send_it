#!/usr/bin/env node
/**
 * Send.Swap Pool Seeder — Creates graduated tokens and seeds AMM pools on devnet
 */
import { Connection, Keypair, PublicKey, SystemProgram, Transaction, TransactionInstruction, LAMPORTS_PER_SOL, sendAndConfirmTransaction } from "@solana/web3.js";
import { TOKEN_PROGRAM_ID, ASSOCIATED_TOKEN_PROGRAM_ID, getAssociatedTokenAddressSync, createAssociatedTokenAccountInstruction } from "@solana/spl-token";
import { createHash } from "crypto";
import fs from "fs";
import path from "path";

// ─── Config ───
const PROGRAM_ID = new PublicKey("HTKq18cATdwCZb6XM66Mhn8JWKCFTrZqH6zU1zip88Zx");
const RPC = process.env.SOLANA_RPC || "https://api.devnet.solana.com";
const connection = new Connection(RPC, "confirmed");

const PLATFORM_CONFIG_SEED = Buffer.from("platform_config");
const TOKEN_LAUNCH_SEED = Buffer.from("token_launch");
const USER_POSITION_SEED = Buffer.from("user_position");
const PLATFORM_VAULT_SEED = Buffer.from("platform_vault");
const SOL_VAULT_SEED = Buffer.from("sol_vault");
const POOL_SEED = Buffer.from("amm_pool");
const POOL_SOL_VAULT_SEED = Buffer.from("pool_sol_vault");

const MPL_TOKEN_METADATA_ID = new PublicKey("metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s");

const [platformConfig] = PublicKey.findProgramAddressSync([PLATFORM_CONFIG_SEED], PROGRAM_ID);
const [platformVault] = PublicKey.findProgramAddressSync([PLATFORM_VAULT_SEED], PROGRAM_ID);

function disc(name) { return createHash("sha256").update(`global:${name}`).digest().subarray(0, 8); }
function encodeString(s) { const buf = Buffer.alloc(4 + s.length); buf.writeUInt32LE(s.length, 0); buf.write(s, 4); return buf; }
function encodeU16LE(v) { const b = Buffer.alloc(2); b.writeUInt16LE(v); return b; }
function encodeU64LE(v) { const b = Buffer.alloc(8); b.writeBigUInt64LE(BigInt(v)); return b; }

function loadKeypair() {
  const paths = [
    process.env.KEYPAIR_PATH,
    path.resolve('deployer.json'),
    path.join(process.env.HOME || '', '.config/solana/id.json'),
  ].filter(Boolean);
  for (const p of paths) {
    try { return Keypair.fromSecretKey(Uint8Array.from(JSON.parse(fs.readFileSync(p, 'utf8')))); } catch {}
  }
  console.error("❌ No keypair found"); process.exit(1);
}

async function airdrop(pubkey, sol) {
  console.log(`  💧 Airdropping ${sol} SOL to ${pubkey.toBase58().slice(0,8)}...`);
  try {
    const sig = await connection.requestAirdrop(pubkey, sol * LAMPORTS_PER_SOL);
    await connection.confirmTransaction(sig, "confirmed");
    console.log(`  ✅ Airdrop confirmed`);
  } catch (e) {
    console.log(`  ⚠️ Airdrop failed (${e.message}) — may need manual funding`);
  }
}

async function sendTx(tx, signers) {
  tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
  tx.feePayer = signers[0].publicKey;
  const sig = await sendAndConfirmTransaction(connection, tx, signers, { commitment: "confirmed" });
  return sig;
}

// ─── Pool tokens to create ───
const POOLS = [
  { name: "AlphaSwap", symbol: "ALPHA", uri: "https://senditsolana.io/meta/alpha.json", buyAmount: 0.6 },
  { name: "BetaToken", symbol: "BETA",  uri: "https://senditsolana.io/meta/beta.json",  buyAmount: 0.6 },
  { name: "GammaFi",   symbol: "GAMMA", uri: "https://senditsolana.io/meta/gamma.json", buyAmount: 0.6 },
];

async function createToken(wallet, { name, symbol, uri }) {
  const mintKp = Keypair.generate();
  const mk = mintKp.publicKey;

  const [tokenLaunch, tlBump] = PublicKey.findProgramAddressSync([TOKEN_LAUNCH_SEED, mk.toBuffer()], PROGRAM_ID);
  const [solVault] = PublicKey.findProgramAddressSync([SOL_VAULT_SEED, mk.toBuffer()], PROGRAM_ID);
  const launchTokenVault = getAssociatedTokenAddressSync(mk, tokenLaunch, true);
  const [metadata] = PublicKey.findProgramAddressSync(
    [Buffer.from("metadata"), MPL_TOKEN_METADATA_ID.toBuffer(), mk.toBuffer()],
    MPL_TOKEN_METADATA_ID
  );

  const data = Buffer.concat([
    disc("create_token"),
    encodeString(name),
    encodeString(symbol),
    encodeString(uri),
    encodeU16LE(100), // creator_fee_bps = 1%
  ]);

  const keys = [
    { pubkey: tokenLaunch, isSigner: false, isWritable: true },
    { pubkey: mk, isSigner: true, isWritable: true },
    { pubkey: launchTokenVault, isSigner: false, isWritable: true },
    { pubkey: solVault, isSigner: false, isWritable: true },
    { pubkey: metadata, isSigner: false, isWritable: true },
    { pubkey: platformConfig, isSigner: false, isWritable: true },
    { pubkey: wallet.publicKey, isSigner: true, isWritable: true },
    { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
    { pubkey: ASSOCIATED_TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
    { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    { pubkey: new PublicKey("SysvarRent111111111111111111111111111111111"), isSigner: false, isWritable: false },
    { pubkey: MPL_TOKEN_METADATA_ID, isSigner: false, isWritable: false },
  ];

  const tx = new Transaction().add(new TransactionInstruction({ programId: PROGRAM_ID, keys, data }));
  const sig = await sendTx(tx, [wallet, mintKp]);
  console.log(`  ✅ Created ${name} (${symbol}) — mint: ${mk.toBase58()}`);
  console.log(`     tx: ${sig}`);
  return mk;
}

async function buyTokens(wallet, mint, solAmount) {
  const mk = mint;
  const [tokenLaunch] = PublicKey.findProgramAddressSync([TOKEN_LAUNCH_SEED, mk.toBuffer()], PROGRAM_ID);
  const [solVault] = PublicKey.findProgramAddressSync([SOL_VAULT_SEED, mk.toBuffer()], PROGRAM_ID);
  const launchTokenVault = getAssociatedTokenAddressSync(mk, tokenLaunch, true);
  const buyerTokenAccount = getAssociatedTokenAddressSync(mk, wallet.publicKey);
  const [userPosition] = PublicKey.findProgramAddressSync([USER_POSITION_SEED, wallet.publicKey.toBuffer(), mk.toBuffer()], PROGRAM_ID);

  // Fetch token launch to get creator
  const launchData = await connection.getAccountInfo(tokenLaunch);
  // Creator is at offset 8 (after discriminator), 32 bytes
  const creator = new PublicKey(launchData.data.subarray(8, 40));

  const data = Buffer.concat([disc("buy"), encodeU64LE(Math.floor(solAmount * LAMPORTS_PER_SOL))]);

  const keys = [
    { pubkey: tokenLaunch, isSigner: false, isWritable: true },
    { pubkey: mk, isSigner: false, isWritable: true },
    { pubkey: launchTokenVault, isSigner: false, isWritable: true },
    { pubkey: solVault, isSigner: false, isWritable: true },
    { pubkey: buyerTokenAccount, isSigner: false, isWritable: true },
    { pubkey: userPosition, isSigner: false, isWritable: true },
    { pubkey: platformVault, isSigner: false, isWritable: true },
    { pubkey: creator, isSigner: false, isWritable: true },
    { pubkey: platformConfig, isSigner: false, isWritable: true },
    { pubkey: wallet.publicKey, isSigner: true, isWritable: true },
    { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
    { pubkey: ASSOCIATED_TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
    { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
  ];

  const tx = new Transaction().add(new TransactionInstruction({ programId: PROGRAM_ID, keys, data }));
  const sig = await sendTx(tx, [wallet]);
  console.log(`  ✅ Bought ${solAmount} SOL worth — tx: ${sig}`);
}

async function createPool(wallet, mint) {
  const mk = mint;
  const [tokenLaunch] = PublicKey.findProgramAddressSync([TOKEN_LAUNCH_SEED, mk.toBuffer()], PROGRAM_ID);
  const [solVault] = PublicKey.findProgramAddressSync([SOL_VAULT_SEED, mk.toBuffer()], PROGRAM_ID);
  const launchTokenVault = getAssociatedTokenAddressSync(mk, tokenLaunch, true);
  const [ammPool] = PublicKey.findProgramAddressSync([POOL_SEED, mk.toBuffer()], PROGRAM_ID);
  const [poolSolVault] = PublicKey.findProgramAddressSync([POOL_SOL_VAULT_SEED, mk.toBuffer()], PROGRAM_ID);
  const poolTokenVault = getAssociatedTokenAddressSync(mk, ammPool, true);

  // LP mint — Anchor init uses a random keypair for non-PDA mints, but here it's init with seeds
  // Looking at the context: lp_mint is `init, payer=creator, mint::decimals=6, mint::authority=amm_pool`
  // It's NOT a PDA — it's a fresh keypair. We need to generate one.
  const lpMintKp = Keypair.generate();
  const lpMint = lpMintKp.publicKey;
  const creatorLpAccount = getAssociatedTokenAddressSync(lpMint, wallet.publicKey);

  const data = disc("create_pool");

  const keys = [
    { pubkey: ammPool, isSigner: false, isWritable: true },
    { pubkey: tokenLaunch, isSigner: false, isWritable: true },
    { pubkey: mk, isSigner: false, isWritable: false },
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
    { pubkey: new PublicKey("SysvarRent111111111111111111111111111111111"), isSigner: false, isWritable: false },
  ];

  const tx = new Transaction().add(new TransactionInstruction({ programId: PROGRAM_ID, keys, data }));
  const sig = await sendTx(tx, [wallet, lpMintKp]);
  console.log(`  ✅ Pool created — AMM: ${ammPool.toBase58()}`);
  console.log(`     LP Mint: ${lpMint.toBase58()}`);
  console.log(`     tx: ${sig}`);
  return { ammPool, lpMint };
}

async function sleep(ms) { return new Promise(r => setTimeout(r, ms)); }

// ─── Main ───
async function main() {
  const wallet = loadKeypair();
  console.log(`\n🚀 Send.Swap Pool Seeder`);
  console.log(`   Wallet: ${wallet.publicKey.toBase58()}`);
  console.log(`   Program: ${PROGRAM_ID.toBase58()}`);
  console.log(`   Network: devnet\n`);

  // Check balance, airdrop if needed
  let balance = await connection.getBalance(wallet.publicKey);
  console.log(`💰 Balance: ${balance / LAMPORTS_PER_SOL} SOL`);

  const totalNeeded = 6; // ~2 SOL per pool (create token + buy + create pool + rent)
  if (balance < totalNeeded * LAMPORTS_PER_SOL) {
    const rounds = Math.ceil((totalNeeded * LAMPORTS_PER_SOL - balance) / (2 * LAMPORTS_PER_SOL));
    for (let i = 0; i < rounds; i++) {
      await airdrop(wallet.publicKey, 2);
      await sleep(2000);
    }
    balance = await connection.getBalance(wallet.publicKey);
    console.log(`💰 Balance after airdrops: ${balance / LAMPORTS_PER_SOL} SOL\n`);
  }

  // Check if platform is initialized
  const configAcct = await connection.getAccountInfo(platformConfig);
  if (!configAcct) {
    console.log("⚙️ Initializing platform...");
    const data = Buffer.concat([
      disc("initialize_platform"),
      encodeU16LE(100),  // platform_fee_bps = 1%
      encodeU64LE(Math.floor(0.2 * LAMPORTS_PER_SOL)), // migration threshold = 0.2 SOL (low for testing)
    ]);
    const keys = [
      { pubkey: platformConfig, isSigner: false, isWritable: true },
      { pubkey: wallet.publicKey, isSigner: true, isWritable: true },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ];
    const tx = new Transaction().add(new TransactionInstruction({ programId: PROGRAM_ID, keys, data }));
    await sendTx(tx, [wallet]);
    console.log("  ✅ Platform initialized\n");
  } else {
    // Check migration threshold — we need it low enough for our buys to trigger graduation
    console.log("⚙️ Platform already initialized\n");
  }

  const results = [];

  for (const pool of POOLS) {
    console.log(`\n═══ Creating ${pool.name} (${pool.symbol}) ═══`);

    // 1. Create token
    console.log("📦 Step 1: Create token...");
    let mint;
    try {
      mint = await createToken(wallet, pool);
    } catch (e) {
      if (e.message.includes('already in use')) {
        console.log(`  ⚠️ Token already exists, skipping creation`);
        continue; // Can't reuse random mint, skip this pool
      }
      throw e;
    }
    await sleep(2000);

    // 2. Buy enough to hit migration threshold
    console.log(`📈 Step 2: Buy ${pool.buyAmount} SOL worth...`);
    await buyTokens(wallet, mint, pool.buyAmount);
    await sleep(2000);

    // 3. Create pool (graduation)
    console.log("🏊 Step 3: Create Send.Swap pool (graduate)...");
    try {
      const { ammPool, lpMint } = await createPool(wallet, mint);
      results.push({ name: pool.name, symbol: pool.symbol, mint: mint.toBase58(), ammPool: ammPool.toBase58(), lpMint: lpMint.toBase58() });
    } catch (e) {
      console.log(`  ⚠️ Pool creation failed: ${e.message}`);
      console.log(`     Token may not have reached migration threshold yet.`);
      results.push({ name: pool.name, symbol: pool.symbol, mint: mint.toBase58(), ammPool: null, error: e.message });
    }
  }

  console.log(`\n\n═══════════════════════════════════════`);
  console.log(`  🏊 Send.Swap Pool Summary`);
  console.log(`═══════════════════════════════════════\n`);

  for (const r of results) {
    console.log(`${r.symbol}:`);
    console.log(`  Mint: ${r.mint}`);
    if (r.ammPool) {
      console.log(`  Pool: ${r.ammPool}`);
      console.log(`  LP:   ${r.lpMint}`);
      console.log(`  Status: ✅ Live`);
    } else {
      console.log(`  Status: ❌ ${r.error}`);
    }
    console.log();
  }

  // Save results
  fs.writeFileSync('scripts/pool-results.json', JSON.stringify(results, null, 2));
  console.log("💾 Results saved to scripts/pool-results.json");
}

main().catch(e => { console.error("❌ Fatal:", e.message); process.exit(1); });
