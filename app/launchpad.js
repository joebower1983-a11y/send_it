/**
 * Send.it Launchpad — Devnet Integration
 * Real wallet connect + on-chain token creation, buying, selling
 * Program: HTKq18cATdwCZb6XM66Mhn8JWKCFTrZqH6zU1zip88Zx
 */

const PROGRAM_ID = 'program_id_placeholder'; // replaced below
const RPC = 'https://api.devnet.solana.com';
const TAPESTRY_KEY = '601a8251-9c95-4456-97af-c1e79b5c0259';
const TAPESTRY_API = 'https://api.usetapestry.dev/v1';

// We use raw web3.js via CDN — loaded in HTML
// solanaWeb3 = window.solanaWeb3

let connection = null;
let walletAdapter = null; // Phantom/Solflare provider
let walletPubkey = null;

// ─── Constants ───
const PROGRAM = null; // set after web3 loads
const SEEDS = {
  platformConfig: 'platform_config',
  tokenLaunch: 'token_launch',
  platformVault: 'platform_vault',
  solVault: 'sol_vault',
  userPosition: 'user_position',
};

// ─── Init ───
async function initLaunchpad() {
  const { Connection, PublicKey } = solanaWeb3;
  connection = new Connection(RPC, 'confirmed');
  window.PROGRAM_KEY = new PublicKey('HTKq18cATdwCZb6XM66Mhn8JWKCFTrZqH6zU1zip88Zx');
  console.log('[Launchpad] Initialized, RPC:', RPC);
  updatePlatformStats();
}

// ─── Wallet ───
async function connectWallet() {
  const { PublicKey } = solanaWeb3;
  const btn = document.querySelector('.wallet-btn');

  try {
    if (window.solana?.isPhantom) {
      const resp = await window.solana.connect();
      walletAdapter = window.solana;
      walletPubkey = resp.publicKey;
    } else if (window.solflare?.isSolflare) {
      await window.solflare.connect();
      walletAdapter = window.solflare;
      walletPubkey = window.solflare.publicKey;
    } else {
      showToast('No wallet found — install Phantom or Solflare');
      return;
    }

    btn.innerHTML = `<i class="fas fa-wallet"></i>&nbsp; ${walletPubkey.toBase58().slice(0,4)}...${walletPubkey.toBase58().slice(-4)}`;
    btn.style.background = 'var(--bg3)';
    btn.style.color = 'var(--accent)';
    btn.style.border = '1px solid rgba(0,200,150,.3)';

    showToast('Wallet connected! ✅');
    updateBalance();
    loadLiveTokens();
  } catch (e) {
    console.error('Wallet connect failed:', e);
    showToast('Connection failed: ' + e.message);
  }
}

async function updateBalance() {
  if (!walletPubkey) return;
  const bal = await connection.getBalance(walletPubkey);
  const el = document.getElementById('wallet-balance');
  if (el) el.textContent = (bal / 1e9).toFixed(4) + ' SOL';
}

// ─── Helpers ───
function sha256(data) {
  // Simple discriminator hash
  return crypto.subtle.digest('SHA-256', new TextEncoder().encode(data));
}

async function getDiscriminator(name) {
  const hash = await sha256(`global:${name}`);
  return new Uint8Array(hash).slice(0, 8);
}

function encodeString(s) {
  const buf = new Uint8Array(4 + s.length);
  new DataView(buf.buffer).setUint32(0, s.length, true);
  new TextEncoder().encodeInto(s, buf.subarray(4));
  return buf;
}

function concat(...arrays) {
  const total = arrays.reduce((s, a) => s + a.length, 0);
  const result = new Uint8Array(total);
  let offset = 0;
  for (const a of arrays) { result.set(a, offset); offset += a.length; }
  return result;
}

function findPDA(seeds) {
  const { PublicKey } = solanaWeb3;
  return PublicKey.findProgramAddressSync(
    seeds.map(s => typeof s === 'string' ? new TextEncoder().encode(s) : s),
    window.PROGRAM_KEY
  );
}

function getATA(mint, owner, allowPDA = false) {
  const { PublicKey } = solanaWeb3;
  const TOKEN_PROGRAM = new PublicKey('TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA');
  const ATA_PROGRAM = new PublicKey('ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL');
  const [ata] = PublicKey.findProgramAddressSync(
    [owner.toBuffer(), TOKEN_PROGRAM.toBuffer(), mint.toBuffer()],
    ATA_PROGRAM
  );
  return ata;
}

