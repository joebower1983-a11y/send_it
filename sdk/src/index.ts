import {
  Connection,
  PublicKey,
  TransactionInstruction,
  Transaction,
  SystemProgram,
  Keypair,
  SYSVAR_RENT_PUBKEY,
  SendOptions,
  Signer,
} from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  getAssociatedTokenAddressSync,
} from "@solana/spl-token";
import BN from "bn.js";

// ── Constants ────────────────────────────────────────────────────────────────

const PROGRAM_ID = new PublicKey(
  "SenditXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX" // placeholder — replace with deployed program id
);

const SEEDS = {
  PLATFORM: Buffer.from("platform"),
  LAUNCH: Buffer.from("launch"),
  POSITION: Buffer.from("position"),
  LEADERBOARD: Buffer.from("leaderboard"),
} as const;

// ── Types ────────────────────────────────────────────────────────────────────

export enum CurveType {
  Linear = 0,
  Exponential = 1,
  Logarithmic = 2,
}

export interface AntiSnipeConfig {
  /** Max buy in first N slots (lamports) */
  maxBuyFirstSlots: BN;
  /** Number of slots the snipe guard is active */
  protectedSlots: number;
}

export interface PlatformConfig {
  authority: PublicKey;
  feeBps: number;
  migrationThreshold: BN;
  paused: boolean;
  bump: number;
}

export interface TokenLaunch {
  mint: PublicKey;
  creator: PublicKey;
  name: string;
  symbol: string;
  uri: string;
  curveType: CurveType;
  creatorFeeBps: number;
  antiSnipeConfig: AntiSnipeConfig | null;
  totalSupply: BN;
  virtualSolReserves: BN;
  virtualTokenReserves: BN;
  realSolReserves: BN;
  realTokenReserves: BN;
  accumulatedCreatorFees: BN;
  migrated: boolean;
  paused: boolean;
  createdAt: BN;
  bump: number;
}

export interface UserPosition {
  wallet: PublicKey;
  mint: PublicKey;
  tokenBalance: BN;
  totalSolSpent: BN;
  totalSolReceived: BN;
  bump: number;
}

export interface LeaderboardEntry {
  mint: PublicKey;
  volume: BN;
}

export interface Leaderboard {
  entries: LeaderboardEntry[];
  bump: number;
}

export interface SendItClientConfig {
  connection: Connection;
  programId?: PublicKey;
  wallet?: Keypair;
}

// ── Instruction discriminators (Anchor 8-byte sha256 hashes) ─────────────

function ixDiscriminator(name: string): Buffer {
  const crypto = require("crypto");
  const hash = crypto
    .createHash("sha256")
    .update(`global:${name}`)
    .digest() as Buffer;
  return hash.slice(0, 8);
}

// ── Borsh-lite encode helpers ────────────────────────────────────────────────

function encodeU16(val: number): Buffer {
  const buf = Buffer.alloc(2);
  buf.writeUInt16LE(val);
  return buf;
}

function encodeU32(val: number): Buffer {
  const buf = Buffer.alloc(4);
  buf.writeUInt32LE(val);
  return buf;
}

function encodeU64(val: BN): Buffer {
  return val.toArrayLike(Buffer, "le", 8);
}

function encodeU8(val: number): Buffer {
  return Buffer.from([val]);
}

function encodeBool(val: boolean): Buffer {
  return Buffer.from([val ? 1 : 0]);
}

function encodeString(val: string): Buffer {
  const strBuf = Buffer.from(val, "utf-8");
  const lenBuf = Buffer.alloc(4);
  lenBuf.writeUInt32LE(strBuf.length);
  return Buffer.concat([lenBuf, strBuf]);
}

function encodeOption<T>(
  val: T | null,
  encoder: (v: T) => Buffer
): Buffer {
  if (val === null || val === undefined) return Buffer.from([0]);
  return Buffer.concat([Buffer.from([1]), encoder(val)]);
}

function encodeAntiSnipeConfig(cfg: AntiSnipeConfig): Buffer {
  return Buffer.concat([
    encodeU64(cfg.maxBuyFirstSlots),
    encodeU32(cfg.protectedSlots),
  ]);
}

// ── PDA derivation ───────────────────────────────────────────────────────────

export function derivePlatformConfig(
  programId: PublicKey = PROGRAM_ID
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync([SEEDS.PLATFORM], programId);
}

export function deriveTokenLaunch(
  mint: PublicKey,
  programId: PublicKey = PROGRAM_ID
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [SEEDS.LAUNCH, mint.toBuffer()],
    programId
  );
}

