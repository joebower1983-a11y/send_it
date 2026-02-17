/**
 * Send.it AMM Trading UI — Devnet
 * Swap, Add/Remove Liquidity on graduated token pools
 * Program: HTKq18cATdwCZb6XM66Mhn8JWKCFTrZqH6zU1zip88Zx
 */

const PROGRAM_ID_STR = 'HTKq18cATdwCZb6XM66Mhn8JWKCFTrZqH6zU1zip88Zx';
const RPC = 'https://api.devnet.solana.com';
const TOKEN_DECIMALS = 6;
const SWAP_FEE_BPS = 100;
const LP_FEE_BPS = 30;
const PROTOCOL_FEE_BPS = 70;
const SLIPPAGE_BPS = 100; // 1% default slippage

// Seeds
const POOL_SEED = 'amm_pool';
const POOL_SOL_VAULT_SEED = 'pool_sol_vault';
const PLATFORM_CONFIG_SEED = 'platform_config';
const PLATFORM_VAULT_SEED = 'platform_vault';
const TOKEN_LAUNCH_SEED = 'token_launch';

// Globals
let connection = null;
let walletPubkey = null;
let PROGRAM = null;
let selectedPool = null;
let pools = [];
let swapDirection = 'buy'; // 'buy' = SOL→Token, 'sell' = Token→SOL

// ─── Init ───
function init() {
  const { Connection, PublicKey } = solanaWeb3;
  connection = new Connection(RPC, 'confirmed');
  PROGRAM = new PublicKey(PROGRAM_ID_STR);
}
init();

// ─── Wallet ───
async function connectWallet() {
  const provider = window.phantom?.solana || window.solflare;
  if (!provider) {
    alert('Please install Phantom or Solflare wallet');
    return;
  }
  try {
    const resp = await provider.connect();
    walletPubkey = resp.publicKey;
    document.getElementById('walletBtn').innerHTML = `<i class="fas fa-wallet"></i>&nbsp; ${walletPubkey.toBase58().slice(0,4)}...${walletPubkey.toBase58().slice(-4)}`;
    document.getElementById('swapBtn').disabled = false;
    document.getElementById('swapBtn').textContent = '⚡ Swap';
    document.getElementById('addLiqBtn').disabled = false;
    document.getElementById('removeLiqBtn').disabled = false;
    await loadPools();
    await updateBalances();
  } catch (e) {
    console.error('Wallet connect failed:', e);
  }
}

// ─── PDA Helpers ───
function findPda(seeds) {
  const { PublicKey } = solanaWeb3;
  return PublicKey.findProgramAddressSync(
    seeds.map(s => typeof s === 'string' ? new TextEncoder().encode(s) : s),
    PROGRAM
  );
}

function getAta(mint, owner) {
  const { PublicKey } = solanaWeb3;
  const SPL_ATA = new PublicKey('ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL');
  const TOKEN_PROGRAM = new PublicKey('TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA');
  return PublicKey.findProgramAddressSync(
    [owner.toBuffer(), TOKEN_PROGRAM.toBuffer(), mint.toBuffer()],
    SPL_ATA
  )[0];
}

// ─── Load Pools ───
async function loadPools() {
  const { PublicKey } = solanaWeb3;
  try {
    // Fetch all AmmPool accounts (discriminator for AmmPool)
    const accounts = await connection.getProgramAccounts(PROGRAM, {
      filters: [{ dataSize: 122 }] // AmmPool::SIZE = 8+32+8+8+32+8+8+8+8+1+1 = 122
    });

    pools = accounts.map(({ pubkey, account }) => {
      const data = account.data;
      const mint = new PublicKey(data.subarray(8, 40));
      const tokenReserve = Number(readU64(data, 40));
      const solReserve = Number(readU64(data, 48));
      const lpMint = new PublicKey(data.subarray(56, 88));
      const lpSupply = Number(readU64(data, 88));
      const totalFeesSol = Number(readU64(data, 96));
      const totalFeesToken = Number(readU64(data, 104));
      const createdAt = Number(readI64(data, 112));
      const bump = data[120];
      const poolSolVaultBump = data[121];

      return {
        pubkey, mint, tokenReserve, solReserve, lpMint,
        lpSupply, totalFeesSol, totalFeesToken, createdAt,
        bump, poolSolVaultBump,
        symbol: mint.toBase58().slice(0, 6), // placeholder
      };
    });

    renderPools();
    updateStats();
    if (pools.length > 0) selectPool(0);
  } catch (e) {
    console.error('Failed to load pools:', e);
  }
}