// ─── Create Token ───
async function createToken() {
  if (!walletPubkey) { showToast('Connect wallet first'); return; }

  const { PublicKey, Transaction, TransactionInstruction, SystemProgram, Keypair, SYSVAR_RENT_PUBKEY } = solanaWeb3;
  const TOKEN_PROGRAM = new PublicKey('TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA');
  const ATA_PROGRAM = new PublicKey('ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL');
  const MPL_TOKEN_METADATA = new PublicKey('metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s');

  const name = document.getElementById('tokenName')?.value?.trim() || 'SendIt Token';
  const symbol = document.getElementById('tokenSymbol')?.value?.trim() || 'SENDIT';
  const uri = document.getElementById('tokenUri')?.value?.trim() || 'https://senditsolana.io';

  const launchBtn = document.getElementById('launchBtn');
  launchBtn.disabled = true;
  launchBtn.textContent = '⏳ Creating...';

  try {
    const mintKeypair = Keypair.generate();
    const mint = mintKeypair.publicKey;
    const [platformConfig] = findPDA([SEEDS.platformConfig]);
    const [tokenLaunch] = findPDA([SEEDS.tokenLaunch, mint.toBuffer()]);
    const [solVault] = findPDA([SEEDS.solVault, mint.toBuffer()]);
    const launchVault = getATA(mint, tokenLaunch);

    // Derive Metaplex metadata PDA
    const [metadata] = PublicKey.findProgramAddressSync(
      [new TextEncoder().encode('metadata'), MPL_TOKEN_METADATA.toBuffer(), mint.toBuffer()],
      MPL_TOKEN_METADATA
    );

    const disc = await getDiscriminator('create_token');
    const data = concat(disc, encodeString(name), encodeString(symbol), encodeString(uri), new Uint8Array([0xF4, 0x01]));

    const ix = new TransactionInstruction({
      keys: [
        { pubkey: tokenLaunch, isSigner: false, isWritable: true },
        { pubkey: mint, isSigner: true, isWritable: true },
        { pubkey: launchVault, isSigner: false, isWritable: true },
        { pubkey: solVault, isSigner: false, isWritable: true },
        { pubkey: metadata, isSigner: false, isWritable: true },
        { pubkey: platformConfig, isSigner: false, isWritable: true },
        { pubkey: walletPubkey, isSigner: true, isWritable: true },
        { pubkey: TOKEN_PROGRAM, isSigner: false, isWritable: false },
        { pubkey: ATA_PROGRAM, isSigner: false, isWritable: false },
        { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
        { pubkey: SYSVAR_RENT_PUBKEY, isSigner: false, isWritable: false },
        { pubkey: MPL_TOKEN_METADATA, isSigner: false, isWritable: false },
      ],
      programId: window.PROGRAM_KEY,
      data,
    });

    const tx = new Transaction().add(ix);
    tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
    tx.feePayer = walletPubkey;
    tx.partialSign(mintKeypair);

    const signed = await walletAdapter.signTransaction(tx);
    const sig = await connection.sendRawTransaction(signed.serialize());
    await connection.confirmTransaction(sig, 'confirmed');

    showToast(`🚀 Token created! Mint: ${mint.toBase58().slice(0,8)}...`);
    showTxResult('create', sig, mint.toBase58());

    // Post to Tapestry social
    try {
      await fetch(`${TAPESTRY_API}/contents/create?apiKey=${TAPESTRY_KEY}`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          profileId: walletPubkey.toBase58().slice(0, 8),
          content: `🚀 Launched ${name} ($${symbol}) on Send.it!\n\nMint: ${mint.toBase58()}`,
          contentType: 'text',
          customProperties: [
            { key: 'type', value: 'token_launch' },
            { key: 'mint', value: mint.toBase58() },
            { key: 'name', value: name },
            { key: 'symbol', value: symbol },
            { key: 'app', value: 'sendit' },
          ],
          blockchain: 'SOLANA',
          execution: 'FAST_UNCONFIRMED'
        })
      });
    } catch (e) { console.warn('Tapestry post failed:', e); }

    loadLiveTokens();
    updateBalance();

  } catch (e) {
    console.error('Create token failed:', e);
    showToast('❌ Launch failed: ' + (e.message || e));
  } finally {
    launchBtn.disabled = false;
    launchBtn.textContent = '🚀 Launch Token';
  }
}

