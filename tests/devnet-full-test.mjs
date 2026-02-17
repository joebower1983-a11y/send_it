import { Connection, Keypair, PublicKey, SystemProgram, Transaction, TransactionInstruction, SYSVAR_RENT_PUBKEY } from "@solana/web3.js";
import { TOKEN_PROGRAM_ID, ASSOCIATED_TOKEN_PROGRAM_ID, getAssociatedTokenAddressSync } from "@solana/spl-token";
import { createHash } from "crypto";
import fs from "fs";

const PROGRAM_ID = new PublicKey("HTKq18cATdwCZb6XM66Mhn8JWKCFTrZqH6zU1zip88Zx");
const connection = new Connection("https://api.devnet.solana.com", "confirmed");
const keypairData = JSON.parse(fs.readFileSync("/home/joebower1983/.openclaw/workspace/sendit-5ive/deployer.json", "utf8"));
const wallet = Keypair.fromSecretKey(Uint8Array.from(keypairData));

function disc(name) { return createHash("sha256").update(`global:${name}`).digest().subarray(0, 8); }
function encodeString(s) { const buf = Buffer.alloc(4 + s.length); buf.writeUInt32LE(s.length, 0); buf.write(s, 4); return buf; }

const PLATFORM_CONFIG_SEED = Buffer.from("platform_config");
const TOKEN_LAUNCH_SEED = Buffer.from("token_launch");
const PLATFORM_VAULT_SEED = Buffer.from("platform_vault");
const SOL_VAULT_SEED = Buffer.from("sol_vault");
const USER_POSITION_SEED = Buffer.from("user_position");

const [platformConfig] = PublicKey.findProgramAddressSync([PLATFORM_CONFIG_SEED], PROGRAM_ID);
const [platformVault] = PublicKey.findProgramAddressSync([PLATFORM_VAULT_SEED], PROGRAM_ID);

console.log("Wallet:", wallet.publicKey.toBase58());
console.log("Balance:", (await connection.getBalance(wallet.publicKey)) / 1e9, "SOL\n");

// === 1. Init Platform (skip if exists) ===
const pcInfo = await connection.getAccountInfo(platformConfig);
if (pcInfo) {
  console.log("Platform already initialized, skipping...\n");
} else {
  console.log("--- Initialize Platform ---");
  const data = Buffer.alloc(8 + 2 + 8);
  disc("initialize_platform").copy(data, 0);
  data.writeUInt16LE(250, 8);
  data.writeBigUInt64LE(100_000_000_000n, 10);
  const tx = new Transaction().add(new TransactionInstruction({
    keys: [
      { pubkey: platformConfig, isSigner: false, isWritable: true },
      { pubkey: wallet.publicKey, isSigner: true, isWritable: true },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ], programId: PROGRAM_ID, data,
  }));
  tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
  tx.feePayer = wallet.publicKey; tx.sign(wallet);
  const sig = await connection.sendRawTransaction(tx.serialize());
  await connection.confirmTransaction(sig, "confirmed");
  console.log("✅ Platform initialized! Tx:", sig, "\n");
}

// === 2. Create Token ===
console.log("--- Create Token ---");
const mintKeypair = Keypair.generate();
const mint = mintKeypair.publicKey;
const [tokenLaunch] = PublicKey.findProgramAddressSync([TOKEN_LAUNCH_SEED, mint.toBuffer()], PROGRAM_ID);
const [solVault] = PublicKey.findProgramAddressSync([SOL_VAULT_SEED, mint.toBuffer()], PROGRAM_ID);
const launchVault = getAssociatedTokenAddressSync(mint, tokenLaunch, true);

console.log("Mint:", mint.toBase58());

const createData = Buffer.concat([
  disc("create_token"), encodeString("SendIt Test V2"), encodeString("SENDIT"), encodeString("https://send-it-seven-sigma.vercel.app"), Buffer.from([0xF4, 0x01]),
]);
const createTx = new Transaction().add(new TransactionInstruction({
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
  ], programId: PROGRAM_ID, data: createData,
}));
createTx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
createTx.feePayer = wallet.publicKey; createTx.sign(wallet, mintKeypair);
try {
  const sig = await connection.sendRawTransaction(createTx.serialize());
  await connection.confirmTransaction(sig, "confirmed");
  console.log("✅ Token created! Tx:", sig, "\n");
} catch (e) {
  console.error("Create error:", e.message);
  if (e.logs) e.logs.forEach(l => console.log("  ", l));
  process.exit(1);
}