function readU64(buf, offset) {
  let val = BigInt(0);
  for (let i = 0; i < 8; i++) val |= BigInt(buf[offset + i]) << BigInt(i * 8);
  return val;
}
function readI64(buf, offset) {
  return readU64(buf, offset); // simplified
}

function renderPools() {
  const list = document.getElementById('poolList');
  if (pools.length === 0) {
    list.innerHTML = `<div style="text-align:center;padding:32px 12px;color:var(--text3)">
      <i class="fas fa-water" style="font-size:28px;margin-bottom:8px;display:block;color:var(--accent)"></i>
      <div style="font-size:13px">No graduated pools yet</div>
      <div style="font-size:11px;margin-top:4px">Launch a token and buy enough to hit the migration threshold</div>
    </div>`;
    return;
  }

  list.innerHTML = pools.map((p, i) => `
    <div class="pool-item ${i === 0 ? 'active' : ''}" onclick="selectPool(${i})" id="pool-${i}">
      <div class="pool-icon">🏊</div>
      <div>
        <div class="pool-name">${p.symbol}/SOL</div>
        <div class="pool-pair">${p.mint.toBase58().slice(0, 8)}...</div>
        <div class="pool-tvl">TVL: ${(p.solReserve / 1e9).toFixed(4)} SOL</div>
      </div>
    </div>
  `).join('');
}

function selectPool(idx) {
  selectedPool = pools[idx];
  document.querySelectorAll('.pool-item').forEach((el, i) => {
    el.classList.toggle('active', i === idx);
  });
  document.getElementById('swapOutputSymbol').textContent = selectedPool.symbol;
  updatePoolInfo();
  updateBalances();
  calculateSwapOutput();
}

function updateStats() {
  document.getElementById('stat-pools').textContent = pools.length;
  const totalTvl = pools.reduce((s, p) => s + p.solReserve, 0) / 1e9;
  document.getElementById('stat-tvl').textContent = totalTvl.toFixed(2);
  const totalFees = pools.reduce((s, p) => s + p.totalFeesSol, 0) / 1e9;
  document.getElementById('stat-vol').textContent = totalFees > 0 ? (totalFees / 0.01).toFixed(2) : '—'; // estimate from fees
}

function updatePoolInfo() {
  if (!selectedPool) return;
  const p = selectedPool;
  const tokenRes = (p.tokenReserve / 1e6).toLocaleString();
  const solRes = (p.solReserve / 1e9).toFixed(4);
  const price = p.tokenReserve > 0 ? (p.solReserve / p.tokenReserve * 1e3).toFixed(8) : '—';

  document.getElementById('poolInfoContent').innerHTML = `
    <div style="text-align:left">
      <div style="display:flex;justify-content:space-between;padding:6px 0;border-bottom:1px solid rgba(255,255,255,.04)">
        <span style="color:var(--text2)">Token Mint</span>
        <a href="https://solscan.io/token/${p.mint.toBase58()}?cluster=devnet" target="_blank" class="mono" style="color:var(--accent);font-size:12px">${p.mint.toBase58().slice(0,8)}...</a>
      </div>
      <div style="display:flex;justify-content:space-between;padding:6px 0;border-bottom:1px solid rgba(255,255,255,.04)">
        <span style="color:var(--text2)">Token Reserve</span>
        <span class="mono">${tokenRes}</span>
      </div>
      <div style="display:flex;justify-content:space-between;padding:6px 0;border-bottom:1px solid rgba(255,255,255,.04)">
        <span style="color:var(--text2)">SOL Reserve</span>
        <span class="mono">${solRes} SOL</span>
      </div>
      <div style="display:flex;justify-content:space-between;padding:6px 0;border-bottom:1px solid rgba(255,255,255,.04)">
        <span style="color:var(--text2)">Price</span>
        <span class="mono">${price} SOL</span>
      </div>
      <div style="display:flex;justify-content:space-between;padding:6px 0;border-bottom:1px solid rgba(255,255,255,.04)">
        <span style="color:var(--text2)">LP Supply</span>
        <span class="mono">${(p.lpSupply / 1e6).toFixed(2)}</span>
      </div>
      <div style="display:flex;justify-content:space-between;padding:6px 0;border-bottom:1px solid rgba(255,255,255,.04)">
        <span style="color:var(--text2)">Total Fees (SOL)</span>
        <span class="mono">${(p.totalFeesSol / 1e9).toFixed(6)}</span>
      </div>
      <div style="display:flex;justify-content:space-between;padding:6px 0">
        <span style="color:var(--text2)">Pool Address</span>
        <a href="https://solscan.io/account/${p.pubkey.toBase58()}?cluster=devnet" target="_blank" class="mono" style="color:var(--accent);font-size:12px">${p.pubkey.toBase58().slice(0,8)}...</a>
      </div>
    </div>
  `;

  // Update LP stats
  document.getElementById('lpPoolTokens').textContent = (p.tokenReserve / 1e6).toFixed(0);
  document.getElementById('lpPoolSol').textContent = (p.solReserve / 1e9).toFixed(4);
}

