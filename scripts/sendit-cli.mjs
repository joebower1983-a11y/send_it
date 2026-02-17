#!/usr/bin/env node
/**
 * Send.it Devnet CLI
 * Interact with the Send.it protocol on Solana devnet
 * 
 * Usage:
 *   node sendit-cli.mjs init                              Initialize platform
 *   node sendit-cli.mjs create --name "Token" --symbol TK Create a token
 *   node sendit-cli.mjs buy --mint <addr> --sol 0.01      Buy tokens
 *   node sendit-cli.mjs sell --mint <addr> --tokens 5000  Sell tokens
 *   node sendit-cli.mjs info --mint <addr>                Show token info
 *   node sendit-cli.mjs balance                           Show wallet balance
 *   node sendit-cli.mjs social-profile --wallet <addr> --username <name>  Create social profile
 */

import { Connection, Keypair, PublicKey, SystemProgram, Transaction, TransactionInstruction, LAMPORTS_PER_SOL, SYSVAR_RENT_PUBKEY } from "@solana/web3.js";
import { TOKEN_PROGRAM_ID, ASSOCIATED_TOKEN_PROGRAM_ID, getAssociatedTokenAddressSync } from "@solana/spl-token";
import { createHash } from "crypto";
import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";

// ─── Config ───
const PROGRAM_ID = new PublicKey("HTKq18cATdwCZb6XM66Mhn8JWKCFTrZqH6zU1zip88Zx");
const RPC = process.env.SOLANA_RPC || "https://api.devnet.solana.com";
const connection = new Connection(RPC, "confirmed");

// Seeds
const SEEDS = {
  platformConfig: Buffer.from("platform_config"),
  tokenLaunch: Buffer.from("token_launch"),
  platformVault: Buffer.from("platform_vault"),
  solVault: Buffer.from("sol_vault"),
  userPosition: Buffer.from("user_position"),
};

// PDAs
const [platformConfig] = PublicKey.findProgramAddressSync([SEEDS.platformConfig], PROGRAM_ID);
const [platformVault] = PublicKey.findProgramAddressSync([SEEDS.platformVault], PROGRAM_ID);

// ─── Helpers ───
function disc(name) { return createHash("sha256").update(`global:${name}`).digest().subarray(0, 8); }
function encodeString(s) { const buf = Buffer.alloc(4 + s.length); buf.writeUInt32LE(s.length, 0); buf.write(s, 4); return buf; }

function loadKeypair() {
  const paths = [
    path.resolve('deployer.json'),
    path.join(process.env.HOME || '', '.config/solana/id.json'),
  ];
  // Allow env override
  if (process.env.KEYPAIR_PATH) paths.unshift(process.env.KEYPAIR_PATH);

  for (const p of paths) {
    try {
      const data = JSON.parse(fs.readFileSync(p, 'utf8'));
      return Keypair.fromSecretKey(Uint8Array.from(data));
    } catch {}
  }
  console.error("❌ No keypair found. Set KEYPAIR_PATH or place deployer.json in current dir.");
  process.exit(1);
}

function parseArgs(args) {
  const parsed = { _: [] };
  for (let i = 0; i < args.length; i++) {
    if (args[i].startsWith('--')) {
      const key = args[i].slice(2);
      const val = args[i + 1] && !args[i + 1].startsWith('--') ? args[++i] : true;
      parsed[key] = val;
    } else {
      parsed._.push(args[i]);
    }
  }
  return parsed;
}

async function sendTx(tx, signers) {
  tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
  tx.feePayer = signers[0].publicKey;
  tx.sign(...signers);
  const sig = await connection.sendRawTransaction(tx.serialize());
  await connection.confirmTransaction(sig, "confirmed");
  return sig;
}

// ─── Commands ───

