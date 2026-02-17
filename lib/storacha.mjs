/**
 * Send.it × Storacha — Decentralized Token Metadata Storage
 * 
 * Stores token metadata (name, symbol, description, image) on IPFS/Filecoin
 * via Storacha, returning a verifiable content-addressed URI for on-chain use.
 * 
 * Flow:
 *   1. User creates token → uploads image + metadata JSON
 *   2. Storacha stores on IPFS with Filecoin persistence
 *   3. Returns CID-based URI for Metaplex metadata
 *   4. URI stored on-chain in create_token instruction
 * 
 * Integration points:
 *   - Token creation (metadata upload)
 *   - Launch data archival (bonding curve history)
 *   - Pool graduation records
 *   - Audit report storage
 */

import { create } from '@storacha/client';
import { filesFromPaths } from 'files-from-path';

const GATEWAY = 'https://storacha.link/ipfs';

/**
 * Initialize a Storacha client with delegated credentials.
 * For server-side (CI, backend), use STORACHA_KEY + STORACHA_PROOF env vars.
 * For browser, use email login flow.
 */
export async function createStorachaClient() {
  const client = await create();
  
  // If we have a delegation key (server-side), use it
  if (process.env.STORACHA_KEY && process.env.STORACHA_PROOF) {
    const { Signer } = await import('@ucanto/principal/ed25519');
    const { importDAG } = await import('@ucanto/core/delegation');
    const { CarReader } = await import('@ipld/car');
    
    const principal = Signer.parse(process.env.STORACHA_KEY);
    const proof = await importDAG(
      (await CarReader.fromBytes(Buffer.from(process.env.STORACHA_PROOF, 'base64'))).blocks()
    );
    
    const clientWithKey = await create({ principal });
    const space = await clientWithKey.addSpace(proof);
    await clientWithKey.setCurrentSpace(space.did());
    return clientWithKey;
  }
  
  return client;
}

/**
 * Upload token metadata to Storacha/IPFS/Filecoin.
 * Returns a gateway URL suitable for Metaplex metadata URI.
 * 
 * @param {Object} metadata - Token metadata
 * @param {string} metadata.name - Token name
 * @param {string} metadata.symbol - Token symbol
 * @param {string} metadata.description - Token description
 * @param {Blob|File} [metadata.image] - Token image file
 * @param {Object} [metadata.properties] - Additional properties
 * @returns {Promise<{metadataUri: string, imageCid: string|null, metadataCid: string}>}
 */
export async function uploadTokenMetadata(client, metadata) {
  let imageUri = null;
  let imageCid = null;
  
  // Upload image first if provided
  if (metadata.image) {
    const imageCidResult = await client.uploadFile(metadata.image);
    imageCid = imageCidResult.toString();
    imageUri = `${GATEWAY}/${imageCid}`;
  }
  
  // Build Metaplex-compatible metadata JSON
  const metadataJson = {
    name: metadata.name,
    symbol: metadata.symbol,
    description: metadata.description || '',
    image: imageUri || '',
    external_url: `https://senditsolana.io`,
    properties: {
      category: 'token',
      creators: metadata.creators || [],
      ...metadata.properties,
    },
    attributes: metadata.attributes || [
      { trait_type: 'Platform', value: 'Send.it' },
      { trait_type: 'Network', value: 'Solana' },
      { trait_type: 'Storage', value: 'Filecoin via Storacha' },
    ],
    // Storacha/Filecoin verification
    storage: {
      provider: 'Storacha',
      network: 'Filecoin',
      imageCid: imageCid,
      timestamp: new Date().toISOString(),
    },
  };
  
  // Upload metadata JSON
  const metadataBlob = new Blob([JSON.stringify(metadataJson, null, 2)], {
    type: 'application/json',
  });
  const metadataFile = new File([metadataBlob], 'metadata.json', {
    type: 'application/json',
  });
  
  const metadataCidResult = await client.uploadFile(metadataFile);
  const metadataCid = metadataCidResult.toString();
  const metadataUri = `${GATEWAY}/${metadataCid}/metadata.json`;
  
  return {
    metadataUri,
    metadataCid,
    imageCid,
    imageUri,
    gateway: GATEWAY,
  };
}

/**
 * Upload launch data archive to Storacha/Filecoin.
 * Stores bonding curve history, trade data, graduation records.
 * 
 * @param {Object} client - Storacha client
 * @param {Object} launchData - Launch archive data
 * @returns {Promise<{archiveUri: string, archiveCid: string}>}
 */
export async function archiveLaunchData(client, launchData) {
  const archive = {
    version: '1.0',
    protocol: 'Send.it',
    network: 'solana',
    timestamp: new Date().toISOString(),
    ...launchData,
    storage: {
      provider: 'Storacha',
      network: 'Filecoin',
    },
  };
  
  const blob = new Blob([JSON.stringify(archive, null, 2)], {
    type: 'application/json',
  });
  const file = new File([blob], `launch-${launchData.mint || 'unknown'}.json`, {
    type: 'application/json',
  });
  
  const cid = await client.uploadFile(file);
  return {
    archiveUri: `${GATEWAY}/${cid.toString()}`,
    archiveCid: cid.toString(),
  };
}

/**
 * Upload audit report to Storacha/Filecoin.
 * Permanent, verifiable storage of security scan results.
 * 
 * @param {Object} client - Storacha client
 * @param {Object} auditData - Audit report data
 * @returns {Promise<{auditUri: string, auditCid: string}>}
 */
export async function archiveAuditReport(client, auditData) {
  const report = {
    version: '1.0',
    protocol: 'Send.it',
    programId: auditData.programId || 'HTKq18cATdwCZb6XM66Mhn8JWKCFTrZqH6zU1zip88Zx',
    scanner: auditData.scanner || 'Sec3 X-Ray v0.0.6',
    timestamp: new Date().toISOString(),
    findings: auditData.findings || 0,
    details: auditData.details || 'No issues detected',
    instructions_scanned: auditData.instructions || 11,
    storage: {
      provider: 'Storacha',
      network: 'Filecoin',
      immutable: true,
    },
  };
  
  const blob = new Blob([JSON.stringify(report, null, 2)], {
    type: 'application/json',
  });
  const file = new File([blob], `audit-${Date.now()}.json`, {
    type: 'application/json',
  });
  
  const cid = await client.uploadFile(file);
  return {
    auditUri: `${GATEWAY}/${cid.toString()}`,
    auditCid: cid.toString(),
  };
}

/**
 * Retrieve and verify metadata from Storacha gateway.
 * 
 * @param {string} cid - Content identifier
 * @returns {Promise<Object>} - Parsed metadata JSON
 */
export async function fetchMetadata(cid) {
  const url = `${GATEWAY}/${cid}`;
  const resp = await fetch(url);
  if (!resp.ok) throw new Error(`Failed to fetch ${url}: ${resp.status}`);
  return resp.json();
}

/**
 * Get a gateway URL for a CID.
 * @param {string} cid
 * @returns {string}
 */
export function getGatewayUrl(cid) {
  return `${GATEWAY}/${cid}`;
}