async function updateBalances() {
  if (!walletPubkey) return;
  try {
    const solBal = await connection.getBalance(walletPubkey);
    if (swapDirection === 'buy') {
      document.getElementById('swapInputBalance').textContent = (solBal / 1e9).toFixed(4) + ' SOL';
    } else {
      document.getElementById('swapOutputBalance').textContent = (solBal / 1e9).toFixed(4) + ' SOL';
    }

    if (selectedPool) {
      const tokenAta = getAta(selectedPool.mint, walletPubkey);
      try {
        const tokenBal = await connection.getTokenAccountBalance(tokenAta);
        const bal = tokenBal.value.uiAmountString;
        if (swapDirection === 'buy') {
          document.getElementById('swapOutputBalance').textContent = bal;
        } else {
          document.getElementById('swapInputBalance').textContent = bal;
        }
      } catch { /* no token account yet */ }

      // LP balance
      const lpAta = getAta(selectedPool.lpMint, walletPubkey);
      try {
        const lpBal = await connection.getTokenAccountBalance(lpAta);
        document.getElementById('lpYourLp').textContent = lpBal.value.uiAmountString;
        const share = selectedPool.lpSupply > 0
          ? (Number(lpBal.value.amount) / selectedPool.lpSupply * 100).toFixed(2) + '%'
          : '0%';
        document.getElementById('lpYourShare').textContent = share;
      } catch {
        document.getElementById('lpYourLp').textContent = '0';
        document.getElementById('lpYourShare').textContent = '0%';
      }
    }
  } catch (e) {
    console.error('Balance update failed:', e);
  }
}