async function cmdInit(wallet) {
  const info = await connection.getAccountInfo(platformConfig);
  if (info) {
    console.log("✅ Platform already initialized");
    console.log("   Config PDA:", platformConfig.toBase58());
    return;
  }
  console.log("Initializing platform...");
  const data = Buffer.alloc(8 + 2 + 8);
  disc("initialize_platform").copy(data, 0);
  data.writeUInt16LE(250, 8); // 2.5% fee
  data.writeBigUInt64LE(100_000_000_000n, 10); // 100B supply
  const tx = new Transaction().add(new TransactionInstruction({
    keys: [
      { pubkey: platformConfig, isSigner: false, isWritable: true },
      { pubkey: wallet.publicKey, isSigner: true, isWritable: true },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    programId: PROGRAM_ID, data,
  }));
  const sig = await sendTx(tx, [wallet]);
  console.log("✅ Platform initialized!");
  console.log("   Tx:", sig);
  console.log("   Config:", platformConfig.toBase58());
}

async function cmdCreate(wallet, args) {
  const name = args.name || "SendIt Token";
  const symbol = args.symbol || "SENDIT";
  const uri = args.uri || "https://send-it-seven-sigma.vercel.app";

  const mintKeypair = Keypair.generate();
  const mint = mintKeypair.publicKey;
  const [tokenLaunch] = PublicKey.findProgramAddressSync([SEEDS.tokenLaunch, mint.toBuffer()], PROGRAM_ID);
  const [solVault] = PublicKey.findProgramAddressSync([SEEDS.solVault, mint.toBuffer()], PROGRAM_ID);
  const launchVault = getAssociatedTokenAddressSync(mint, tokenLaunch, true);

  console.log(`Creating token: ${name} ($${symbol})`);
  console.log("   Mint:", mint.toBase58());

  const createData = Buffer.concat([
    disc("create_token"), encodeString(name), encodeString(symbol), encodeString(uri), Buffer.from([0xF4, 0x01]),
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
    programId: PROGRAM_ID, data: createData,
  }));
  const sig = await sendTx(tx, [wallet, mintKeypair]);
  console.log("✅ Token created!");
  console.log("   Tx:", sig);
  console.log("   Mint:", mint.toBase58());
  console.log("   Token Launch PDA:", tokenLaunch.toBase58());
}

async function cmdBuy(wallet, args) {
  if (!args.mint) { console.error("❌ --mint required"); process.exit(1); }
  const solAmount = parseFloat(args.sol || args.amount || "0.01");
  const mint = new PublicKey(args.mint);
  const [tokenLaunch] = PublicKey.findProgramAddressSync([SEEDS.tokenLaunch, mint.toBuffer()], PROGRAM_ID);
  const [solVault] = PublicKey.findProgramAddressSync([SEEDS.solVault, mint.toBuffer()], PROGRAM_ID);
  const launchVault = getAssociatedTokenAddressSync(mint, tokenLaunch, true);
  const buyerAta = getAssociatedTokenAddressSync(mint, wallet.publicKey);
  const [userPosition] = PublicKey.findProgramAddressSync([SEEDS.userPosition, wallet.publicKey.toBuffer(), mint.toBuffer()], PROGRAM_ID);

  console.log(`Buying with ${solAmount} SOL on mint ${mint.toBase58()}`);

  // Pre-fund vaults if needed
  const rentExempt = await connection.getMinimumBalanceForRentExemption(0);
  const preTx = new Transaction();
  if (!(await connection.getAccountInfo(solVault))) preTx.add(SystemProgram.transfer({ fromPubkey: wallet.publicKey, toPubkey: solVault, lamports: rentExempt }));
  if (!(await connection.getAccountInfo(platformVault))) preTx.add(SystemProgram.transfer({ fromPubkey: wallet.publicKey, toPubkey: platformVault, lamports: rentExempt }));
  if (preTx.instructions.length > 0) {
    await sendTx(preTx, [wallet]);
    console.log("   Pre-funded vaults");
  }

  const lamports = BigInt(Math.round(solAmount * LAMPORTS_PER_SOL));
  const buyData = Buffer.alloc(16); disc("buy").copy(buyData, 0); buyData.writeBigUInt64LE(lamports, 8);
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
  const tokenBal = await connection.getTokenAccountBalance(buyerAta);
  console.log("✅ Buy succeeded!");
  console.log("   Tokens received:", tokenBal.value.uiAmountString);
  console.log("   Tx:", sig);
}

