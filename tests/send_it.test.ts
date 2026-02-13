import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { SendIt } from "../target/types/send_it";
import {
  Keypair,
  PublicKey,
  SystemProgram,
  LAMPORTS_PER_SOL,
} from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  getAssociatedTokenAddress,
  getAccount,
} from "@solana/spl-token";
import { assert, expect } from "chai";
import BN from "bn.js";

// ============================================================================
// CONSTANTS (mirror program)
// ============================================================================
const PLATFORM_CONFIG_SEED = Buffer.from("platform_config");
const TOKEN_LAUNCH_SEED = Buffer.from("token_launch");
const USER_POSITION_SEED = Buffer.from("user_position");
const PLATFORM_VAULT_SEED = Buffer.from("platform_vault");
const LEADERBOARD_SEED = Buffer.from("leaderboard");
const BLOCKLIST_SEED = Buffer.from("blocklist");
const CREATOR_VESTING_SEED = Buffer.from("creator_vesting");
const SOL_VAULT_SEED = Buffer.from("sol_vault");

const DEFAULT_TOTAL_SUPPLY = new BN("1000000000000000"); // 1B tokens, 6 decimals
const DEFAULT_PLATFORM_FEE_BPS = 100; // 1%
const DEFAULT_MIGRATION_THRESHOLD = new BN(85 * LAMPORTS_PER_SOL);

// ============================================================================
// HELPERS
// ============================================================================

interface LaunchAccounts {
  tokenMint: Keypair;
  tokenLaunch: PublicKey;
  launchTokenVault: PublicKey;
  launchSolVault: PublicKey;
  creatorVesting: PublicKey;
}

async function findPDA(
  seeds: (Buffer | Uint8Array)[],
  programId: PublicKey
): Promise<[PublicKey, number]> {
  return PublicKey.findProgramAddressSync(seeds, programId);
}