export function deriveUserPosition(
  wallet: PublicKey,
  mint: PublicKey,
  programId: PublicKey = PROGRAM_ID
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [SEEDS.POSITION, wallet.toBuffer(), mint.toBuffer()],
    programId
  );
}

export function deriveLeaderboard(
  programId: PublicKey = PROGRAM_ID
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync([SEEDS.LEADERBOARD], programId);
}

// ── Client ───────────────────────────────────────────────────────────────────

export class SendItClient {
  readonly connection: Connection;
  readonly programId: PublicKey;
  private wallet: Keypair | null;

  constructor(config: SendItClientConfig) {
    this.connection = config.connection;
    this.programId = config.programId ?? PROGRAM_ID;
    this.wallet = config.wallet ?? null;
  }

  // ── helpers ──

  private signer(): Keypair {
    if (!this.wallet) throw new Error("Wallet not set on SendItClient");
    return this.wallet;
  }

  private async sendTx(
    ix: TransactionInstruction,
    signers: Signer[],
    opts?: SendOptions
  ): Promise<string> {
    const tx = new Transaction().add(ix);
    tx.feePayer = signers[0].publicKey;
    tx.recentBlockhash = (
      await this.connection.getLatestBlockhash()
    ).blockhash;
    tx.sign(...signers);
    return this.connection.sendRawTransaction(tx.serialize(), opts);
  }

  // ── PDA shortcuts ──

  get platformConfigPda(): PublicKey {
    return derivePlatformConfig(this.programId)[0];
  }

  tokenLaunchPda(mint: PublicKey): PublicKey {
    return deriveTokenLaunch(mint, this.programId)[0];
  }

  userPositionPda(wallet: PublicKey, mint: PublicKey): PublicKey {
    return deriveUserPosition(wallet, mint, this.programId)[0];
  }

  get leaderboardPda(): PublicKey {
    return deriveLeaderboard(this.programId)[0];
  }

  // ══════════════════════════════════════════════════════════════════════════
  //  INSTRUCTIONS
  // ══════════════════════════════════════════════════════════════════════════