async function cmdSell(wallet, args) {
  if (!args.mint) { console.error("❌ --mint required"); process.exit(1); }
  const tokenAmount = BigInt(args.tokens || args.amount || "5000000");
  const mint = new PublicKey(args.mint);
  const [tokenLaunch] = PublicKey.findProgramAddressSync([SEEDS.tokenLaunch, mint.toBuffer()], PROGRAM_ID);
  const [solVault] = PublicKey.findProgramAddressSync([SEEDS.solVault, mint.toBuffer()], PROGRAM_ID);
  const launchVault = getAssociatedTokenAddressSync(mint, tokenLaunch, true);
  const buyerAta = getAssociatedTokenAddressSync(mint, wallet.publicKey);
  const [userPosition] = PublicKey.findProgramAddressSync([SEEDS.userPosition, wallet.publicKey.toBuffer(), mint.toBuffer()], PROGRAM_ID);

  console.log(`Selling ${tokenAmount} tokens on mint ${mint.toBase58()}`);
  const sellData = Buffer.alloc(16); disc("sell").copy(sellData, 0); sellData.writeBigUInt64LE(tokenAmount, 8);
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
  const tokenBal = await connection.getTokenAccountBalance(buyerAta);
  const solBal = await connection.getBalance(wallet.publicKey);
  console.log("✅ Sell succeeded!");
  console.log("   Tokens remaining:", tokenBal.value.uiAmountString);
  console.log("   SOL balance:", (solBal / LAMPORTS_PER_SOL).toFixed(4));
  console.log("   Tx:", sig);
}

async function cmdInfo(args) {
  if (!args.mint) { console.error("❌ --mint required"); process.exit(1); }
  const mint = new PublicKey(args.mint);
  const [tokenLaunch] = PublicKey.findProgramAddressSync([SEEDS.tokenLaunch, mint.toBuffer()], PROGRAM_ID);
  const [solVault] = PublicKey.findProgramAddressSync([SEEDS.solVault, mint.toBuffer()], PROGRAM_ID);
  const launchVault = getAssociatedTokenAddressSync(mint, tokenLaunch, true);

  console.log("Token Info for:", mint.toBase58());
  console.log("─".repeat(50));

  const launchInfo = await connection.getAccountInfo(tokenLaunch);
  if (!launchInfo) { console.log("❌ Token launch not found"); return; }
  console.log("   Token Launch PDA:", tokenLaunch.toBase58());
  console.log("   SOL Vault:", solVault.toBase58());
  console.log("   Token Vault:", launchVault.toBase58());

  try {
    const solBal = await connection.getBalance(solVault);
    console.log("   SOL in vault:", (solBal / LAMPORTS_PER_SOL).toFixed(4), "SOL");
  } catch { console.log("   SOL vault: not funded"); }

  try {
    const tokenBal = await connection.getTokenAccountBalance(launchVault);
    console.log("   Tokens in vault:", tokenBal.value.uiAmountString);
  } catch { console.log("   Token vault: not created"); }

  console.log("   Launch data size:", launchInfo.data.length, "bytes");
}

async function cmdBalance(wallet) {
  const bal = await connection.getBalance(wallet.publicKey);
  console.log("Wallet:", wallet.publicKey.toBase58());
  console.log("Balance:", (bal / LAMPORTS_PER_SOL).toFixed(4), "SOL");
  console.log("Network:", RPC);
  console.log("Program:", PROGRAM_ID.toBase58());
  console.log("Platform Config:", platformConfig.toBase58());
}

// ─── Main ───
const args = parseArgs(process.argv.slice(2));
const command = args._[0];

if (!command || command === 'help') {
  console.log(`
Send.it Devnet CLI 🚀

Usage:
  node sendit-cli.mjs <command> [options]

Commands:
  init                                    Initialize platform config
  create --name "Name" --symbol SYM       Create a new token launch
  buy --mint <addr> --sol 0.01            Buy tokens with SOL
  sell --mint <addr> --tokens 5000000     Sell tokens for SOL
  info --mint <addr>                      Show token launch info
  balance                                 Show wallet balance & config
  help                                    Show this help

Environment:
  SOLANA_RPC      RPC URL (default: devnet)
  KEYPAIR_PATH    Path to keypair JSON

Program: ${PROGRAM_ID.toBase58()}
Network: Solana Devnet
`);
  process.exit(0);
}

const wallet = loadKeypair();
console.log(`Send.it CLI 🚀 | ${wallet.publicKey.toBase58().slice(0, 8)}... | devnet\n`);

try {
  switch (command) {
    case 'init': await cmdInit(wallet); break;
    case 'create': await cmdCreate(wallet, args); break;
    case 'buy': await cmdBuy(wallet, args); break;
    case 'sell': await cmdSell(wallet, args); break;
    case 'info': await cmdInfo(args); break;
    case 'balance': await cmdBalance(wallet); break;
    default: console.error(`Unknown command: ${command}. Run with 'help' for usage.`);
  }
} catch (e) {
  console.error("❌ Error:", e.message);
  if (e.logs) e.logs.forEach(l => console.log("  ", l));
  process.exit(1);
}
