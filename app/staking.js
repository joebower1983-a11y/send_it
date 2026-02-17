// Send.it Staking — Devnet
const PROGRAM_ID = new solanaWeb3.PublicKey("HTKq18cATdwCZb6XM66Mhn8JWKCFTrZqH6zU1zip88Zx");
const TOKEN_PROGRAM_ID = new solanaWeb3.PublicKey("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
const ASSOCIATED_TOKEN_PROGRAM_ID = new solanaWeb3.PublicKey("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
const SYSTEM_PROGRAM_ID = solanaWeb3.SystemProgram.programId;
const connection = new solanaWeb3.Connection("https://api.devnet.solana.com", "confirmed");

const TOKEN_LAUNCH_SEED = new TextEncoder().encode("token_launch");
const STAKE_SEED = new TextEncoder().encode("stake");
const USER_POSITION_SEED = new TextEncoder().encode("user_position");

let wallet = null;
let selectedMint = null;

// --- Wallet ---
async function connectWallet() {
  const provider = window.phantom?.solana || window.solflare;
  if (!provider) { alert("Install Phantom or Solflare wallet!"); return; }
  try {
    const resp = await provider.connect();
    wallet = provider;
    document.getElementById("wallet-status").textContent = resp.publicKey.toBase58().slice(0, 8) + "...";
    document.getElementById("connect-btn").textContent = "Connected ✓";
    loadUserTokens();
  } catch (e) { console.error("Connect failed:", e); }
}

function disc(name) {
  const encoder = new TextEncoder();
  const data = encoder.encode("global:" + name);
  return crypto.subtle.digest("SHA-256", data).then(h => new Uint8Array(h).slice(0, 8));
}

function findPDA(seeds, programId) {
  return solanaWeb3.PublicKey.findProgramAddressSync(seeds, programId);
}

function getATA(mint, owner, allowOwnerOffCurve = false) {
  return findPDA(
    [owner.toBuffer(), TOKEN_PROGRAM_ID.toBuffer(), mint.toBuffer()],
    ASSOCIATED_TOKEN_PROGRAM_ID
  )[0];
}

// --- Load user tokens from program ---
async function loadUserTokens() {
  if (!wallet) return;
  const tokenList = document.getElementById("token-list");
  tokenList.innerHTML = '<p class="loading">Loading tokens...</p>';

  try {
    // Get all token accounts for user
    const tokenAccounts = await connection.getParsedTokenAccountsByOwner(wallet.publicKey, { programId: TOKEN_PROGRAM_ID });
    const tokens = tokenAccounts.value.filter(t => {
      const amt = t.account.data.parsed.info.tokenAmount;
      return amt.uiAmount > 0;
    });

    if (tokens.length === 0) {
      tokenList.innerHTML = '<p class="empty">No tokens found. Buy some on the <a href="/launchpad.html">Launchpad</a> first!</p>';
      return;
    }

    tokenList.innerHTML = "";
    for (const t of tokens) {
      const mintKey = new solanaWeb3.PublicKey(t.account.data.parsed.info.mint);
      const amount = t.account.data.parsed.info.tokenAmount;

      // Check if this token has a launch PDA (it's a Send.it token)
      const [tokenLaunch] = findPDA([TOKEN_LAUNCH_SEED, mintKey.toBuffer()], PROGRAM_ID);
      const launchInfo = await connection.getAccountInfo(tokenLaunch);
      if (!launchInfo) continue; // Not a Send.it token

      // Check for existing stake
      const [stakeAccount] = findPDA([STAKE_SEED, wallet.publicKey.toBuffer(), mintKey.toBuffer()], PROGRAM_ID);
      const stakeInfo = await connection.getAccountInfo(stakeAccount);
      let stakeStatus = null;
      if (stakeInfo && stakeInfo.data.length > 8) {
        const data = stakeInfo.data;
        // StakeAccount: staker(32) + mint(32) + amount(8) + staked_at(8) + unlock_at(8) + claimed(1) + bump(1)
        const stakedAmount = Number(data.readBigUInt64LE(8 + 32 + 32));
        const unlockAt = Number(data.readBigInt64LE(8 + 32 + 32 + 8 + 8));
        const claimed = data[8 + 32 + 32 + 8 + 8 + 8] === 1;
        if (stakedAmount > 0 && !claimed) {
          const now = Math.floor(Date.now() / 1000);
          const stakedAt = Number(data.readBigInt64LE(8 + 32 + 32 + 8));
          const elapsed = now - stakedAt;
          // 10% APY: reward = amount * 1000 * elapsed / (10000 * 31536000)
          const estimatedReward = Math.floor(stakedAmount * 1000 * elapsed / (10000 * 31536000));
          stakeStatus = { amount: stakedAmount, unlockAt, claimed, unlocked: now >= unlockAt, stakedAt, estimatedReward };
        }
      }

      const card = document.createElement("div");
      card.className = "token-card";
      card.innerHTML = `
        <div class="token-header">
          <span class="token-mint" title="${mintKey.toBase58()}">${mintKey.toBase58().slice(0, 12)}...</span>
          <span class="token-balance">${amount.uiAmountString} tokens</span>
        </div>
        ${stakeStatus ? `
          <div class="stake-info ${stakeStatus.unlocked ? 'unlocked' : 'locked'}">
            <span>🔒 Staked: ${(stakeStatus.amount / 1e6).toLocaleString()}</span>
            <span>💰 Est. Reward: ${(stakeStatus.estimatedReward / 1e6).toLocaleString()} (10% APY)</span>
            <span>${stakeStatus.unlocked ? '✅ Unlocked — ready to claim!' : '⏳ Unlocks: ' + new Date(stakeStatus.unlockAt * 1000).toLocaleString()}</span>
            ${stakeStatus.unlocked ? `<button class="btn btn-green" onclick="unstakeTokens('${mintKey.toBase58()}')">Unstake + Claim Rewards</button>` : ''}
          </div>
        ` : ''}
        <div class="stake-form">
          <input type="number" id="stake-amount-${mintKey.toBase58()}" placeholder="Amount to stake" min="1" max="${amount.uiAmount}" step="1">
          <select id="stake-duration-${mintKey.toBase58()}">
            <option value="60">1 minute (test)</option>
            <option value="3600">1 hour</option>
            <option value="86400">1 day</option>
            <option value="604800" selected>1 week</option>
            <option value="2592000">30 days</option>
            <option value="7776000">90 days</option>
            <option value="31536000">1 year (max)</option>
          </select>
          <button class="btn btn-primary" onclick="stakeTokens('${mintKey.toBase58()}', ${amount.decimals})">Stake</button>
          <div class="reward-estimate" id="reward-est-${mintKey.toBase58()}" style="font-size:0.8rem;color:#00c896;margin-top:4px;width:100%"></div>
        </div>
        <script>
          (function(){
            const amt = document.getElementById('stake-amount-${mintKey.toBase58()}');
            const dur = document.getElementById('stake-duration-${mintKey.toBase58()}');
            const est = document.getElementById('reward-est-${mintKey.toBase58()}');
            function calc() {
              const a = parseFloat(amt.value) || 0;
              const d = parseInt(dur.value) || 0;
              const reward = a * 0.10 * d / 31536000;
              est.textContent = a > 0 ? '💰 Est. reward: ' + reward.toFixed(2) + ' tokens (10% APY)' : '';
            }
            amt.addEventListener('input', calc);
            dur.addEventListener('change', calc);
          })();
        </script>
      `;
      tokenList.appendChild(card);
    }
  } catch (e) {
    console.error("Load tokens error:", e);
    tokenList.innerHTML = `<p class="error">Error loading tokens: ${e.message}</p>`;
  }
}

// --- Stake ---
async function stakeTokens(mintStr, decimals) {
  if (!wallet) { alert("Connect wallet first!"); return; }
  const mint = new solanaWeb3.PublicKey(mintStr);
  const amountInput = document.getElementById(`stake-amount-${mintStr}`);
  const durationSelect = document.getElementById(`stake-duration-${mintStr}`);
  const amount = parseFloat(amountInput.value);
  const duration = parseInt(durationSelect.value);
  if (!amount || amount <= 0) { alert("Enter an amount to stake"); return; }

  const rawAmount = BigInt(Math.floor(amount * (10 ** decimals)));
  const statusEl = document.getElementById("tx-status");
  statusEl.textContent = "Staking tokens...";
  statusEl.className = "tx-status pending";

  try {
    const discBytes = await disc("stake");
    const data = new Uint8Array(8 + 8 + 8);
    data.set(discBytes, 0);
    const view = new DataView(data.buffer);
    view.setBigUint64(8, rawAmount, true);
    view.setBigInt64(16, BigInt(duration), true);

    const [stakeAccount] = findPDA([STAKE_SEED, wallet.publicKey.toBuffer(), mint.toBuffer()], PROGRAM_ID);
    const [tokenLaunch] = findPDA([TOKEN_LAUNCH_SEED, mint.toBuffer()], PROGRAM_ID);
    const stakerAta = getATA(mint, wallet.publicKey);
    const launchVault = getATA(mint, tokenLaunch);

    const tx = new solanaWeb3.Transaction().add(new solanaWeb3.TransactionInstruction({
      keys: [
        { pubkey: stakeAccount, isSigner: false, isWritable: true },
        { pubkey: tokenLaunch, isSigner: false, isWritable: true },
        { pubkey: mint, isSigner: false, isWritable: false },
        { pubkey: stakerAta, isSigner: false, isWritable: true },
        { pubkey: launchVault, isSigner: false, isWritable: true },
        { pubkey: wallet.publicKey, isSigner: true, isWritable: true },
        { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
        { pubkey: SYSTEM_PROGRAM_ID, isSigner: false, isWritable: false },
      ],
      programId: PROGRAM_ID,
      data: Buffer.from(data),
    }));

    tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
    tx.feePayer = wallet.publicKey;
    const signed = await wallet.signTransaction(tx);
    const sig = await connection.sendRawTransaction(signed.serialize());
    await connection.confirmTransaction(sig, "confirmed");

    statusEl.textContent = `✅ Staked! Tx: ${sig.slice(0, 20)}...`;
    statusEl.className = "tx-status success";
    statusEl.innerHTML += ` <a href="https://solscan.io/tx/${sig}?cluster=devnet" target="_blank">View on Solscan</a>`;
    loadUserTokens();
  } catch (e) {
    console.error("Stake error:", e);
    statusEl.textContent = `❌ Stake failed: ${e.message}`;
    statusEl.className = "tx-status error";
  }
}

// --- Unstake ---
async function unstakeTokens(mintStr) {
  if (!wallet) { alert("Connect wallet first!"); return; }
  const mint = new solanaWeb3.PublicKey(mintStr);
  const statusEl = document.getElementById("tx-status");
  statusEl.textContent = "Unstaking tokens...";
  statusEl.className = "tx-status pending";

  try {
    const discBytes = await disc("unstake");
    const data = new Uint8Array(8);
    data.set(discBytes, 0);

    const [stakeAccount] = findPDA([STAKE_SEED, wallet.publicKey.toBuffer(), mint.toBuffer()], PROGRAM_ID);
    const [tokenLaunch] = findPDA([TOKEN_LAUNCH_SEED, mint.toBuffer()], PROGRAM_ID);
    const stakerAta = getATA(mint, wallet.publicKey);
    const launchVault = getATA(mint, tokenLaunch);

    const tx = new solanaWeb3.Transaction().add(new solanaWeb3.TransactionInstruction({
      keys: [
        { pubkey: stakeAccount, isSigner: false, isWritable: true },
        { pubkey: tokenLaunch, isSigner: false, isWritable: true },
        { pubkey: mint, isSigner: false, isWritable: false },
        { pubkey: stakerAta, isSigner: false, isWritable: true },
        { pubkey: launchVault, isSigner: false, isWritable: true },
        { pubkey: wallet.publicKey, isSigner: true, isWritable: true },
        { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
        { pubkey: SYSTEM_PROGRAM_ID, isSigner: false, isWritable: false },
      ],
      programId: PROGRAM_ID,
      data: Buffer.from(data),
    }));

    tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
    tx.feePayer = wallet.publicKey;
    const signed = await wallet.signTransaction(tx);
    const sig = await connection.sendRawTransaction(signed.serialize());
    await connection.confirmTransaction(sig, "confirmed");

    statusEl.textContent = `✅ Unstaked! Tx: ${sig.slice(0, 20)}...`;
    statusEl.className = "tx-status success";
    statusEl.innerHTML += ` <a href="https://solscan.io/tx/${sig}?cluster=devnet" target="_blank">View on Solscan</a>`;
    loadUserTokens();
  } catch (e) {
    console.error("Unstake error:", e);
    statusEl.textContent = `❌ Unstake failed: ${e.message}`;
    statusEl.className = "tx-status error";
  }
}

document.getElementById("connect-btn")?.addEventListener("click", connectWallet);