// ─── Swap Logic ───
function calculateSwapOutput() {
  if (!selectedPool) return;
  const input = parseFloat(document.getElementById('swapInputAmount').value) || 0;
  if (input <= 0) {
    document.getElementById('swapOutputAmount').value = '';
    document.getElementById('swapDetails').style.display = 'none';
    return;
  }

  const p = selectedPool;
  let output, fee, impact;

  if (swapDirection === 'buy') {
    // SOL → Token
    const solIn = Math.floor(input * 1e9);
    fee = Math.floor(solIn * SWAP_FEE_BPS / 10000);
    const netSol = solIn - fee;
    const newSolReserve = p.solReserve + netSol;
    const tokenOut = p.tokenReserve - Math.floor(p.tokenReserve * p.solReserve / newSolReserve);
    output = tokenOut / 1e6;
    impact = netSol / p.solReserve * 100;

    document.getElementById('swapRate').textContent = `1 SOL = ${(tokenOut / input / 1e6).toFixed(0)} ${p.symbol}`;
    document.getElementById('swapFee').textContent = `${(fee / 1e9).toFixed(6)} SOL`;
    document.getElementById('swapLpFee').textContent = `${(fee * LP_FEE_BPS / SWAP_FEE_BPS / 1e9).toFixed(6)} SOL`;
    document.getElementById('swapProtocolFee').textContent = `${(fee * PROTOCOL_FEE_BPS / SWAP_FEE_BPS / 1e9).toFixed(6)} SOL`;
    document.getElementById('swapMinReceived').textContent = `${(output * (1 - SLIPPAGE_BPS / 10000)).toFixed(2)} ${p.symbol}`;
  } else {
    // Token → SOL
    const tokenIn = Math.floor(input * 1e6);
    const feeTokens = Math.floor(tokenIn * SWAP_FEE_BPS / 10000);
    const netTokens = tokenIn - feeTokens;
    const newTokenReserve = p.tokenReserve + netTokens;
    const solOut = p.solReserve - Math.floor(p.solReserve * p.tokenReserve / newTokenReserve);
    output = solOut / 1e9;
    fee = feeTokens;
    impact = netTokens / p.tokenReserve * 100;

    document.getElementById('swapRate').textContent = `1 ${p.symbol} = ${(solOut / input / 1e9).toFixed(8)} SOL`;
    document.getElementById('swapFee').textContent = `${(feeTokens / 1e6).toFixed(2)} ${p.symbol}`;
    document.getElementById('swapLpFee').textContent = `${(feeTokens * LP_FEE_BPS / SWAP_FEE_BPS / 1e6).toFixed(2)} ${p.symbol}`;
    document.getElementById('swapProtocolFee').textContent = `${(feeTokens * PROTOCOL_FEE_BPS / SWAP_FEE_BPS / 1e6).toFixed(2)} ${p.symbol}`;
    document.getElementById('swapMinReceived').textContent = `${(output * (1 - SLIPPAGE_BPS / 10000)).toFixed(6)} SOL`;
  }

  document.getElementById('swapOutputAmount').value = output > 0 ? output.toFixed(swapDirection === 'buy' ? 2 : 6) : '';
  document.getElementById('swapImpact').textContent = impact.toFixed(2) + '%';
  document.getElementById('swapImpact').style.color = impact > 5 ? '#ef4444' : impact > 2 ? '#ff8800' : '#22c55e';
  document.getElementById('swapDetails').style.display = output > 0 ? 'block' : 'none';
}

function flipSwap() {
  swapDirection = swapDirection === 'buy' ? 'sell' : 'buy';
  const sym = selectedPool ? selectedPool.symbol : 'TOKEN';

  if (swapDirection === 'buy') {
    document.getElementById('swapInputToken').innerHTML = `<div class="icon" style="background:linear-gradient(135deg,#9945FF,#14F195);border-radius:50%"></div> SOL`;
    document.getElementById('swapOutputToken').innerHTML = `<div class="icon" style="background:var(--accent);border-radius:50%"></div> <span id="swapOutputSymbol">${sym}</span>`;
  } else {
    document.getElementById('swapInputToken').innerHTML = `<div class="icon" style="background:var(--accent);border-radius:50%"></div> ${sym}`;
    document.getElementById('swapOutputToken').innerHTML = `<div class="icon" style="background:linear-gradient(135deg,#9945FF,#14F195);border-radius:50%"></div> <span id="swapOutputSymbol">SOL</span>`;
  }

  document.getElementById('swapInputAmount').value = '';
  document.getElementById('swapOutputAmount').value = '';
  document.getElementById('swapDetails').style.display = 'none';
  updateBalances();
}

function maxSwapInput() {
  // Set max based on balance
  const balText = document.getElementById('swapInputBalance').textContent;
  const num = parseFloat(balText);
  if (!isNaN(num) && num > 0) {
    const val = swapDirection === 'buy' ? Math.max(0, num - 0.01) : num; // leave some SOL for fees
    document.getElementById('swapInputAmount').value = val.toFixed(swapDirection === 'buy' ? 4 : 2);
    calculateSwapOutput();
  }
}