  /**
   * Initialize the platform. Called once by the deployer.
   */
  async initializePlatform(params: {
    feeBps: number;
    migrationThreshold: BN;
    authority: PublicKey;
  }): Promise<string> {
    const signer = this.signer();
    const data = Buffer.concat([
      ixDiscriminator("initialize_platform"),
      encodeU16(params.feeBps),
      encodeU64(params.migrationThreshold),
      params.authority.toBuffer(),
    ]);

    const ix = new TransactionInstruction({
      programId: this.programId,
      keys: [
        { pubkey: signer.publicKey, isSigner: true, isWritable: true },
        { pubkey: this.platformConfigPda, isSigner: false, isWritable: true },
        { pubkey: this.leaderboardPda, isSigner: false, isWritable: true },
        { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      ],
      data,
    });
    return this.sendTx(ix, [signer]);
  }

  /**
   * Create a new token launch with bonding curve.
   */
  async createToken(params: {
    mint: Keypair;
    name: string;
    symbol: string;
    uri: string;
    curveType: CurveType;
    creatorFeeBps: number;
    antiSnipeConfig?: AntiSnipeConfig | null;
  }): Promise<string> {
    const signer = this.signer();
    const launchPda = this.tokenLaunchPda(params.mint.publicKey);

    const data = Buffer.concat([
      ixDiscriminator("create_token"),
      encodeString(params.name),
      encodeString(params.symbol),
      encodeString(params.uri),
      encodeU8(params.curveType),
      encodeU16(params.creatorFeeBps),
      encodeOption(params.antiSnipeConfig ?? null, encodeAntiSnipeConfig),
    ]);

    const ix = new TransactionInstruction({
      programId: this.programId,
      keys: [
        { pubkey: signer.publicKey, isSigner: true, isWritable: true },
        { pubkey: params.mint.publicKey, isSigner: true, isWritable: true },
        { pubkey: launchPda, isSigner: false, isWritable: true },
        { pubkey: this.platformConfigPda, isSigner: false, isWritable: false },
        { pubkey: this.leaderboardPda, isSigner: false, isWritable: true },
        { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
        { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
        { pubkey: SYSVAR_RENT_PUBKEY, isSigner: false, isWritable: false },
      ],
      data,
    });
    return this.sendTx(ix, [signer, params.mint]);
  }

  /**
   * Buy tokens on the bonding curve.
   */
  async buy(params: {
    mint: PublicKey;
    amount: BN;
    maxSolCost: BN;
  }): Promise<string> {
    const signer = this.signer();
    const launchPda = this.tokenLaunchPda(params.mint);
    const positionPda = this.userPositionPda(signer.publicKey, params.mint);
    const buyerAta = getAssociatedTokenAddressSync(
      params.mint,
      signer.publicKey
    );

    const data = Buffer.concat([
      ixDiscriminator("buy"),
      encodeU64(params.amount),
      encodeU64(params.maxSolCost),
    ]);

    const ix = new TransactionInstruction({
      programId: this.programId,
      keys: [
        { pubkey: signer.publicKey, isSigner: true, isWritable: true },
        { pubkey: launchPda, isSigner: false, isWritable: true },
        { pubkey: params.mint, isSigner: false, isWritable: false },
        { pubkey: buyerAta, isSigner: false, isWritable: true },
        { pubkey: positionPda, isSigner: false, isWritable: true },
        { pubkey: this.platformConfigPda, isSigner: false, isWritable: true },
        { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
        {
          pubkey: ASSOCIATED_TOKEN_PROGRAM_ID,
          isSigner: false,
          isWritable: false,
        },
        { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      ],
      data,
    });
    return this.sendTx(ix, [signer]);
  }

  /**
   * Sell tokens back to the bonding curve.
   */
  async sell(params: {
    mint: PublicKey;
    amount: BN;
    minSolReturn: BN;
  }): Promise<string> {
    const signer = this.signer();
    const launchPda = this.tokenLaunchPda(params.mint);
    const positionPda = this.userPositionPda(signer.publicKey, params.mint);
    const sellerAta = getAssociatedTokenAddressSync(
      params.mint,
      signer.publicKey
    );

    const data = Buffer.concat([
      ixDiscriminator("sell"),
      encodeU64(params.amount),
      encodeU64(params.minSolReturn),
    ]);

    const ix = new TransactionInstruction({
      programId: this.programId,
      keys: [
        { pubkey: signer.publicKey, isSigner: true, isWritable: true },
        { pubkey: launchPda, isSigner: false, isWritable: true },
        { pubkey: params.mint, isSigner: false, isWritable: false },
        { pubkey: sellerAta, isSigner: false, isWritable: true },
        { pubkey: positionPda, isSigner: false, isWritable: true },
        { pubkey: this.platformConfigPda, isSigner: false, isWritable: true },
        { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
        { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      ],
      data,
    });
    return this.sendTx(ix, [signer]);
  }

  /**
   * Migrate token liquidity to Raydium when threshold is hit.
   */
  async migrateToRaydium(params: {
    mint: PublicKey;
    raydiumPoolAccounts: {
      ammId: PublicKey;
      ammAuthority: PublicKey;
      ammOpenOrders: PublicKey;
      lpMint: PublicKey;
      coinVault: PublicKey;
      pcVault: PublicKey;
      raydiumProgram: PublicKey;
    };
  }): Promise<string> {
    const signer = this.signer();
    const launchPda = this.tokenLaunchPda(params.mint);
    const r = params.raydiumPoolAccounts;

    const data = ixDiscriminator("migrate_to_raydium");

    const ix = new TransactionInstruction({
      programId: this.programId,
      keys: [
        { pubkey: signer.publicKey, isSigner: true, isWritable: true },
        { pubkey: launchPda, isSigner: false, isWritable: true },
        { pubkey: params.mint, isSigner: false, isWritable: true },
        { pubkey: this.platformConfigPda, isSigner: false, isWritable: false },
        { pubkey: r.ammId, isSigner: false, isWritable: true },
        { pubkey: r.ammAuthority, isSigner: false, isWritable: false },
        { pubkey: r.ammOpenOrders, isSigner: false, isWritable: true },
        { pubkey: r.lpMint, isSigner: false, isWritable: true },
        { pubkey: r.coinVault, isSigner: false, isWritable: true },
        { pubkey: r.pcVault, isSigner: false, isWritable: true },
        { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
        { pubkey: r.raydiumProgram, isSigner: false, isWritable: false },
        { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      ],
      data,
    });
    return this.sendTx(ix, [signer]);
  }

  /**
   * Creator claims accumulated fees from their token launch.
   */
  async claimCreatorFees(params: { mint: PublicKey }): Promise<string> {
    const signer = this.signer();
    const launchPda = this.tokenLaunchPda(params.mint);

    const data = ixDiscriminator("claim_creator_fees");

    const ix = new TransactionInstruction({
      programId: this.programId,
      keys: [
        { pubkey: signer.publicKey, isSigner: true, isWritable: true },
        { pubkey: launchPda, isSigner: false, isWritable: true },
        { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      ],
      data,
    });
    return this.sendTx(ix, [signer]);
  }

  /**
   * Authority updates platform configuration.
   */
  async updatePlatformConfig(params: {
    feeBps?: number;
    migrationThreshold?: BN;
    authority?: PublicKey;
  }): Promise<string> {
    const signer = this.signer();

    const data = Buffer.concat([
      ixDiscriminator("update_platform_config"),
      encodeOption(params.feeBps ?? null, (v) => encodeU16(v)),
      encodeOption(params.migrationThreshold ?? null, (v) => encodeU64(v)),
      encodeOption(params.authority ?? null, (v) => v.toBuffer()),
    ]);

    const ix = new TransactionInstruction({
      programId: this.programId,
      keys: [
        { pubkey: signer.publicKey, isSigner: true, isWritable: false },
        { pubkey: this.platformConfigPda, isSigner: false, isWritable: true },
      ],
      data,
    });
    return this.sendTx(ix, [signer]);
  }

  /**
   * Emergency pause a token launch.
   */
  async pauseToken(params: { mint: PublicKey }): Promise<string> {
    const signer = this.signer();
    const launchPda = this.tokenLaunchPda(params.mint);

    const data = ixDiscriminator("pause_token");

    const ix = new TransactionInstruction({
      programId: this.programId,
      keys: [
        { pubkey: signer.publicKey, isSigner: true, isWritable: false },
        { pubkey: launchPda, isSigner: false, isWritable: true },
        { pubkey: this.platformConfigPda, isSigner: false, isWritable: false },
      ],
      data,
    });
    return this.sendTx(ix, [signer]);
  }

  /**
   * Unpause a token launch.
   */
  async unpauseToken(params: { mint: PublicKey }): Promise<string> {
    const signer = this.signer();
    const launchPda = this.tokenLaunchPda(params.mint);

    const data = ixDiscriminator("unpause_token");

    const ix = new TransactionInstruction({
      programId: this.programId,
      keys: [
        { pubkey: signer.publicKey, isSigner: true, isWritable: false },
        { pubkey: launchPda, isSigner: false, isWritable: true },
        { pubkey: this.platformConfigPda, isSigner: false, isWritable: false },
      ],
      data,
    });
    return this.sendTx(ix, [signer]);
  }

  // ══════════════════════════════════════════════════════════════════════════
  //  READ / HELPER METHODS
  // ══════════════════════════════════════════════════════════════════════════

  /**
   * Fetch and deserialize a TokenLaunch account.
   */
  async getTokenLaunch(mint: PublicKey): Promise<TokenLaunch | null> {
    const pda = this.tokenLaunchPda(mint);
    const info = await this.connection.getAccountInfo(pda);
    if (!info) return null;
    return this.deserializeTokenLaunch(info.data);
  }

  /**
   * Fetch platform config.
   */
  async getPlatformConfig(): Promise<PlatformConfig | null> {
    const info = await this.connection.getAccountInfo(this.platformConfigPda);
    if (!info) return null;
    return this.deserializePlatformConfig(info.data);
  }

  /**
   * Fetch a user's position for a specific token.
   */
  async getUserPosition(
    wallet: PublicKey,
    mint: PublicKey
  ): Promise<UserPosition | null> {
    const pda = this.userPositionPda(wallet, mint);
    const info = await this.connection.getAccountInfo(pda);
    if (!info) return null;
    return this.deserializeUserPosition(info.data);
  }

  /**
   * Fetch the leaderboard.
   */
  async getLeaderboard(): Promise<Leaderboard | null> {
    const info = await this.connection.getAccountInfo(this.leaderboardPda);
    if (!info) return null;
    return this.deserializeLeaderboard(info.data);
  }

  /**
   * Estimate SOL cost to buy `amount` tokens on a given curve state.
   * Uses the constant-product bonding curve formula: cost = virtualSol * amount / (virtualTokens - amount)
   */
  estimateBuyPrice(
    amount: BN,
    curve: { virtualSolReserves: BN; virtualTokenReserves: BN }
  ): BN {
    const numerator = curve.virtualSolReserves.mul(amount);
    const denominator = curve.virtualTokenReserves.sub(amount);
    if (denominator.lte(new BN(0)))
      throw new Error("Amount exceeds available token reserves");
    // add 1 to round up
    return numerator.div(denominator).add(new BN(1));
  }

  /**
   * Estimate SOL returned when selling `amount` tokens.
   */
  estimateSellReturn(
    amount: BN,
    curve: { virtualSolReserves: BN; virtualTokenReserves: BN }
  ): BN {
    const numerator = curve.virtualSolReserves.mul(amount);
    const denominator = curve.virtualTokenReserves.add(amount);
    return numerator.div(denominator);
  }

  // ── Deserialization (Anchor discriminator + borsh) ─────────────────────

  private deserializePlatformConfig(data: Buffer): PlatformConfig {
    let offset = 8; // skip discriminator
    const authority = new PublicKey(data.subarray(offset, offset + 32));
    offset += 32;
    const feeBps = data.readUInt16LE(offset);
    offset += 2;
    const migrationThreshold = new BN(
      data.subarray(offset, offset + 8),
      "le"
    );
    offset += 8;
    const paused = data[offset] === 1;
    offset += 1;
    const bump = data[offset];
    return { authority, feeBps, migrationThreshold, paused, bump };
  }

  private deserializeTokenLaunch(data: Buffer): TokenLaunch {
    let offset = 8; // skip discriminator

    const mint = new PublicKey(data.subarray(offset, offset + 32));
    offset += 32;
    const creator = new PublicKey(data.subarray(offset, offset + 32));
    offset += 32;

    const nameLen = data.readUInt32LE(offset);
    offset += 4;
    const name = data.subarray(offset, offset + nameLen).toString("utf-8");
    offset += nameLen;

    const symbolLen = data.readUInt32LE(offset);
    offset += 4;
    const symbol = data.subarray(offset, offset + symbolLen).toString("utf-8");
    offset += symbolLen;

    const uriLen = data.readUInt32LE(offset);
    offset += 4;
    const uri = data.subarray(offset, offset + uriLen).toString("utf-8");
    offset += uriLen;

    const curveType = data[offset] as CurveType;
    offset += 1;
    const creatorFeeBps = data.readUInt16LE(offset);
    offset += 2;

    // anti snipe config (Option)
    const hasAntiSnipe = data[offset] === 1;
    offset += 1;
    let antiSnipeConfig: AntiSnipeConfig | null = null;
    if (hasAntiSnipe) {
      const maxBuyFirstSlots = new BN(
        data.subarray(offset, offset + 8),
        "le"
      );
      offset += 8;
      const protectedSlots = data.readUInt32LE(offset);
      offset += 4;
      antiSnipeConfig = { maxBuyFirstSlots, protectedSlots };
    }

    const readBN = () => {
      const val = new BN(data.subarray(offset, offset + 8), "le");
      offset += 8;
      return val;
    };

    const totalSupply = readBN();
    const virtualSolReserves = readBN();
    const virtualTokenReserves = readBN();
    const realSolReserves = readBN();
    const realTokenReserves = readBN();
    const accumulatedCreatorFees = readBN();

    const migrated = data[offset] === 1;
    offset += 1;
    const paused = data[offset] === 1;
    offset += 1;
    const createdAt = readBN();
    const bump = data[offset];

    return {
      mint,
      creator,
      name,
      symbol,
      uri,
      curveType,
      creatorFeeBps,
      antiSnipeConfig,
      totalSupply,
      virtualSolReserves,
      virtualTokenReserves,
      realSolReserves,
      realTokenReserves,
      accumulatedCreatorFees,
      migrated,
      paused,
      createdAt,
      bump,
    };
  }

  private deserializeUserPosition(data: Buffer): UserPosition {
    let offset = 8;
    const wallet = new PublicKey(data.subarray(offset, offset + 32));
    offset += 32;
    const mint = new PublicKey(data.subarray(offset, offset + 32));
    offset += 32;
    const tokenBalance = new BN(data.subarray(offset, offset + 8), "le");
    offset += 8;
    const totalSolSpent = new BN(data.subarray(offset, offset + 8), "le");
    offset += 8;
    const totalSolReceived = new BN(data.subarray(offset, offset + 8), "le");
    offset += 8;
    const bump = data[offset];
    return { wallet, mint, tokenBalance, totalSolSpent, totalSolReceived, bump };
  }

  private deserializeLeaderboard(data: Buffer): Leaderboard {
    let offset = 8;
    const entryCount = data.readUInt32LE(offset);
    offset += 4;
    const entries: LeaderboardEntry[] = [];
    for (let i = 0; i < entryCount; i++) {
      const mint = new PublicKey(data.subarray(offset, offset + 32));
      offset += 32;
      const volume = new BN(data.subarray(offset, offset + 8), "le");
      offset += 8;
      entries.push({ mint, volume });
    }
    const bump = data[offset];
    return { entries, bump };
  }
}

// ── Re-exports for convenience ───────────────────────────────────────────────

export {
  derivePlatformConfig as getPlatformConfigPda,
  deriveTokenLaunch as getTokenLaunchPda,
  deriveUserPosition as getUserPositionPda,
  deriveLeaderboard as getLeaderboardPda,
};

export default SendItClient;