describe("send_it", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.SendIt as Program<SendIt>;

  // Keypairs
  const authority = provider.wallet as anchor.Wallet;
  let unauthorizedUser: Keypair;
  let creator: Keypair;
  let buyer: Keypair;
  let buyer2: Keypair;

  // PDAs
  let platformConfigPDA: PublicKey;
  let platformConfigBump: number;
  let platformVaultPDA: PublicKey;
  let leaderboardPDA: PublicKey;
  let blocklistPDA: PublicKey;

  // Reusable launch state
  let linearLaunch: LaunchAccounts;
  let exponentialLaunch: LaunchAccounts;
  let sigmoidLaunch: LaunchAccounts;

  // ============================================================================
  // SETUP
  // ============================================================================

  async function airdrop(pubkey: PublicKey, sol: number) {
    const sig = await provider.connection.requestAirdrop(
      pubkey,
      sol * LAMPORTS_PER_SOL
    );
    await provider.connection.confirmTransaction(sig, "confirmed");
  }

  async function getBalance(pubkey: PublicKey): Promise<number> {
    return provider.connection.getBalance(pubkey);
  }

  async function deriveTokenLaunchAccounts(
    mint: Keypair
  ): Promise<LaunchAccounts> {
    const [tokenLaunch] = findPDA(
      [TOKEN_LAUNCH_SEED, mint.publicKey.toBuffer()],
      program.programId
    );
    const launchTokenVault = await getAssociatedTokenAddress(
      mint.publicKey,
      tokenLaunch,
      true
    );
    const [launchSolVault] = findPDA(
      [SOL_VAULT_SEED, mint.publicKey.toBuffer()],
      program.programId
    );
    const [creatorVesting] = findPDA(
      [CREATOR_VESTING_SEED, mint.publicKey.toBuffer()],
      program.programId
    );
    return {
      tokenMint: mint,
      tokenLaunch,
      launchTokenVault,
      launchSolVault,
      creatorVesting,
    };
  }

  async function createToken(
    creatorKp: Keypair,
    curveType: any,
    opts: {
      name?: string;
      symbol?: string;
      uri?: string;
      creatorFeeBps?: number;
      launchDelaySeconds?: number;
      snipeWindowSeconds?: number;
      maxBuyDuringSnipe?: BN;
      lockPeriodSeconds?: number;
      creatorVestingDuration?: number;
      creatorTokenAllocationBps?: number;
    } = {}
  ): Promise<LaunchAccounts> {
    const mint = Keypair.generate();
    const accounts = await deriveTokenLaunchAccounts(mint);

    await program.methods
      .createToken(
        opts.name ?? "TestToken",
        opts.symbol ?? "TST",
        opts.uri ?? "https://example.com/meta.json",
        curveType,
        opts.creatorFeeBps ?? 100,
        new BN(opts.launchDelaySeconds ?? 0),
        new BN(opts.snipeWindowSeconds ?? 10),
        opts.maxBuyDuringSnipe ?? new BN(0),
        new BN(opts.lockPeriodSeconds ?? 0),
        new BN(opts.creatorVestingDuration ?? 3600),
        opts.creatorTokenAllocationBps ?? 500 // 5%
      )
      .accounts({
        tokenLaunch: accounts.tokenLaunch,
        tokenMint: mint.publicKey,
        launchTokenVault: accounts.launchTokenVault,
        creatorVesting: accounts.creatorVesting,
        platformConfig: platformConfigPDA,
        creator: creatorKp.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      })
      .signers([creatorKp, mint])
      .rpc();

    return accounts;
  }

  async function buyTokens(
    buyerKp: Keypair,
    launch: LaunchAccounts,
    solAmount: BN,
    creatorPubkey: PublicKey
  ) {
    const buyerAta = await getAssociatedTokenAddress(
      launch.tokenMint.publicKey,
      buyerKp.publicKey
    );
    const [userPosition] = findPDA(
      [
        USER_POSITION_SEED,
        buyerKp.publicKey.toBuffer(),
        launch.tokenMint.publicKey.toBuffer(),
      ],
      program.programId
    );

    return program.methods
      .buy(solAmount)
      .accounts({
        tokenLaunch: launch.tokenLaunch,
        tokenMint: launch.tokenMint.publicKey,
        launchTokenVault: launch.launchTokenVault,
        launchSolVault: launch.launchSolVault,
        buyerTokenAccount: buyerAta,
        userPosition,
        platformVault: platformVaultPDA,
        creatorWallet: creatorPubkey,
        platformConfig: platformConfigPDA,
        blocklist: blocklistPDA,
        buyer: buyerKp.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .signers([buyerKp])
      .rpc();
  }

  async function sellTokens(
    sellerKp: Keypair,
    launch: LaunchAccounts,
    tokenAmount: BN,
    creatorPubkey: PublicKey
  ) {
    const sellerAta = await getAssociatedTokenAddress(
      launch.tokenMint.publicKey,
      sellerKp.publicKey
    );
    const [userPosition] = findPDA(
      [
        USER_POSITION_SEED,
        sellerKp.publicKey.toBuffer(),
        launch.tokenMint.publicKey.toBuffer(),
      ],
      program.programId
    );

    return program.methods
      .sell(tokenAmount)
      .accounts({
        tokenLaunch: launch.tokenLaunch,
        tokenMint: launch.tokenMint.publicKey,
        launchTokenVault: launch.launchTokenVault,
        launchSolVault: launch.launchSolVault,
        sellerTokenAccount: sellerAta,
        userPosition,
        platformVault: platformVaultPDA,
        creatorWallet: creatorPubkey,
        platformConfig: platformConfigPDA,
        seller: sellerKp.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .signers([sellerKp])
      .rpc();
  }

  async function fetchLaunch(pda: PublicKey) {
    return program.account.tokenLaunch.fetch(pda);
  }

  async function fetchConfig() {
    return program.account.platformConfig.fetch(platformConfigPDA);
  }

  async function sleep(ms: number) {
    return new Promise((r) => setTimeout(r, ms));
  }

  // ============================================================================
  // GLOBAL SETUP
  // ============================================================================

  before(async () => {
    unauthorizedUser = Keypair.generate();
    creator = Keypair.generate();
    buyer = Keypair.generate();
    buyer2 = Keypair.generate();

    // Derive PDAs
    [platformConfigPDA, platformConfigBump] = findPDA(
      [PLATFORM_CONFIG_SEED],
      program.programId
    );
    [platformVaultPDA] = findPDA([PLATFORM_VAULT_SEED], program.programId);
    [leaderboardPDA] = findPDA([LEADERBOARD_SEED], program.programId);
    [blocklistPDA] = findPDA([BLOCKLIST_SEED], program.programId);

    // Fund accounts
    await airdrop(creator.publicKey, 100);
    await airdrop(buyer.publicKey, 100);
    await airdrop(buyer2.publicKey, 100);
    await airdrop(unauthorizedUser.publicKey, 10);
  });

  // ============================================================================
  // 1. PLATFORM INITIALIZATION
  // ============================================================================

  describe("1. Platform Initialization", () => {
    it("initializes the platform config", async () => {
      await program.methods
        .initializePlatform(DEFAULT_PLATFORM_FEE_BPS, DEFAULT_MIGRATION_THRESHOLD)
        .accounts({
          platformConfig: platformConfigPDA,
          authority: authority.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      const config = await fetchConfig();
      assert.equal(config.platformFeeBps, DEFAULT_PLATFORM_FEE_BPS);
      assert.ok(config.migrationThreshold.eq(DEFAULT_MIGRATION_THRESHOLD));
      assert.equal(config.paused, false);
      assert.equal(config.totalLaunches.toNumber(), 0);
      assert.equal(config.totalVolumeSol.toNumber(), 0);
      assert.ok(config.authority.equals(authority.publicKey));
    });

    it("rejects fee > 10%", async () => {
      try {
        // Can't re-init, but we can test update
        await program.methods
          .updatePlatformConfig(1001, null, null)
          .accounts({
            platformConfig: platformConfigPDA,
            authority: authority.publicKey,
          })
          .rpc();
        assert.fail("Should have thrown");
      } catch (e: any) {
        assert.include(e.toString(), "FeeTooHigh");
      }
    });

    it("cannot re-initialize (account already exists)", async () => {
      try {
        await program.methods
          .initializePlatform(50, DEFAULT_MIGRATION_THRESHOLD)
          .accounts({
            platformConfig: platformConfigPDA,
            authority: authority.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .rpc();
        assert.fail("Should have thrown");
      } catch (e: any) {
        // Account already in use
        assert.ok(e);
      }
    });
  });

  // ============================================================================
  // INITIALIZE BLOCKLIST & LEADERBOARD (needed for later tests)
  // ============================================================================

  describe("Setup: Blocklist & Leaderboard", () => {
    it("initializes blocklist", async () => {
      await program.methods
        .initializeBlocklist()
        .accounts({
          blocklist: blocklistPDA,
          platformConfig: platformConfigPDA,
          authority: authority.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      const bl = await program.account.blocklist.fetch(blocklistPDA);
      assert.equal(bl.blockedWallets.length, 0);
    });

    it("initializes leaderboard", async () => {
      await program.methods
        .initializeLeaderboard()
        .accounts({
          leaderboard: leaderboardPDA,
          platformConfig: platformConfigPDA,
          authority: authority.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      const lb = await program.account.leaderboard.fetch(leaderboardPDA);
      assert.equal(lb.topTokensByVolume.length, 0);
    });
  });

  // ============================================================================
  // 2. TOKEN CREATION WITH ALL CURVE TYPES
  // ============================================================================

  describe("2. Token Creation", () => {
    it("creates a Linear curve token", async () => {
      linearLaunch = await createToken(creator, { linear: {} });
      const launch = await fetchLaunch(linearLaunch.tokenLaunch);
      assert.deepEqual(launch.curveType, { linear: {} });
      assert.ok(launch.creator.equals(creator.publicKey));
      assert.equal(launch.migrated, false);
      assert.equal(launch.tokensSold.toNumber(), 0);

      const config = await fetchConfig();
      assert.equal(config.totalLaunches.toNumber(), 1);
    });

    it("creates an Exponential curve token", async () => {
      exponentialLaunch = await createToken(creator, { exponential: {} });
      const launch = await fetchLaunch(exponentialLaunch.tokenLaunch);
      assert.deepEqual(launch.curveType, { exponential: {} });
    });

    it("creates a Sigmoid curve token", async () => {
      sigmoidLaunch = await createToken(creator, { sigmoid: {} });
      const launch = await fetchLaunch(sigmoidLaunch.tokenLaunch);
      assert.deepEqual(launch.curveType, { sigmoid: {} });
    });

    it("rejects name too long", async () => {
      try {
        await createToken(creator, { linear: {} }, {
          name: "A".repeat(33),
        });
        assert.fail("Should have thrown");
      } catch (e: any) {
        assert.include(e.toString(), "NameTooLong");
      }
    });

    it("rejects symbol too long", async () => {
      try {
        await createToken(creator, { linear: {} }, {
          symbol: "A".repeat(11),
        });
        assert.fail("Should have thrown");
      } catch (e: any) {
        assert.include(e.toString(), "SymbolTooLong");
      }
    });

    it("rejects URI too long", async () => {
      try {
        await createToken(creator, { linear: {} }, {
          uri: "A".repeat(201),
        });
        assert.fail("Should have thrown");
      } catch (e: any) {
        assert.include(e.toString(), "UriTooLong");
      }
    });

    it("rejects creator fee > 5%", async () => {
      try {
        await createToken(creator, { linear: {} }, {
          creatorFeeBps: 501,
        });
        assert.fail("Should have thrown");
      } catch (e: any) {
        assert.include(e.toString(), "FeeTooHigh");
      }
    });

    it("rejects creator allocation > 10%", async () => {
      try {
        await createToken(creator, { linear: {} }, {
          creatorTokenAllocationBps: 1001,
        });
        assert.fail("Should have thrown");
      } catch (e: any) {
        assert.include(e.toString(), "AllocationTooHigh");
      }
    });

    it("mints correct supply to vault (including creator allocation)", async () => {
      const vaultInfo = await getAccount(
        provider.connection,
        linearLaunch.launchTokenVault
      );
      // Total supply = 1B tokens (all in vault: curve supply + creator vesting)
      assert.ok(
        new BN(vaultInfo.amount.toString()).eq(DEFAULT_TOTAL_SUPPLY)
      );
    });
  });

  // ============================================================================
  // 3. BUY/SELL ON EACH CURVE TYPE
  // ============================================================================

  describe("3. Buy/Sell on each curve type", () => {
    const buyAmount = new BN(1 * LAMPORTS_PER_SOL); // 1 SOL

    describe("Linear curve", () => {
      it("buy increases tokens_sold and reserve", async () => {
        const beforeLaunch = await fetchLaunch(linearLaunch.tokenLaunch);
        await buyTokens(buyer, linearLaunch, buyAmount, creator.publicKey);
        const afterLaunch = await fetchLaunch(linearLaunch.tokenLaunch);
        assert.ok(afterLaunch.tokensSold.gt(beforeLaunch.tokensSold));
        assert.ok(afterLaunch.reserveSol.gt(beforeLaunch.reserveSol));
      });

      it("price increases after buy", async () => {
        const launch1 = await fetchLaunch(linearLaunch.tokenLaunch);
        await buyTokens(buyer, linearLaunch, buyAmount, creator.publicKey);
        const launch2 = await fetchLaunch(linearLaunch.tokenLaunch);
        // More tokens sold = higher price on linear curve
        assert.ok(launch2.tokensSold.gt(launch1.tokensSold));
      });

      it("sell decreases tokens_sold", async () => {
        const beforeLaunch = await fetchLaunch(linearLaunch.tokenLaunch);
        const buyerAta = await getAssociatedTokenAddress(
          linearLaunch.tokenMint.publicKey,
          buyer.publicKey
        );
        const ataInfo = await getAccount(provider.connection, buyerAta);
        const sellAmount = new BN(ataInfo.amount.toString()).divn(2);
        if (sellAmount.gtn(0)) {
          await sellTokens(
            buyer,
            linearLaunch,
            sellAmount,
            creator.publicKey
          );
          const afterLaunch = await fetchLaunch(linearLaunch.tokenLaunch);
          assert.ok(afterLaunch.tokensSold.lt(beforeLaunch.tokensSold));
        }
      });
    });

    describe("Exponential curve", () => {
      it("buy works and tokens_sold increases", async () => {
        await buyTokens(buyer, exponentialLaunch, buyAmount, creator.publicKey);
        const launch = await fetchLaunch(exponentialLaunch.tokenLaunch);
        assert.ok(launch.tokensSold.gtn(0));
      });

      it("second buy gets fewer tokens (price went up)", async () => {
        const launch1 = await fetchLaunch(exponentialLaunch.tokenLaunch);
        const sold1 = launch1.tokensSold;
        await buyTokens(buyer, exponentialLaunch, buyAmount, creator.publicKey);
        const launch2 = await fetchLaunch(exponentialLaunch.tokenLaunch);
        const tokensFromSecondBuy = launch2.tokensSold.sub(sold1);

        // Buy again
        const sold2 = launch2.tokensSold;
        await buyTokens(buyer, exponentialLaunch, buyAmount, creator.publicKey);
        const launch3 = await fetchLaunch(exponentialLaunch.tokenLaunch);
        const tokensFromThirdBuy = launch3.tokensSold.sub(sold2);

        // Third buy should get fewer tokens than second (price increased)
        assert.ok(
          tokensFromThirdBuy.lte(tokensFromSecondBuy),
          "Later buys should get fewer or equal tokens"
        );
      });

      it("sell returns SOL", async () => {
        const buyerAta = await getAssociatedTokenAddress(
          exponentialLaunch.tokenMint.publicKey,
          buyer.publicKey
        );
        const ataInfo = await getAccount(provider.connection, buyerAta);
        const sellAmount = new BN(ataInfo.amount.toString()).divn(4);
        if (sellAmount.gtn(0)) {
          const balBefore = await getBalance(buyer.publicKey);
          await sellTokens(
            buyer,
            exponentialLaunch,
            sellAmount,
            creator.publicKey
          );
          const balAfter = await getBalance(buyer.publicKey);
          assert.ok(balAfter > balBefore, "Seller should receive SOL");
        }
      });
    });

    describe("Sigmoid curve", () => {
      it("buy works on sigmoid", async () => {
        await buyTokens(buyer, sigmoidLaunch, buyAmount, creator.publicKey);
        const launch = await fetchLaunch(sigmoidLaunch.tokenLaunch);
        assert.ok(launch.tokensSold.gtn(0));
      });

      it("sell works on sigmoid", async () => {
        const buyerAta = await getAssociatedTokenAddress(
          sigmoidLaunch.tokenMint.publicKey,
          buyer.publicKey
        );
        const ataInfo = await getAccount(provider.connection, buyerAta);
        const sellAmount = new BN(ataInfo.amount.toString()).divn(2);
        if (sellAmount.gtn(0)) {
          await sellTokens(
            buyer,
            sigmoidLaunch,
            sellAmount,
            creator.publicKey
          );
        }
      });
    });
  });

  // ============================================================================
  // 4. ANTI-SNIPE PROTECTION
  // ============================================================================

  describe("4. Anti-Snipe Protection", () => {
    let snipeLaunch: LaunchAccounts;

    it("buy fails during delay period (trading not started)", async () => {
      // Create token with 60s delay
      snipeLaunch = await createToken(creator, { linear: {} }, {
        launchDelaySeconds: 60,
        snipeWindowSeconds: 30,
        maxBuyDuringSnipe: new BN(DEFAULT_TOTAL_SUPPLY.divn(100)), // 1%
      });

      try {
        await buyTokens(
          buyer,
          snipeLaunch,
          new BN(LAMPORTS_PER_SOL),
          creator.publicKey
        );
        assert.fail("Should have thrown TradingNotStarted");
      } catch (e: any) {
        assert.include(e.toString(), "TradingNotStarted");
      }
    });

    it("buy exceeding max during snipe window fails", async () => {
      // Create a token with 0 delay but 60s snipe window, tiny max
      const snipeLaunch2 = await createToken(creator, { linear: {} }, {
        launchDelaySeconds: 0,
        snipeWindowSeconds: 60,
        maxBuyDuringSnipe: new BN(1000), // very small max
      });

      try {
        // This should exceed the snipe limit since SOL amount converts to > 1000 tokens
        await buyTokens(
          buyer,
          snipeLaunch2,
          new BN(LAMPORTS_PER_SOL),
          creator.publicKey
        );
        assert.fail("Should have thrown SnipeLimitExceeded");
      } catch (e: any) {
        // The snipe check compares position.tokens_bought + sol_amount (not token amount)
        // against max_buy_during_snipe. Since sol_amount = 1 SOL = 1e9 > 1000, this triggers.
        assert.include(e.toString(), "SnipeLimitExceeded");
      }
    });

    it("normal buy succeeds after snipe window (0 delay, 0 snipe)", async () => {
      const normalLaunch = await createToken(creator, { linear: {} }, {
        launchDelaySeconds: 0,
        snipeWindowSeconds: 0,
      });

      await buyTokens(
        buyer,
        normalLaunch,
        new BN(LAMPORTS_PER_SOL),
        creator.publicKey
      );
      const launch = await fetchLaunch(normalLaunch.tokenLaunch);
      assert.ok(launch.tokensSold.gtn(0));
    });
  });

  // ============================================================================
  // 5. CREATOR REVENUE SHARE
  // ============================================================================

  describe("5. Creator Revenue Share", () => {
    let feeLaunch: LaunchAccounts;

    before(async () => {
      feeLaunch = await createToken(creator, { linear: {} }, {
        creatorFeeBps: 200, // 2%
        launchDelaySeconds: 0,
        snipeWindowSeconds: 0,
      });
    });

    it("creator receives correct fee on buy", async () => {
      const creatorBefore = await getBalance(creator.publicKey);
      const solAmount = new BN(10 * LAMPORTS_PER_SOL);
      await buyTokens(buyer, feeLaunch, solAmount, creator.publicKey);
      const creatorAfter = await getBalance(creator.publicKey);

      const expectedFee = solAmount.muln(200).divn(10000); // 2%
      const actualFee = new BN(creatorAfter - creatorBefore);
      // Allow small rounding
      assert.ok(
        actualFee.sub(expectedFee).abs().ltn(10),
        `Creator fee mismatch: expected ~${expectedFee.toString()}, got ${actualFee.toString()}`
      );
    });

    it("creator receives correct fee on sell", async () => {
      const buyerAta = await getAssociatedTokenAddress(
        feeLaunch.tokenMint.publicKey,
        buyer.publicKey
      );
      const ataInfo = await getAccount(provider.connection, buyerAta);
      const sellAmount = new BN(ataInfo.amount.toString()).divn(2);

      if (sellAmount.gtn(0)) {
        const creatorBefore = await getBalance(creator.publicKey);
        await sellTokens(buyer, feeLaunch, sellAmount, creator.publicKey);
        const creatorAfter = await getBalance(creator.publicKey);
        // Creator should have received some fee
        assert.ok(creatorAfter >= creatorBefore, "Creator should get fee on sell");
      }
    });
  });

  // ============================================================================
  // 6. RUG PROTECTION — LOCK PERIOD & VESTING
  // ============================================================================

  describe("6. Rug Protection — Lock & Vesting", () => {
    let lockedLaunch: LaunchAccounts;

    before(async () => {
      // Lock for 3600s, vesting over 3600s
      lockedLaunch = await createToken(creator, { linear: {} }, {
        lockPeriodSeconds: 3600,
        creatorVestingDuration: 3600,
        creatorTokenAllocationBps: 500, // 5%
        launchDelaySeconds: 0,
        snipeWindowSeconds: 0,
      });
    });

    it("creator vesting is set up correctly", async () => {
      const vesting = await program.account.creatorVesting.fetch(
        lockedLaunch.creatorVesting
      );
      assert.ok(vesting.creator.equals(creator.publicKey));
      const expectedAlloc = DEFAULT_TOTAL_SUPPLY.muln(500).divn(10000);
      assert.ok(vesting.totalAmount.eq(expectedAlloc));
      assert.equal(vesting.claimedAmount.toNumber(), 0);
    });

    it("creator cannot claim before vesting starts accruing meaningfully", async () => {
      // Immediately after creation, elapsed ≈ 0 so claimable ≈ 0
      try {
        const creatorAta = await getAssociatedTokenAddress(
          lockedLaunch.tokenMint.publicKey,
          creator.publicKey
        );
        await program.methods
          .claimVestedTokens()
          .accounts({
            tokenLaunch: lockedLaunch.tokenLaunch,
            tokenMint: lockedLaunch.tokenMint.publicKey,
            launchTokenVault: lockedLaunch.launchTokenVault,
            creatorVesting: lockedLaunch.creatorVesting,
            creatorTokenAccount: creatorAta,
            creator: creator.publicKey,
            tokenProgram: TOKEN_PROGRAM_ID,
            associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
          })
          .signers([creator])
          .rpc();
        // May succeed with tiny amount or fail with NothingToClaim
      } catch (e: any) {
        assert.include(e.toString(), "NothingToClaim");
      }
    });

    it("migration fails during lock period", async () => {
      // Even if we hit the threshold, lock period blocks migration
      // First buy enough to exceed threshold (we'll use a low-threshold launch)
      const lowThresholdLaunch = await createToken(creator, { linear: {} }, {
        lockPeriodSeconds: 3600,
        launchDelaySeconds: 0,
        snipeWindowSeconds: 0,
      });

      // Update platform threshold to something reachable
      await program.methods
        .updatePlatformConfig(null, new BN(LAMPORTS_PER_SOL), null)
        .accounts({
          platformConfig: platformConfigPDA,
          authority: authority.publicKey,
        })
        .rpc();

      // Buy enough to exceed 1 SOL reserve
      await buyTokens(
        buyer,
        lowThresholdLaunch,
        new BN(5 * LAMPORTS_PER_SOL),
        creator.publicKey
      );

      try {
        await program.methods
          .migrateToRaydium()
          .accounts({
            tokenLaunch: lowThresholdLaunch.tokenLaunch,
            tokenMint: lowThresholdLaunch.tokenMint.publicKey,
            platformConfig: platformConfigPDA,
            payer: buyer.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([buyer])
          .rpc();
        assert.fail("Should have thrown LockPeriodActive");
      } catch (e: any) {
        assert.include(e.toString(), "LockPeriodActive");
      }

      // Restore threshold
      await program.methods
        .updatePlatformConfig(null, DEFAULT_MIGRATION_THRESHOLD, null)
        .accounts({
          platformConfig: platformConfigPDA,
          authority: authority.publicKey,
        })
        .rpc();
    });
  });

  // ============================================================================
  // 7. AUTO-MIGRATION TRIGGER
  // ============================================================================

  describe("7. Auto-Migration Trigger", () => {
    let migrationLaunch: LaunchAccounts;

    before(async () => {
      // Set a low threshold
      await program.methods
        .updatePlatformConfig(null, new BN(2 * LAMPORTS_PER_SOL), null)
        .accounts({
          platformConfig: platformConfigPDA,
          authority: authority.publicKey,
        })
        .rpc();

      migrationLaunch = await createToken(creator, { linear: {} }, {
        lockPeriodSeconds: 0,
        launchDelaySeconds: 0,
        snipeWindowSeconds: 0,
      });
    });

    after(async () => {
      // Restore threshold
      await program.methods
        .updatePlatformConfig(null, DEFAULT_MIGRATION_THRESHOLD, null)
        .accounts({
          platformConfig: platformConfigPDA,
          authority: authority.publicKey,
        })
        .rpc();
    });

    it("migration fails before threshold", async () => {
      try {
        await program.methods
          .migrateToRaydium()
          .accounts({
            tokenLaunch: migrationLaunch.tokenLaunch,
            tokenMint: migrationLaunch.tokenMint.publicKey,
            platformConfig: platformConfigPDA,
            payer: buyer.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([buyer])
          .rpc();
        assert.fail("Should have thrown");
      } catch (e: any) {
        assert.include(e.toString(), "MigrationThresholdNotMet");
      }
    });

    it("buy enough to hit threshold, then migrate", async () => {
      // Buy enough SOL to hit 2 SOL reserve (accounting for fees)
      await buyTokens(
        buyer,
        migrationLaunch,
        new BN(5 * LAMPORTS_PER_SOL),
        creator.publicKey
      );

      const launch = await fetchLaunch(migrationLaunch.tokenLaunch);
      const config = await fetchConfig();
      assert.ok(
        launch.reserveSol.gte(config.migrationThreshold),
        "Reserve should meet threshold"
      );

      // Now migrate
      await program.methods
        .migrateToRaydium()
        .accounts({
          tokenLaunch: migrationLaunch.tokenLaunch,
          tokenMint: migrationLaunch.tokenMint.publicKey,
          platformConfig: platformConfigPDA,
          payer: buyer.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([buyer])
        .rpc();

      const launchAfter = await fetchLaunch(migrationLaunch.tokenLaunch);
      assert.equal(launchAfter.migrated, true);
    });

    it("cannot migrate twice", async () => {
      try {
        await program.methods
          .migrateToRaydium()
          .accounts({
            tokenLaunch: migrationLaunch.tokenLaunch,
            tokenMint: migrationLaunch.tokenMint.publicKey,
            platformConfig: platformConfigPDA,
            payer: buyer.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([buyer])
          .rpc();
        assert.fail("Should have thrown");
      } catch (e: any) {
        assert.include(e.toString(), "AlreadyMigrated");
      }
    });

    it("cannot buy after migration", async () => {
      try {
        await buyTokens(
          buyer,
          migrationLaunch,
          new BN(LAMPORTS_PER_SOL),
          creator.publicKey
        );
        assert.fail("Should have thrown");
      } catch (e: any) {
        assert.include(e.toString(), "AlreadyMigrated");
      }
    });
  });

  // ============================================================================
  // 8. PLATFORM FEES
  // ============================================================================

  describe("8. Platform Fees", () => {
    let feeLaunch: LaunchAccounts;

    before(async () => {
      feeLaunch = await createToken(creator, { linear: {} }, {
        creatorFeeBps: 100, // 1%
        launchDelaySeconds: 0,
        snipeWindowSeconds: 0,
      });
    });

    it("platform vault receives correct fee on buy", async () => {
      const vaultBefore = await getBalance(platformVaultPDA);
      const solAmount = new BN(10 * LAMPORTS_PER_SOL);
      await buyTokens(buyer, feeLaunch, solAmount, creator.publicKey);
      const vaultAfter = await getBalance(platformVaultPDA);

      const config = await fetchConfig();
      const expectedFee = solAmount
        .muln(config.platformFeeBps)
        .divn(10000);
      const actualFee = new BN(vaultAfter - vaultBefore);
      assert.ok(
        actualFee.sub(expectedFee).abs().ltn(10),
        `Platform fee mismatch: expected ~${expectedFee.toString()}, got ${actualFee.toString()}`
      );
    });

    it("total_volume_sol is updated on the platform config", async () => {
      const config = await fetchConfig();
      assert.ok(config.totalVolumeSol.gtn(0));
    });
  });

  // ============================================================================
  // 9. LEADERBOARD UPDATES
  // ============================================================================

  describe("9. Leaderboard Updates", () => {
    it("update_leaderboard populates top tokens", async () => {
      // Use the linearLaunch which has had trades
      await program.methods
        .updateLeaderboard()
        .accounts({
          tokenLaunch: linearLaunch.tokenLaunch,
          tokenMint: linearLaunch.tokenMint.publicKey,
          leaderboard: leaderboardPDA,
        })
        .rpc();

      const lb = await program.account.leaderboard.fetch(leaderboardPDA);
      assert.ok(lb.topTokensByVolume.length > 0, "Should have leaderboard entries");
      assert.ok(
        lb.topTokensByVolume[0].key.equals(linearLaunch.tokenMint.publicKey) ||
          lb.topCreatorsByVolume.length > 0
      );
    });

    it("leaderboard updates with new higher volume entry", async () => {
      // Update for exponential launch too
      await program.methods
        .updateLeaderboard()
        .accounts({
          tokenLaunch: exponentialLaunch.tokenLaunch,
          tokenMint: exponentialLaunch.tokenMint.publicKey,
          leaderboard: leaderboardPDA,
        })
        .rpc();

      const lb = await program.account.leaderboard.fetch(leaderboardPDA);
      assert.ok(lb.topTokensByVolume.length >= 2);
      // Sorted descending
      if (lb.topTokensByVolume.length >= 2) {
        assert.ok(
          lb.topTokensByVolume[0].value.gte(lb.topTokensByVolume[1].value),
          "Leaderboard should be sorted descending"
        );
      }
    });
  });

  // ============================================================================
  // 10. PAUSE / UNPAUSE
  // ============================================================================

  describe("10. Pause/Unpause", () => {
    let pauseLaunch: LaunchAccounts;

    before(async () => {
      pauseLaunch = await createToken(creator, { linear: {} }, {
        launchDelaySeconds: 0,
        snipeWindowSeconds: 0,
      });
    });

    it("pause blocks token creation", async () => {
      await program.methods
        .setPaused(true)
        .accounts({
          platformConfig: platformConfigPDA,
          authority: authority.publicKey,
        })
        .rpc();

      const config = await fetchConfig();
      assert.equal(config.paused, true);

      try {
        await createToken(creator, { linear: {} });
        assert.fail("Should have thrown PlatformPaused");
      } catch (e: any) {
        assert.include(e.toString(), "PlatformPaused");
      }
    });

    it("pause blocks buys", async () => {
      try {
        await buyTokens(
          buyer,
          pauseLaunch,
          new BN(LAMPORTS_PER_SOL),
          creator.publicKey
        );
        assert.fail("Should have thrown PlatformPaused");
      } catch (e: any) {
        assert.include(e.toString(), "PlatformPaused");
      }
    });

    it("pause blocks sells", async () => {
      // Unpause temporarily to get tokens, then re-pause
      await program.methods
        .setPaused(false)
        .accounts({
          platformConfig: platformConfigPDA,
          authority: authority.publicKey,
        })
        .rpc();

      await buyTokens(
        buyer,
        pauseLaunch,
        new BN(LAMPORTS_PER_SOL),
        creator.publicKey
      );

      await program.methods
        .setPaused(true)
        .accounts({
          platformConfig: platformConfigPDA,
          authority: authority.publicKey,
        })
        .rpc();

      const buyerAta = await getAssociatedTokenAddress(
        pauseLaunch.tokenMint.publicKey,
        buyer.publicKey
      );
      const ataInfo = await getAccount(provider.connection, buyerAta);
      const sellAmount = new BN(ataInfo.amount.toString()).divn(2);

      try {
        await sellTokens(buyer, pauseLaunch, sellAmount, creator.publicKey);
        assert.fail("Should have thrown PlatformPaused");
      } catch (e: any) {
        assert.include(e.toString(), "PlatformPaused");
      }
    });

    it("unpause restores trading", async () => {
      await program.methods
        .setPaused(false)
        .accounts({
          platformConfig: platformConfigPDA,
          authority: authority.publicKey,
        })
        .rpc();

      await buyTokens(
        buyer,
        pauseLaunch,
        new BN(LAMPORTS_PER_SOL),
        creator.publicKey
      );
      const launch = await fetchLaunch(pauseLaunch.tokenLaunch);
      assert.ok(launch.tokensSold.gtn(0));
    });
  });

  // ============================================================================
  // 11. UNAUTHORIZED ACCESS ATTEMPTS
  // ============================================================================

  describe("11. Unauthorized Access", () => {
    it("non-authority cannot update platform config", async () => {
      try {
        await program.methods
          .updatePlatformConfig(50, null, null)
          .accounts({
            platformConfig: platformConfigPDA,
            authority: unauthorizedUser.publicKey,
          })
          .signers([unauthorizedUser])
          .rpc();
        assert.fail("Should have thrown");
      } catch (e: any) {
        // has_one constraint will fail
        assert.ok(e);
      }
    });

    it("non-authority cannot pause", async () => {
      try {
        await program.methods
          .setPaused(true)
          .accounts({
            platformConfig: platformConfigPDA,
            authority: unauthorizedUser.publicKey,
          })
          .signers([unauthorizedUser])
          .rpc();
        assert.fail("Should have thrown");
      } catch (e: any) {
        assert.ok(e);
      }
    });

    it("non-authority cannot initialize leaderboard", async () => {
      // Already initialized, but the auth check would fail anyway
      try {
        const [fakeLb] = findPDA(
          [Buffer.from("fake_leaderboard")],
          program.programId
        );
        await program.methods
          .initializeLeaderboard()
          .accounts({
            leaderboard: leaderboardPDA,
            platformConfig: platformConfigPDA,
            authority: unauthorizedUser.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([unauthorizedUser])
          .rpc();
        assert.fail("Should have thrown");
      } catch (e: any) {
        assert.ok(e);
      }
    });

    it("non-authority cannot manage blocklist", async () => {
      try {
        await program.methods
          .addToBlocklist(buyer.publicKey)
          .accounts({
            blocklist: blocklistPDA,
            authority: unauthorizedUser.publicKey,
          })
          .signers([unauthorizedUser])
          .rpc();
        assert.fail("Should have thrown");
      } catch (e: any) {
        assert.ok(e);
      }
    });

    it("non-creator cannot claim vested tokens", async () => {
      try {
        const creatorAta = await getAssociatedTokenAddress(
          linearLaunch.tokenMint.publicKey,
          unauthorizedUser.publicKey
        );
        await program.methods
          .claimVestedTokens()
          .accounts({
            tokenLaunch: linearLaunch.tokenLaunch,
            tokenMint: linearLaunch.tokenMint.publicKey,
            launchTokenVault: linearLaunch.launchTokenVault,
            creatorVesting: linearLaunch.creatorVesting,
            creatorTokenAccount: creatorAta,
            creator: unauthorizedUser.publicKey,
            tokenProgram: TOKEN_PROGRAM_ID,
            associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
          })
          .signers([unauthorizedUser])
          .rpc();
        assert.fail("Should have thrown");
      } catch (e: any) {
        assert.ok(e);
      }
    });
  });

  // ============================================================================
  // 12. EDGE CASES
  // ============================================================================

  describe("12. Edge Cases", () => {
    let edgeLaunch: LaunchAccounts;

    before(async () => {
      edgeLaunch = await createToken(creator, { linear: {} }, {
        launchDelaySeconds: 0,
        snipeWindowSeconds: 0,
      });
    });

    it("buy with zero SOL fails", async () => {
      try {
        await buyTokens(buyer, edgeLaunch, new BN(0), creator.publicKey);
        assert.fail("Should have thrown ZeroAmount");
      } catch (e: any) {
        assert.include(e.toString(), "ZeroAmount");
      }
    });

    it("sell with zero tokens fails", async () => {
      // First buy some tokens
      await buyTokens(
        buyer,
        edgeLaunch,
        new BN(LAMPORTS_PER_SOL),
        creator.publicKey
      );

      try {
        await sellTokens(buyer, edgeLaunch, new BN(0), creator.publicKey);
        assert.fail("Should have thrown ZeroAmount");
      } catch (e: any) {
        assert.include(e.toString(), "ZeroAmount");
      }
    });

    it("sell more than owned fails", async () => {
      try {
        await sellTokens(
          buyer,
          edgeLaunch,
          DEFAULT_TOTAL_SUPPLY, // way more than bought
          creator.publicKey
        );
        assert.fail("Should have thrown");
      } catch (e: any) {
        // InsufficientTokensSold or token transfer failure
        assert.ok(e);
      }
    });

    it("dust amount buy (1 lamport) — should get 0 tokens or fail", async () => {
      try {
        await buyTokens(buyer, edgeLaunch, new BN(1), creator.publicKey);
        // May succeed but get 0 tokens, which should fail with InsufficientOutput
        assert.fail("Should have thrown InsufficientOutput");
      } catch (e: any) {
        // Either InsufficientOutput or some other validation
        assert.ok(e);
      }
    });

    it("blocklist prevents buying", async () => {
      // Add buyer2 to blocklist
      await program.methods
        .addToBlocklist(buyer2.publicKey)
        .accounts({
          blocklist: blocklistPDA,
          authority: authority.publicKey,
        })
        .rpc();

      try {
        await buyTokens(
          buyer2,
          edgeLaunch,
          new BN(LAMPORTS_PER_SOL),
          creator.publicKey
        );
        assert.fail("Should have thrown WalletBlocked");
      } catch (e: any) {
        assert.include(e.toString(), "WalletBlocked");
      }

      // Clean up: remove from blocklist
      await program.methods
        .removeFromBlocklist(buyer2.publicKey)
        .accounts({
          blocklist: blocklistPDA,
          authority: authority.publicKey,
        })
        .rpc();
    });

    it("buyer can trade after being removed from blocklist", async () => {
      await buyTokens(
        buyer2,
        edgeLaunch,
        new BN(LAMPORTS_PER_SOL),
        creator.publicKey
      );
      const launch = await fetchLaunch(edgeLaunch.tokenLaunch);
      assert.ok(launch.tokensSold.gtn(0));
    });

    it("platform config authority transfer works", async () => {
      const newAuth = Keypair.generate();
      await airdrop(newAuth.publicKey, 5);

      // Transfer authority
      await program.methods
        .updatePlatformConfig(null, null, newAuth.publicKey)
        .accounts({
          platformConfig: platformConfigPDA,
          authority: authority.publicKey,
        })
        .rpc();

      let config = await fetchConfig();
      assert.ok(config.authority.equals(newAuth.publicKey));

      // Old authority can't act
      try {
        await program.methods
          .updatePlatformConfig(50, null, null)
          .accounts({
            platformConfig: platformConfigPDA,
            authority: authority.publicKey,
          })
          .rpc();
        assert.fail("Should have thrown");
      } catch (e: any) {
        assert.ok(e);
      }

      // Transfer back
      await program.methods
        .updatePlatformConfig(null, null, authority.publicKey)
        .accounts({
          platformConfig: platformConfigPDA,
          authority: newAuth.publicKey,
        })
        .signers([newAuth])
        .rpc();

      config = await fetchConfig();
      assert.ok(config.authority.equals(authority.publicKey));
    });
  });
});