// ─── Buy ───
async function buyToken(mintStr, solAmount = 0.01) {
  if (!walletPubkey) { showToast('Connect wallet first'); return; }

  const { PublicKey, Transaction, TransactionInstruction, SystemProgram } = solanaWeb3;
  const TOKEN_PROGRAM = new PublicKey('TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA');
  const ATA_PROGRAM = new PublicKey('ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL');

  const mint = new PublicKey(mintStr);
  const [platformConfig] = findPDA([SEEDS.platformConfig]);
  const [tokenLaunch] = findPDA([SEEDS.tokenLaunch, mint.toBuffer()]);
  const [solVault] = findPDA([SEEDS.solVault, mint.toBuffer()]);
  const [platformVault] = findPDA([SEEDS.platformVault]);
  const launchVault = getATA(mint, tokenLaunch);
  const buyerAta = getATA(mint, walletPubkey);
  const [userPosition] = findPDA([SEEDS.userPosition, walletPubkey.toBuffer(), mint.toBuffer()]);

  showToast(`⏳ Buying with ${solAmount} SOL...`);

  try {
    const tx = new Transaction();

    // Pre-fund vaults if needed
    const rentExempt = await connection.getMinimumBalanceForRentExemption(0);
    const svInfo = await connection.getAccountInfo(solVault);
    const pvInfo = await connection.getAccountInfo(platformVault);
    if (!svInfo) tx.add(SystemProgram.transfer({ fromPubkey: walletPubkey, toPubkey: solVault, lamports: rentExempt }));
    if (!pvInfo) tx.add(SystemProgram.transfer({ fromPubkey: walletPubkey, toPubkey: platformVault, lamports: rentExempt }));

    const lamports = BigInt(Math.round(solAmount * 1e9));
    const disc = await getDiscriminator('buy');
    const data = new Uint8Array(16);
    data.set(disc, 0);
    new DataView(data.buffer).setBigUint64(8, lamports, true);

    tx.add(new TransactionInstruction({
      keys: [
        { pubkey: tokenLaunch, isSigner: false, isWritable: true },
        { pubkey: mint, isSigner: false, isWritable: false },
        { pubkey: launchVault, isSigner: false, isWritable: true },
        { pubkey: solVault, isSigner: false, isWritable: true },
        { pubkey: buyerAta, isSigner: false, isWritable: true },
        { pubkey: userPosition, isSigner: false, isWritable: true },
        { pubkey: platformVault, isSigner: false, isWritable: true },
        { pubkey: walletPubkey, isSigner: false, isWritable: true }, // creator
        { pubkey: platformConfig, isSigner: false, isWritable: true },
        { pubkey: walletPubkey, isSigner: true, isWritable: true },
        { pubkey: TOKEN_PROGRAM, isSigner: false, isWritable: false },
        { pubkey: ATA_PROGRAM, isSigner: false, isWritable: false },
        { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      ],
      programId: window.PROGRAM_KEY,
      data,
    }));

    tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
    tx.feePayer = walletPubkey;
    const signed = await walletAdapter.signTransaction(tx);
    const sig = await connection.sendRawTransaction(signed.serialize());
    await connection.confirmTransaction(sig, 'confirmed');

    showToast('💚 Buy succeeded!');
    showTxResult('buy', sig, mintStr);
    updateBalance();
  } catch (e) {
    console.error('Buy failed:', e);
    showToast('❌ Buy failed: ' + (e.message || e));
  }
}

// ─── Sell ───
async function sellToken(mintStr, tokenAmount = 5000000) {
  if (!walletPubkey) { showToast('Connect wallet first'); return; }

  const { PublicKey, Transaction, TransactionInstruction, SystemProgram } = solanaWeb3;
  const TOKEN_PROGRAM = new PublicKey('TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA');

  const mint = new PublicKey(mintStr);
  const [platformConfig] = findPDA([SEEDS.platformConfig]);
  const [tokenLaunch] = findPDA([SEEDS.tokenLaunch, mint.toBuffer()]);
  const [solVault] = findPDA([SEEDS.solVault, mint.toBuffer()]);
  const [platformVault] = findPDA([SEEDS.platformVault]);
  const launchVault = getATA(mint, tokenLaunch);
  const buyerAta = getATA(mint, walletPubkey);
  const [userPosition] = findPDA([SEEDS.userPosition, walletPubkey.toBuffer(), mint.toBuffer()]);

  showToast(`⏳ Selling ${tokenAmount} tokens...`);

  try {
    const disc = await getDiscriminator('sell');
    const data = new Uint8Array(16);
    data.set(disc, 0);
    new DataView(data.buffer).setBigUint64(8, BigInt(tokenAmount), true);

    const tx = new Transaction().add(new TransactionInstruction({
      keys: [
        { pubkey: tokenLaunch, isSigner: false, isWritable: true },
        { pubkey: mint, isSigner: false, isWritable: false },
        { pubkey: launchVault, isSigner: false, isWritable: true },
        { pubkey: solVault, isSigner: false, isWritable: true },
        { pubkey: buyerAta, isSigner: false, isWritable: true },
        { pubkey: userPosition, isSigner: false, isWritable: true },
        { pubkey: platformVault, isSigner: false, isWritable: true },
        { pubkey: walletPubkey, isSigner: false, isWritable: true }, // creator
        { pubkey: platformConfig, isSigner: false, isWritable: true },
        { pubkey: walletPubkey, isSigner: true, isWritable: true },
        { pubkey: TOKEN_PROGRAM, isSigner: false, isWritable: false },
        { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      ],
      programId: window.PROGRAM_KEY,
      data,
    }));

    tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
    tx.feePayer = walletPubkey;
    const signed = await walletAdapter.signTransaction(tx);
    const sig = await connection.sendRawTransaction(signed.serialize());
    await connection.confirmTransaction(sig, 'confirmed');

    showToast('🔴 Sell succeeded!');
    showTxResult('sell', sig, mintStr);
    updateBalance();
  } catch (e) {
    console.error('Sell failed:', e);
    showToast('❌ Sell failed: ' + (e.message || e));
  }
}

// ─── Load Live Tokens from Chain ───
async function loadLiveTokens() {
  const { PublicKey } = solanaWeb3;
  const grid = document.getElementById('liveTokenGrid');
  if (!grid) return;

  try {
    // Fetch all token_launch PDAs owned by our program
    const accounts = await connection.getProgramAccounts(window.PROGRAM_KEY, {
      filters: [{ dataSize: 500 }] // approximate token_launch size — adjust if needed
    });

    if (accounts.length === 0) {
      // Show a demo token card with the known test mint
      grid.innerHTML = `
        <div class="card token-card" onclick="showTokenDetail('demo')">
          <div class="token-head">
            <div class="token-img">🚀</div>
            <div>
              <div class="token-name">SendIt Test V2</div>
              <div class="token-sym">SENDIT</div>
              <div class="token-time">Devnet</div>
            </div>
          </div>
          <div class="token-stats">
            <div class="stat"><div class="label">Status</div><div class="value" style="color:var(--accent)">Live</div></div>
            <div class="stat"><div class="label">Network</div><div class="value">Devnet</div></div>
            <div class="stat"><div class="label">Program</div><div class="value">98Vxqk...</div></div>
          </div>
          <div style="text-align:center;padding:8px 0">
            <a href="https://solscan.io/account/HTKq18cATdwCZb6XM66Mhn8JWKCFTrZqH6zU1zip88Zx?cluster=devnet" target="_blank" style="color:var(--accent);font-weight:700;font-size:13px"><i class="fas fa-external-link-alt"></i> View on Solscan</a>
          </div>
        </div>
      `;
      return;
    }

    grid.innerHTML = accounts.map((acc, i) => {
      const addr = acc.pubkey.toBase58();
      return `
        <div class="card token-card" onclick="showTokenDetail('${addr}')">
          <div class="token-head">
            <div class="token-img">🪙</div>
            <div>
              <div class="token-name">Token Launch #${i + 1}</div>
              <div class="token-sym">${addr.slice(0,8)}...</div>
              <div class="token-time">On-chain</div>
            </div>
          </div>
          <div class="token-stats">
            <div class="stat"><div class="label">PDA</div><div class="value">${addr.slice(0,6)}...</div></div>
            <div class="stat"><div class="label">Size</div><div class="value">${acc.account.data.length}B</div></div>
            <div class="stat"><div class="label">SOL</div><div class="value">${(acc.account.lamports / 1e9).toFixed(3)}</div></div>
          </div>
          <div style="display:flex;gap:8px;margin-top:10px">
            <button class="btn-sm" style="flex:1" onclick="event.stopPropagation();promptBuy('${addr}')">💚 Buy</button>
            <button class="btn-outline" style="flex:1" onclick="event.stopPropagation();promptSell('${addr}')">🔴 Sell</button>
          </div>
        </div>
      `;
    }).join('');
  } catch (e) {
    console.error('Load tokens failed:', e);
  }
}

// ─── Platform Stats ───
async function updatePlatformStats() {
  try {
    const accounts = await connection.getProgramAccounts(window.PROGRAM_KEY);
    const el = document.getElementById('stat-launched');
    if (el) el.textContent = accounts.length;
  } catch {}
}

// ─── Buy/Sell Prompts ───
function promptBuy(mintOrPda) {
  const amount = prompt('SOL amount to buy:', '0.01');
  if (amount && !isNaN(parseFloat(amount))) {
    buyToken(mintOrPda, parseFloat(amount));
  }
}

function promptSell(mintOrPda) {
  const amount = prompt('Token amount to sell:', '5000000');
  if (amount && !isNaN(parseInt(amount))) {
    sellToken(mintOrPda, parseInt(amount));
  }
}

// ─── Token Detail Modal ───
function showTokenDetail(addr) {
  showToast(`Token: ${addr.slice(0,12)}... — View on Solscan`);
  if (addr !== 'demo') {
    window.open(`https://solscan.io/account/${addr}?cluster=devnet`, '_blank');
  } else {
    window.open('https://solscan.io/account/HTKq18cATdwCZb6XM66Mhn8JWKCFTrZqH6zU1zip88Zx?cluster=devnet', '_blank');
  }
}

// ─── Tx Result Display ───
function showTxResult(action, sig, mint) {
  const el = document.getElementById('tx-result');
  if (!el) return;
  const emoji = action === 'create' ? '🚀' : action === 'buy' ? '💚' : '🔴';
  const label = action.charAt(0).toUpperCase() + action.slice(1);
  el.innerHTML = `
    <div class="card" style="border-color:rgba(0,200,150,.3);margin-bottom:16px;animation:fadeUp .3s">
      <div style="font-weight:800;margin-bottom:8px">${emoji} ${label} Confirmed!</div>
      <div style="font-size:12px;color:var(--text2);margin-bottom:4px">Mint: <span class="mono">${mint?.slice(0,20) || ''}...</span></div>
      <div style="font-size:12px;color:var(--text2)">Tx: <a href="https://solscan.io/tx/${sig}?cluster=devnet" target="_blank" style="color:var(--accent)" class="mono">${sig.slice(0,20)}...</a></div>
    </div>
  `;
  el.style.display = 'block';
}

// ─── Toast ───
function showToast(msg) {
  // Remove existing
  document.querySelectorAll('.toast-msg').forEach(t => t.remove());
  const toast = document.createElement('div');
  toast.className = 'toast-msg';
  toast.style.cssText = 'position:fixed;bottom:24px;left:50%;transform:translateX(-50%);background:var(--bg3);color:var(--accent);padding:12px 24px;border-radius:10px;font-size:14px;font-weight:600;border:1px solid rgba(0,200,150,.3);box-shadow:0 8px 30px rgba(0,0,0,.5);z-index:10000;animation:fadeUp .3s';
  toast.textContent = msg;
  document.body.appendChild(toast);
  setTimeout(() => toast.remove(), 4000);
}

// ─── Global ───
window.connectWallet = connectWallet;
window.createToken = createToken;
window.buyToken = buyToken;
window.sellToken = sellToken;
window.promptBuy = promptBuy;
window.promptSell = promptSell;
window.showTokenDetail = showTokenDetail;

// Init on load
document.addEventListener('DOMContentLoaded', initLaunchpad);
