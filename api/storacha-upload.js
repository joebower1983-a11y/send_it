/**
 * Send.it × Storacha — Upload API endpoint (Vercel serverless)
 * 
 * POST /api/storacha-upload
 * Accepts multipart form data with a single 'file' field.
 * Returns { cid: string, url: string }
 * 
 * Env vars required:
 *   STORACHA_KEY   — Ed25519 private key (base64)
 *   STORACHA_PROOF — Delegation proof CAR (base64)
 */

export const config = { api: { bodyParser: false } };

import { create } from '@storacha/client';
import { Signer } from '@ucanto/principal/ed25519';
import { importDAG } from '@ucanto/core/delegation';
import { CarReader } from '@ipld/car';
import { Readable } from 'stream';

const GATEWAY = 'https://storacha.link/ipfs';
const MAX_SIZE = 5 * 1024 * 1024; // 5MB

// Parse multipart form data manually (no external dep)
async function parseBody(req) {
  return new Promise((resolve, reject) => {
    const chunks = [];
    let size = 0;
    req.on('data', (chunk) => {
      size += chunk.length;
      if (size > MAX_SIZE + 4096) {
        reject(new Error('File too large'));
        req.destroy();
        return;
      }
      chunks.push(chunk);
    });
    req.on('end', () => resolve(Buffer.concat(chunks)));
    req.on('error', reject);
  });
}

function extractFileFromMultipart(buffer, contentType) {
  const boundary = contentType.split('boundary=')[1];
  if (!boundary) throw new Error('No boundary in content-type');

  const boundaryBuf = Buffer.from(`--${boundary}`);
  const parts = [];
  let start = buffer.indexOf(boundaryBuf) + boundaryBuf.length;

  while (true) {
    const nextBoundary = buffer.indexOf(boundaryBuf, start);
    if (nextBoundary === -1) break;
    parts.push(buffer.slice(start, nextBoundary));
    start = nextBoundary + boundaryBuf.length;
  }

  for (const part of parts) {
    const headerEnd = part.indexOf('\r\n\r\n');
    if (headerEnd === -1) continue;
    const headers = part.slice(0, headerEnd).toString();
    if (headers.includes('name="file"')) {
      const body = part.slice(headerEnd + 4);
      // Trim trailing \r\n
      const trimmed = body.slice(0, body.lastIndexOf('\r\n'));
      
      // Extract filename and content-type from headers
      const filenameMatch = headers.match(/filename="([^"]+)"/);
      const ctMatch = headers.match(/Content-Type:\s*(\S+)/i);
      return {
        data: trimmed.length > 0 ? trimmed : body,
        filename: filenameMatch?.[1] || 'upload',
        contentType: ctMatch?.[1] || 'application/octet-stream',
      };
    }
  }
  throw new Error('No file field found');
}

let cachedClient = null;

async function getClient() {
  if (cachedClient) return cachedClient;

  const key = process.env.STORACHA_KEY;
  const proof = process.env.STORACHA_PROOF;

  if (!key || !proof) {
    throw new Error('STORACHA_KEY and STORACHA_PROOF env vars required');
  }

  const principal = Signer.parse(key);
  const client = await create({ principal });

  const blocks = [];
  const reader = await CarReader.fromBytes(Buffer.from(proof, 'base64'));
  for await (const block of reader.blocks()) {
    blocks.push(block);
  }
  const delegation = importDAG(blocks);

  const space = await client.addSpace(delegation);
  await client.setCurrentSpace(space.did());

  cachedClient = client;
  return client;
}

export default async function handler(req, res) {
  // CORS
  res.setHeader('Access-Control-Allow-Origin', '*');
  res.setHeader('Access-Control-Allow-Methods', 'POST, OPTIONS');
  res.setHeader('Access-Control-Allow-Headers', 'Content-Type');

  if (req.method === 'OPTIONS') return res.status(200).end();
  if (req.method !== 'POST') return res.status(405).json({ error: 'POST only' });

  try {
    const body = await parseBody(req);
    const file = extractFileFromMultipart(body, req.headers['content-type']);

    if (file.data.length > MAX_SIZE) {
      return res.status(413).json({ error: 'File too large (max 5MB)' });
    }

    const client = await getClient();

    // Create a File-like object for the upload
    const blob = new Blob([file.data], { type: file.contentType });
    const uploadFile = new File([blob], file.filename, { type: file.contentType });

    const cid = await client.uploadFile(uploadFile);
    const cidStr = cid.toString();

    return res.status(200).json({
      cid: cidStr,
      url: `${GATEWAY}/${cidStr}`,
      size: file.data.length,
      filename: file.filename,
    });
  } catch (e) {
    console.error('Storacha upload error:', e);
    return res.status(500).json({ error: e.message });
  }
}