// ─── Execute Swap ───
async function executeSwap() {
  if (!walletPubkey || !selectedPool) return;
  const btn = document.getElementById('swapBtn');
  const result = document.getElementById('swapResult');
  btn.disabled = true;
  btn.textContent = '⏳ Swapping...';

  try {
    const { PublicKey, Transaction, SystemProgram, ComputeBudgetProgram } = solanaWeb3;
    const TOKEN_PROGRAM = new PublicKey('TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA');
    const ATA_PROGRAM = new PublicKey('ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL');
    const p = selectedPool;
    const input = parseFloat(document.getElementById('swapInputAmount').value);

    let solAmount, tokenAmount;
    if (swapDirection === 'buy') {
      solAmount = Math.floor(input * 1e9);
      tokenAmount = 0;
    } else {
      solAmount = 0;
      tokenAmount = Math.floor(input * 1e6);
    }

    const [ammPool] = findPda([POOL_SEED, p.mint.toBuffer()]);
    const [poolSolVault] = findPda([POOL_SOL_VAULT_SEED, p.mint.toBuffer()]);
    const [platformVault] = findPda([PLATFORM_VAULT_SEED]);
    const poolTokenVault = getAta(p.mint, ammPool);
    const userTokenAccount = getAta(p.mint, walletPubkey);

    // Build swap discriminator
    const disc = await anchorDisc('swap');
    const data = new Uint8Array(8 + 8 + 8);
    data.set(disc, 0);
    writeU64(data, 8, BigInt(solAmount));
    writeU64(data, 16, BigInt(tokenAmount));

    const tx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 300000 }),
      {
        programId: PROGRAM,
        keys: [
          { pubkey: ammPool, isSigner: false, isWritable: true },
          { pubkey: p.mint, isSigner: false, isWritable: false },
          { pubkey: poolTokenVault, isSigner: false, isWritable: true },
          { pubkey: poolSolVault, isSigner: false, isWritable: true },
          { pubkey: userTokenAccount, isSigner: false, isWritable: true },
          { pubkey: platformVault, isSigner: false, isWritable: true },
          { pubkey: walletPubkey, isSigner: true, isWritable: true },
          { pubkey: TOKEN_PROGRAM, isSigner: false, isWritable: false },
          { pubkey: ATA_PROGRAM, isSigner: false, isWritable: false },
          { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
        ],
        data,
      }
    );

    tx.feePayer = walletPubkey;
    tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;

    const provider = window.phantom?.solana || window.solflare;
    const signed = await provider.signTransaction(tx);
    const sig = await connection.sendRawTransaction(signed.serialize());
    await connection.confirmTransaction(sig, 'confirmed');

    result.style.display = 'block';
    result.className = 'tx-result';
    result.innerHTML = `✅ Swap successful! <a href="https://solscan.io/tx/${sig}?cluster=devnet" target="_blank">View on Solscan →</a>`;

    await loadPools();
    await updateBalances();
  } catch (e) {
    result.style.display = 'block';
    result.className = 'tx-result error';
    result.innerHTML = `❌ ${e.message}`;
    console.error('Swap failed:', e);
  }

  btn.disabled = false;
  btn.textContent = '⚡ Swap';
}

// ─── Liquidity ───
function calculateLpTokens() {
  if (!selectedPool) return;
  const solIn = parseFloat(document.getElementById('addLiqSol').value) || 0;
  if (solIn <= 0 || selectedPool.solReserve === 0) {
    document.getElementById('addLiqTokens').value = '';
    document.getElementById('addLiqLpOut').textContent = '—';
    return;
  }

  const p = selectedPool;
  const tokenAmount = solIn * 1e9 * p.tokenReserve / p.solReserve / 1e6;
  const lpOut = p.lpSupply * solIn * 1e9 / p.solReserve / 1e6;

  document.getElementById('addLiqTokens').value = tokenAmount.toFixed(2);
  document.getElementById('addLiqLpOut').textContent = lpOut.toFixed(4);
}

function calculateRemoveOutput() {
  if (!selectedPool) return;
  const lpIn = parseFloat(document.getElementById('removeLiqAmount').value) || 0;
  if (lpIn <= 0 || selectedPool.lpSupply === 0) {
    document.getElementById('removeSolOut').textContent = '—';
    document.getElementById('removeTokenOut').textContent = '—';
    return;
  }

  const p = selectedPool;
  const lpRaw = lpIn * 1e6;
  const solOut = lpRaw * p.solReserve / p.lpSupply / 1e9;
  const tokenOut = lpRaw * p.tokenReserve / p.lpSupply / 1e6;

  document.getElementById('removeSolOut').textContent = solOut.toFixed(6) + ' SOL';
  document.getElementById('removeTokenOut').textContent = tokenOut.toFixed(2) + ' ' + p.symbol;
}

