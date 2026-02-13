const { Connection, Keypair, PublicKey, Transaction, SystemProgram, LAMPORTS_PER_SOL } = require('@solana/web3.js');
const bs58 = require('bs58');

const connection = new Connection(process.env.RPC_URL || 'https://api.mainnet-beta.solana.com', 'confirmed');
const PROGRAM_ID = new PublicKey(process.env.PROGRAM_ID || '11111111111111111111111111111111');

// ── Wallet helpers ──

function generateWallet() {
  const kp = Keypair.generate();
  return {
    publicKey: kp.publicKey.toBase58(),
    privateKey: bs58.encode(kp.secretKey),
  };
}

function keypairFromSecret(secret) {
  return Keypair.fromSecretKey(bs58.decode(secret));
}

async function getBalance(pubkey) {
  try {
    const bal = await connection.getBalance(new PublicKey(pubkey));
    return bal / LAMPORTS_PER_SOL;
  } catch {
    return 0;
  }
}

// ── Bonding Curve (stub — replace with real program interaction) ──

async function launchToken(user, name, symbol) {
  // TODO: Build and send the actual create-token instruction to the Send.it program
  // For now, return a mock mint address
  const mint = Keypair.generate().publicKey.toBase58();
  return {
    success: true,
    mint,
    name,
    symbol,
    txSig: 'simulated_' + mint.slice(0, 8),
  };
}

async function buyToken(user, mint, solAmount) {
  // TODO: Build buy instruction against bonding curve
  const price = 0.000001; // mock price
  const tokensReceived = solAmount / price;
  return {
    success: true,
    tokensReceived,
    price,
    txSig: 'simulated_buy_' + Date.now().toString(36),
  };
}

async function sellToken(user, mint, tokenAmount) {
  // TODO: Build sell instruction against bonding curve
  const price = 0.0000012; // mock price
  const solReceived = tokenAmount * price;
  return {
    success: true,
    solReceived,
    price,
    txSig: 'simulated_sell_' + Date.now().toString(36),
  };
}

async function getTokenPrice(mint) {
  // TODO: Read bonding curve state on-chain
  return {
    price: 0.000001 + Math.random() * 0.00001,
    mcap: Math.floor(Math.random() * 500000),
    liquidity: Math.floor(Math.random() * 100000),
    volume24h: Math.floor(Math.random() * 200000),
    graduationPct: Math.floor(Math.random() * 100),
  };
}

module.exports = { connection, PROGRAM_ID, generateWallet, keypairFromSecret, getBalance, launchToken, buyToken, sellToken, getTokenPrice };
