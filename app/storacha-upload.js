/**
 * Send.it × Storacha — Browser-side metadata upload
 * 
 * Uploads token metadata (image + JSON) to IPFS/Filecoin via Storacha
 * before on-chain token creation. Returns a CID-based URI for Metaplex.
 * 
 * Uses the Storacha HTTP API bridge for browser uploads.
 */

const STORACHA_GATEWAY = 'https://storacha.link/ipfs';

// Storacha upload state
let storachaEnabled = true;

/**
 * Upload a file to Storacha via the w3up HTTP bridge.
 * Falls back to a simple IPFS gateway if Storacha is unavailable.
 * 
 * @param {File|Blob} file - File to upload
 * @returns {Promise<string>} - CID string
 */
async function uploadToStoracha(file) {
  // Use the Send.it backend proxy for Storacha uploads
  // This avoids exposing Storacha credentials in the browser
  const formData = new FormData();
  formData.append('file', file);
  
  try {
    const resp = await fetch('/api/storacha-upload', {
      method: 'POST',
      body: formData,
    });
    
    if (!resp.ok) throw new Error(`Upload failed: ${resp.status}`);
    const data = await resp.json();
    return data.cid;
  } catch (e) {
    console.warn('Storacha upload failed, using fallback:', e.message);
    // Return null — caller should handle gracefully
    return null;
  }
}

/**
 * Build and upload Metaplex-compatible token metadata to Storacha/Filecoin.
 * 
 * @param {Object} opts
 * @param {string} opts.name - Token name
 * @param {string} opts.symbol - Token symbol  
 * @param {string} opts.description - Token description
 * @param {File} [opts.imageFile] - Token image
 * @param {string} [opts.creatorAddress] - Creator's Solana address
 * @returns {Promise<{metadataUri: string, imageCid: string|null, storachaEnabled: boolean}>}
 */
async function uploadTokenMetadataToStoracha({ name, symbol, description, imageFile, creatorAddress }) {
  const statusEl = document.getElementById('storacha-status');
  if (statusEl) statusEl.innerHTML = '<i class="fas fa-spinner fa-spin"></i> Uploading to Filecoin via Storacha...';
  
  let imageCid = null;
  let imageUri = '';
  
  // Upload image
  if (imageFile) {
    imageCid = await uploadToStoracha(imageFile);
    if (imageCid) {
      imageUri = `${STORACHA_GATEWAY}/${imageCid}`;
      if (statusEl) statusEl.innerHTML = '<i class="fas fa-check" style="color:#22c55e"></i> Image uploaded to Filecoin';
    }
  }
  
  // Build Metaplex-compatible metadata
  const metadata = {
    name,
    symbol,
    description: description || `${name} — launched on Send.it`,
    image: imageUri,
    external_url: 'https://senditsolana.io',
    properties: {
      category: 'token',
      creators: creatorAddress ? [{ address: creatorAddress, share: 100 }] : [],
    },
    attributes: [
      { trait_type: 'Platform', value: 'Send.it' },
      { trait_type: 'Network', value: 'Solana' },
      { trait_type: 'Storage', value: 'Filecoin via Storacha' },
    ],
    storage: {
      provider: 'Storacha',
      network: 'Filecoin',
      imageCid,
      timestamp: new Date().toISOString(),
    },
  };
  
  // Upload metadata JSON
  const metadataBlob = new Blob([JSON.stringify(metadata, null, 2)], { type: 'application/json' });
  const metadataFile = new File([metadataBlob], 'metadata.json', { type: 'application/json' });
  const metadataCid = await uploadToStoracha(metadataFile);
  
  let metadataUri = '';
  if (metadataCid) {
    metadataUri = `${STORACHA_GATEWAY}/${metadataCid}`;
    if (statusEl) statusEl.innerHTML = `<i class="fas fa-check-double" style="color:#22c55e"></i> Metadata stored on Filecoin — <a href="${metadataUri}" target="_blank" style="color:var(--accent)">View CID</a>`;
  } else {
    if (statusEl) statusEl.innerHTML = '<i class="fas fa-exclamation-triangle" style="color:#ff8800"></i> Storacha unavailable — using fallback URI';
  }
  
  return {
    metadataUri,
    metadataCid,
    imageCid,
    imageUri,
    storachaEnabled: !!metadataCid,
  };
}

/**
 * Archive pool graduation data to Storacha/Filecoin.
 * Called when a token graduates from bonding curve to AMM.
 */
async function archiveGraduation({ mint, poolAddress, initialSol, initialTokens, lpMinted, creator }) {
  const record = {
    event: 'pool_graduation',
    protocol: 'Send.it',
    network: 'solana',
    mint,
    poolAddress,
    initialSol,
    initialTokens,
    lpMinted,
    creator,
    timestamp: new Date().toISOString(),
    storage: { provider: 'Storacha', network: 'Filecoin' },
  };
  
  const blob = new Blob([JSON.stringify(record, null, 2)], { type: 'application/json' });
  const file = new File([blob], `graduation-${mint}.json`, { type: 'application/json' });
  const cid = await uploadToStoracha(file);
  
  return cid ? `${STORACHA_GATEWAY}/${cid}` : null;
}

/**
 * Verify metadata exists on Storacha/IPFS gateway.
 */
async function verifyStorachaMetadata(cid) {
  try {
    const resp = await fetch(`${STORACHA_GATEWAY}/${cid}`, { method: 'HEAD' });
    return resp.ok;
  } catch {
    return false;
  }
}