// === 3. Buy 0.01 SOL ===
console.log("--- Buy 0.01 SOL ---");
const buyerAta = getAssociatedTokenAddressSync(mint, wallet.publicKey);
const [userPosition] = PublicKey.findProgramAddressSync([USER_POSITION_SEED, wallet.publicKey.toBuffer(), mint.toBuffer()], PROGRAM_ID);

// Pre-fund vaults
const rentExempt = await connection.getMinimumBalanceForRentExemption(0);
const preTx = new Transaction();
if (!(await connection.getAccountInfo(solVault))) preTx.add(SystemProgram.transfer({ fromPubkey: wallet.publicKey, toPubkey: solVault, lamports: rentExempt }));
if (!(await connection.getAccountInfo(platformVault))) preTx.add(SystemProgram.transfer({ fromPubkey: wallet.publicKey, toPubkey: platformVault, lamports: rentExempt }));
if (preTx.instructions.length > 0) {
  preTx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
  preTx.feePayer = wallet.publicKey; preTx.sign(wallet);
  const preSig = await connection.sendRawTransaction(preTx.serialize());
  await connection.confirmTransaction(preSig, "confirmed");
  console.log("Pre-funded vaults");
}

const buyData = Buffer.alloc(16); disc("buy").copy(buyData, 0); buyData.writeBigUInt64LE(10_000_000n, 8);
const buyTx = new Transaction().add(new TransactionInstruction({
  keys: [
    { pubkey: tokenLaunch, isSigner: false, isWritable: true },
    { pubkey: mint, isSigner: false, isWritable: false },
    { pubkey: launchVault, isSigner: false, isWritable: true },
    { pubkey: solVault, isSigner: false, isWritable: true },
    { pubkey: buyerAta, isSigner: false, isWritable: true },
    { pubkey: userPosition, isSigner: false, isWritable: true },
    { pubkey: platformVault, isSigner: false, isWritable: true },
    { pubkey: wallet.publicKey, isSigner: false, isWritable: true }, // creator = wallet
    { pubkey: platformConfig, isSigner: false, isWritable: true },
    { pubkey: wallet.publicKey, isSigner: true, isWritable: true },
    { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
    { pubkey: ASSOCIATED_TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
    { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
  ], programId: PROGRAM_ID, data: buyData,
}));
buyTx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
buyTx.feePayer = wallet.publicKey; buyTx.sign(wallet);
try {
  const sig = await connection.sendRawTransaction(buyTx.serialize());
  await connection.confirmTransaction(sig, "confirmed");
  const tokenBal = await connection.getTokenAccountBalance(buyerAta);
  console.log("✅ Buy succeeded! Tokens:", tokenBal.value.uiAmountString, "Tx:", sig, "\n");
} catch (e) {
  console.error("Buy error:", e.message);
  if (e.logs) e.logs.forEach(l => console.log("  ", l));
  process.exit(1);
}

// === 4. Sell half the tokens ===
console.log("--- Sell 5M tokens ---");
const sellData = Buffer.alloc(16); disc("sell").copy(sellData, 0); sellData.writeBigUInt64LE(5_000_000n, 8);
const sellTx = new Transaction().add(new TransactionInstruction({
  keys: [
    { pubkey: tokenLaunch, isSigner: false, isWritable: true },
    { pubkey: mint, isSigner: false, isWritable: false },
    { pubkey: launchVault, isSigner: false, isWritable: true },
    { pubkey: solVault, isSigner: false, isWritable: true },
    { pubkey: buyerAta, isSigner: false, isWritable: true },
    { pubkey: userPosition, isSigner: false, isWritable: true },
    { pubkey: platformVault, isSigner: false, isWritable: true },
    { pubkey: wallet.publicKey, isSigner: false, isWritable: true }, // creator
    { pubkey: platformConfig, isSigner: false, isWritable: true },
    { pubkey: wallet.publicKey, isSigner: true, isWritable: true },
    { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
    { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
  ], programId: PROGRAM_ID, data: sellData,
}));
sellTx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
sellTx.feePayer = wallet.publicKey; sellTx.sign(wallet);
try {
  const sig = await connection.sendRawTransaction(sellTx.serialize());
  await connection.confirmTransaction(sig, "confirmed");
  const tokenBal = await connection.getTokenAccountBalance(buyerAta);
  const solBal = await connection.getBalance(wallet.publicKey);
  console.log("✅ Sell succeeded! Tokens remaining:", tokenBal.value.uiAmountString, "SOL balance:", solBal / 1e9, "Tx:", sig);
} catch (e) {
  console.error("Sell error:", e.message);
  if (e.logs) e.logs.forEach(l => console.log("  ", l));
}

console.log("\n🎉 Full loop complete: init → create → buy → sell");