function setRemovePct(pct) {
  const lpText = document.getElementById('lpYourLp').textContent;
  const lpBal = parseFloat(lpText) || 0;
  document.getElementById('removeLiqAmount').value = (lpBal * pct / 100).toFixed(6);
  calculateRemoveOutput();
}

async function executeAddLiquidity() {
  if (!walletPubkey || !selectedPool) return;
  const btn = document.getElementById('addLiqBtn');
  const result = document.getElementById('addLiqResult');
  btn.disabled = true;
  btn.textContent = '⏳ Adding...';

  try {
    const { PublicKey, Transaction, SystemProgram, ComputeBudgetProgram } = solanaWeb3;
    const TOKEN_PROGRAM = new PublicKey('TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA');
    const ATA_PROGRAM = new PublicKey('ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL');
    const p = selectedPool;
    const solIn = parseFloat(document.getElementById('addLiqSol').value);
    const solAmount = Math.floor(solIn * 1e9);

    const [ammPool] = findPda([POOL_SEED, p.mint.toBuffer()]);
    const [poolSolVault] = findPda([POOL_SOL_VAULT_SEED, p.mint.toBuffer()]);
    const poolTokenVault = getAta(p.mint, ammPool);
    const userTokenAccount = getAta(p.mint, walletPubkey);
    const userLpAccount = getAta(p.lpMint, walletPubkey);

    const disc = await anchorDisc('add_liquidity');
    const data = new Uint8Array(8 + 8);
    data.set(disc, 0);
    writeU64(data, 8, BigInt(solAmount));

    const tx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 300000 }),
      {
        programId: PROGRAM,
        keys: [
          { pubkey: ammPool, isSigner: false, isWritable: true },
          { pubkey: p.mint, isSigner: false, isWritable: false },
          { pubkey: poolTokenVault, isSigner: false, isWritable: true },
          { pubkey: poolSolVault, isSigner: false, isWritable: true },
          { pubkey: p.lpMint, isSigner: false, isWritable: true },
          { pubkey: userTokenAccount, isSigner: false, isWritable: true },
          { pubkey: userLpAccount, isSigner: false, isWritable: true },
          { pubkey: walletPubkey, isSigner: true, isWritable: true },
          { pubkey: TOKEN_PROGRAM, isSigner: false, isWritable: false },
          { pubkey: ATA_PROGRAM, isSigner: false, isWritable: false },
          { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
        ],
        data,
      }
    );

    tx.feePayer = walletPubkey;
    tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
    const provider = window.phantom?.solana || window.solflare;
    const signed = await provider.signTransaction(tx);
    const sig = await connection.sendRawTransaction(signed.serialize());
    await connection.confirmTransaction(sig, 'confirmed');

    result.style.display = 'block';
    result.className = 'tx-result';
    result.innerHTML = `✅ Liquidity added! <a href="https://solscan.io/tx/${sig}?cluster=devnet" target="_blank">View →</a>`;
    await loadPools();
    await updateBalances();
  } catch (e) {
    result.style.display = 'block';
    result.className = 'tx-result error';
    result.innerHTML = `❌ ${e.message}`;
  }
  btn.disabled = false;
  btn.textContent = 'Add Liquidity';
}

