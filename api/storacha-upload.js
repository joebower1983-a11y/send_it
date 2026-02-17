/**
 * Send.it × Storacha — Upload API endpoint
 * Vercel serverless function that proxies uploads to Storacha/Filecoin
 * 
 * POST /api/storacha-upload
 * Body: multipart/form-data with 'file' field
 * Returns: { cid, gateway, url }
 */

export const config = {
  api: {
    bodyParser: false, // handle multipart ourselves
  },
};

export default async function handler(req, res) {
  // CORS
  res.setHeader('Access-Control-Allow-Origin', '*');
  res.setHeader('Access-Control-Allow-Methods', 'POST, OPTIONS');
  res.setHeader('Access-Control-Allow-Headers', 'Content-Type');
  
  if (req.method === 'OPTIONS') return res.status(200).end();
  if (req.method !== 'POST') return res.status(405).json({ error: 'Method not allowed' });
  
  try {
    // Parse multipart form data
    const chunks = [];
    for await (const chunk of req) chunks.push(chunk);
    const body = Buffer.concat(chunks);
    
    // Extract boundary from content-type
    const contentType = req.headers['content-type'] || '';
    const boundaryMatch = contentType.match(/boundary=(.+)/);
    if (!boundaryMatch) return res.status(400).json({ error: 'Missing boundary' });
    
    const boundary = boundaryMatch[1];
    const parts = parseMultipart(body, boundary);
    const filePart = parts.find(p => p.name === 'file');
    
    if (!filePart) return res.status(400).json({ error: 'No file provided' });
    
    // Upload to Storacha
    const { create } = await import('@storacha/client');
    const client = await create();
    
    // Use delegated credentials from env
    if (process.env.STORACHA_KEY && process.env.STORACHA_PROOF) {
      const { Signer } = await import('@ucanto/principal/ed25519');
      const { importDAG } = await import('@ucanto/core/delegation');
      const { CarReader } = await import('@ipld/car');
      
      const principal = Signer.parse(process.env.STORACHA_KEY);
      const proof = await importDAG(
        (await CarReader.fromBytes(Buffer.from(process.env.STORACHA_PROOF, 'base64'))).blocks()
      );
      
      const space = await client.addSpace(proof);
      await client.setCurrentSpace(space.did());
    }
    
    const blob = new Blob([filePart.data], { type: filePart.contentType || 'application/octet-stream' });
    const file = new File([blob], filePart.filename || 'upload', { type: filePart.contentType });
    
    const cid = await client.uploadFile(file);
    const cidStr = cid.toString();
    
    return res.status(200).json({
      cid: cidStr,
      gateway: 'https://storacha.link/ipfs',
      url: `https://storacha.link/ipfs/${cidStr}`,
    });
  } catch (e) {
    console.error('Storacha upload error:', e);
    return res.status(500).json({ error: e.message });
  }
}

/**
 * Simple multipart parser
 */
function parseMultipart(body, boundary) {
  const parts = [];
  const boundaryBuf = Buffer.from(`--${boundary}`);
  const endBuf = Buffer.from(`--${boundary}--`);
  
  let start = body.indexOf(boundaryBuf) + boundaryBuf.length + 2; // skip \r\n
  
  while (start < body.length) {
    const nextBoundary = body.indexOf(boundaryBuf, start);
    if (nextBoundary === -1) break;
    
    const partData = body.subarray(start, nextBoundary - 2); // trim \r\n before boundary
    const headerEnd = partData.indexOf('\r\n\r\n');
    if (headerEnd === -1) break;
    
    const headers = partData.subarray(0, headerEnd).toString();
    const data = partData.subarray(headerEnd + 4);
    
    const nameMatch = headers.match(/name="([^"]+)"/);
    const filenameMatch = headers.match(/filename="([^"]+)"/);
    const ctMatch = headers.match(/Content-Type:\s*(.+)/i);
    
    parts.push({
      name: nameMatch?.[1],
      filename: filenameMatch?.[1],
      contentType: ctMatch?.[1]?.trim(),
      data,
    });
    
    start = nextBoundary + boundaryBuf.length + 2;
  }
  
  return parts;
}