async function executeRemoveLiquidity() {
  if (!walletPubkey || !selectedPool) return;
  const btn = document.getElementById('removeLiqBtn');
  const result = document.getElementById('removeLiqResult');
  btn.disabled = true;
  btn.textContent = '⏳ Removing...';

  try {
    const { PublicKey, Transaction, SystemProgram, ComputeBudgetProgram } = solanaWeb3;
    const TOKEN_PROGRAM = new PublicKey('TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA');
    const ATA_PROGRAM = new PublicKey('ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL');
    const p = selectedPool;
    const lpIn = parseFloat(document.getElementById('removeLiqAmount').value);
    const lpAmount = Math.floor(lpIn * 1e6);

    const [ammPool] = findPda([POOL_SEED, p.mint.toBuffer()]);
    const [poolSolVault] = findPda([POOL_SOL_VAULT_SEED, p.mint.toBuffer()]);
    const poolTokenVault = getAta(p.mint, ammPool);
    const userTokenAccount = getAta(p.mint, walletPubkey);
    const userLpAccount = getAta(p.lpMint, walletPubkey);

    const disc = await anchorDisc('remove_liquidity');
    const data = new Uint8Array(8 + 8);
    data.set(disc, 0);
    writeU64(data, 8, BigInt(lpAmount));

    const tx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 300000 }),
      {
        programId: PROGRAM,
        keys: [
          { pubkey: ammPool, isSigner: false, isWritable: true },
          { pubkey: p.mint, isSigner: false, isWritable: false },
          { pubkey: poolTokenVault, isSigner: false, isWritable: true },
          { pubkey: poolSolVault, isSigner: false, isWritable: true },
          { pubkey: p.lpMint, isSigner: false, isWritable: true },
          { pubkey: userTokenAccount, isSigner: false, isWritable: true },
          { pubkey: userLpAccount, isSigner: false, isWritable: true },
          { pubkey: walletPubkey, isSigner: true, isWritable: true },
          { pubkey: TOKEN_PROGRAM, isSigner: false, isWritable: false },
          { pubkey: ATA_PROGRAM, isSigner: false, isWritable: false },
          { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
        ],
        data,
      }
    );

    tx.feePayer = walletPubkey;
    tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
    const provider = window.phantom?.solana || window.solflare;
    const signed = await provider.signTransaction(tx);
    const sig = await connection.sendRawTransaction(signed.serialize());
    await connection.confirmTransaction(sig, 'confirmed');

    result.style.display = 'block';
    result.className = 'tx-result';
    result.innerHTML = `✅ Liquidity removed! <a href="https://solscan.io/tx/${sig}?cluster=devnet" target="_blank">View →</a>`;
    await loadPools();
    await updateBalances();
  } catch (e) {
    result.style.display = 'block';
    result.className = 'tx-result error';
    result.innerHTML = `❌ ${e.message}`;
  }
  btn.disabled = false;
  btn.textContent = 'Remove Liquidity';
}

// ─── Tabs ───
function switchTab(tab) {
  document.querySelectorAll('.tabs .tab')[0].classList.toggle('active', tab === 'swap');
  document.querySelectorAll('.tabs .tab')[1].classList.toggle('active', tab === 'liquidity');
  document.getElementById('swapTab').style.display = tab === 'swap' ? 'block' : 'none';
  document.getElementById('liquidityTab').style.display = tab === 'liquidity' ? 'block' : 'none';
}

function switchLiqTab(tab) {
  document.getElementById('addLiqTab').classList.toggle('active', tab === 'add');
  document.getElementById('removeLiqTab').classList.toggle('active', tab === 'remove');
  document.getElementById('addLiqPanel').style.display = tab === 'add' ? 'block' : 'none';
  document.getElementById('removeLiqPanel').style.display = tab === 'remove' ? 'block' : 'none';
}

// ─── Helpers ───
async function anchorDisc(name) {
  const msgBuffer = new TextEncoder().encode(`global:${name}`);
  const hashBuffer = await crypto.subtle.digest('SHA-256', msgBuffer);
  return new Uint8Array(hashBuffer).slice(0, 8);
}

function writeU64(buf, offset, val) {
  for (let i = 0; i < 8; i++) {
    buf[offset + i] = Number(val & 0xFFn);
    val >>= 8n;
  }
}

// Pool search
document.getElementById('poolSearch')?.addEventListener('input', (e) => {
  const q = e.target.value.toLowerCase();
  document.querySelectorAll('.pool-item').forEach((el, i) => {
    const pool = pools[i];
    const match = pool.symbol.toLowerCase().includes(q) || pool.mint.toBase58().toLowerCase().includes(q);
    el.style.display = match ? '' : 'none';
  });
});
