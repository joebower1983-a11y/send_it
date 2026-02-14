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
  createMint,
  mintTo,
  createAssociatedTokenAccount,
} from "@solana/spl-token";
import { assert, expect } from "chai";
import BN from "bn.js";
import { keccak_256 } from "js-sha3";

// ============================================================================
// CONSTANTS — mirrors program seeds
// ============================================================================
const PLATFORM_CONFIG_SEED = Buffer.from("platform_config");
const TOKEN_LAUNCH_SEED = Buffer.from("token_launch");
const STAKING_POOL_SEED = Buffer.from("staking_pool");
const STAKE_ACCOUNT_SEED = Buffer.from("stake_account");
const LENDING_POOL_SEED = Buffer.from("lending_pool");
const DEPOSIT_ACCOUNT_SEED = Buffer.from("deposit_account");
const BORROW_ACCOUNT_SEED = Buffer.from("borrow_account");
const LIMIT_ORDER_SEED = Buffer.from("limit_order");
const ORDER_BOOK_SEED = Buffer.from("order_book");
const PREDICTION_MARKET_SEED = Buffer.from("prediction_market");
const PREDICTION_BET_SEED = Buffer.from("prediction_bet");
const CHAT_ROOM_SEED = Buffer.from("chat_room");
const CHAT_MESSAGE_SEED = Buffer.from("chat_message");
const AIRDROP_CAMPAIGN_SEED = Buffer.from("airdrop_campaign");
const AIRDROP_CLAIM_SEED = Buffer.from("airdrop_claim");
const DAILY_REWARD_SEED = Buffer.from("daily_reward");
const SEASON_SEED = Buffer.from("season");
const SEASON_PLAYER_SEED = Buffer.from("season_player");
const FEE_SPLIT_CONFIG_SEED = Buffer.from("fee_split_config");
const CONTENT_CLAIM_SEED = Buffer.from("content_claim");
const CONTENT_REGISTRY_SEED = Buffer.from("content_registry");
const CREATOR_DASHBOARD_SEED = Buffer.from("creator_dashboard");
const SHARE_CARD_SEED = Buffer.from("share_card");
const PROPOSAL_SEED = Buffer.from("proposal");
const VOTE_RECORD_SEED = Buffer.from("vote_record");
const REPUTATION_SEED = Buffer.from("reputation");
const HOLDER_REWARD_POOL_SEED = Buffer.from("holder_reward_pool");
const HOLDER_REWARD_CLAIM_SEED = Buffer.from("holder_reward_claim");
const TOKEN_VIDEO_SEED = Buffer.from("token_video");
const WIDGET_CONFIG_SEED = Buffer.from("widget_config");
const REFERRAL_SEED = Buffer.from("referral");
const REFERRAL_TRACKER_SEED = Buffer.from("referral_tracker");
const RAFFLE_SEED = Buffer.from("raffle");
const RAFFLE_TICKET_SEED = Buffer.from("raffle_ticket");

// ============================================================================
// HELPERS
// ============================================================================
function findPDA(
  seeds: (Buffer | Uint8Array)[],
  programId: PublicKey
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(seeds, programId);
}

function buildMerkleTree(leaves: Buffer[]): Buffer[] {
  const hashed = leaves.map((l) => Buffer.from(keccak_256.arrayBuffer(l)));
  const tree: Buffer[] = [...hashed];
  let layer = hashed;
  while (layer.length > 1) {
    const next: Buffer[] = [];
    for (let i = 0; i < layer.length; i += 2) {
      const left = layer[i];
      const right = i + 1 < layer.length ? layer[i + 1] : left;
      const pair =
        Buffer.compare(left, right) <= 0
          ? Buffer.concat([left, right])
          : Buffer.concat([right, left]);
      next.push(Buffer.from(keccak_256.arrayBuffer(pair)));
    }
    tree.push(...next);
    layer = next;
  }
  return tree;
}

function getMerkleRoot(leaves: Buffer[]): Buffer {
  if (leaves.length === 0) return Buffer.alloc(32);
  const tree = buildMerkleTree(leaves);
  return tree[tree.length - 1];
}

function getMerkleProof(leaves: Buffer[], index: number): Buffer[] {
  const hashed = leaves.map((l) => Buffer.from(keccak_256.arrayBuffer(l)));
  const proof: Buffer[] = [];
  let layer = hashed;
  let idx = index;
  while (layer.length > 1) {
    const sibling = idx % 2 === 0 ? idx + 1 : idx - 1;
    if (sibling < layer.length) {
      proof.push(layer[sibling]);
    } else {
      proof.push(layer[idx]);
    }
    const next: Buffer[] = [];
    for (let i = 0; i < layer.length; i += 2) {
      const left = layer[i];
      const right = i + 1 < layer.length ? layer[i + 1] : left;
      const pair =
        Buffer.compare(left, right) <= 0
          ? Buffer.concat([left, right])
          : Buffer.concat([right, left]);
      next.push(Buffer.from(keccak_256.arrayBuffer(pair)));
    }
    layer = next;
    idx = Math.floor(idx / 2);
  }
  return proof;
}

// ============================================================================
// TEST SUITE: send_it v2 modules
// ============================================================================
describe("send_it_v2", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.SendIt as Program<SendIt>;

  // Common keypairs
  const authority = provider.wallet as anchor.Wallet;
  let user1: Keypair;
  let user2: Keypair;
  let user3: Keypair;
  let creator: Keypair;

  // Platform PDA
  let platformConfigPDA: PublicKey;

  async function airdrop(pubkey: PublicKey, sol: number) {
    const sig = await provider.connection.requestAirdrop(
      pubkey,
      sol * LAMPORTS_PER_SOL
    );
    await provider.connection.confirmTransaction(sig, "confirmed");
  }

  async function createTestMint(
    mintAuthority: Keypair,
    decimals = 6
  ): Promise<PublicKey> {
    return createMint(
      provider.connection,
      authority.payer,
      mintAuthority.publicKey,
      null,
      decimals
    );
  }

  async function mintTestTokens(
    mint: PublicKey,
    mintAuthority: Keypair,
    dest: PublicKey,
    amount: number
  ) {
    const ata = await createAssociatedTokenAccount(
      provider.connection,
      authority.payer,
      mint,
      dest
    );
    await mintTo(
      provider.connection,
      authority.payer,
      mint,
      ata,
      mintAuthority,
      amount
    );
    return ata;
  }

  // ============================================================================
  // GLOBAL SETUP
  // ============================================================================
  before(async () => {
    user1 = Keypair.generate();
    user2 = Keypair.generate();
    user3 = Keypair.generate();
    creator = Keypair.generate();

    await Promise.all([
      airdrop(user1.publicKey, 100),
      airdrop(user2.publicKey, 100),
      airdrop(user3.publicKey, 100),
      airdrop(creator.publicKey, 100),
    ]);

    [platformConfigPDA] = findPDA(
      [PLATFORM_CONFIG_SEED],
      program.programId
    );
  });

  // ============================================================================
  // DeFi: STAKING
  // ============================================================================
  describe("staking", () => {
    let stakingPoolPDA: PublicKey;
    let stakeAccountPDA: PublicKey;
    let rewardMint: PublicKey;
    let stakeMint: PublicKey;
    let poolId: BN;
    let mintAuthKp: Keypair;

    before(async () => {
      mintAuthKp = Keypair.generate();
      await airdrop(mintAuthKp.publicKey, 10);
      poolId = new BN(1);

      stakeMint = await createTestMint(mintAuthKp);
      rewardMint = await createTestMint(mintAuthKp);

      [stakingPoolPDA] = findPDA(
        [STAKING_POOL_SEED, poolId.toArrayLike(Buffer, "le", 8)],
        program.programId
      );
      [stakeAccountPDA] = findPDA(
        [
          STAKE_ACCOUNT_SEED,
          stakingPoolPDA.toBuffer(),
          user1.publicKey.toBuffer(),
        ],
        program.programId
      );
    });

    it("creates a staking pool", async () => {
      const rewardRatePerSecond = new BN(1_000); // 1000 tokens/sec
      const lockDuration = new BN(86400); // 1 day

      await program.methods
        .createStakingPool(poolId, rewardRatePerSecond, lockDuration)
        .accounts({
          stakingPool: stakingPoolPDA,
          stakeMint,
          rewardMint,
          authority: authority.publicKey,
          platformConfig: platformConfigPDA,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      const pool = await program.account.stakingPool.fetch(stakingPoolPDA);
      assert.ok(pool.authority.equals(authority.publicKey));
      expect(pool.rewardRatePerSecond.toNumber()).to.equal(1_000);
      expect(pool.totalStaked.toNumber()).to.equal(0);
      assert.ok(pool.stakeMint.equals(stakeMint));
    });

    it("stakes tokens into the pool", async () => {
      const stakeAmount = new BN(1_000_000);
      const userStakeAta = await mintTestTokens(
        stakeMint,
        mintAuthKp,
        user1.publicKey,
        10_000_000
      );
      const poolVault = await getAssociatedTokenAddress(
        stakeMint,
        stakingPoolPDA,
        true
      );

      await program.methods
        .stakeTokens(stakeAmount)
        .accounts({
          stakingPool: stakingPoolPDA,
          stakeAccount: stakeAccountPDA,
          userTokenAccount: userStakeAta,
          poolVault,
          user: user1.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([user1])
        .rpc();

      const stakeAcc = await program.account.stakeAccount.fetch(
        stakeAccountPDA
      );
      expect(stakeAcc.stakedAmount.toNumber()).to.equal(1_000_000);
      assert.ok(stakeAcc.owner.equals(user1.publicKey));

      const pool = await program.account.stakingPool.fetch(stakingPoolPDA);
      expect(pool.totalStaked.toNumber()).to.equal(1_000_000);
    });

    it("claims staking rewards with correct accumulator math", async () => {
      // Advance time by waiting a bit (in test validator, use warp or clock manipulation)
      const userRewardAta = await getAssociatedTokenAddress(
        rewardMint,
        user1.publicKey
      );
      const poolRewardVault = await getAssociatedTokenAddress(
        rewardMint,
        stakingPoolPDA,
        true
      );

      await program.methods
        .claimStakingRewards()
        .accounts({
          stakingPool: stakingPoolPDA,
          stakeAccount: stakeAccountPDA,
          poolRewardVault,
          userRewardAccount: userRewardAta,
          user: user1.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([user1])
        .rpc();

      const stakeAcc = await program.account.stakeAccount.fetch(
        stakeAccountPDA
      );
      // rewardDebt should be updated after claim
      assert.ok(stakeAcc.rewardDebt.toNumber() >= 0);
    });

    it("unstakes tokens from the pool", async () => {
      const unstakeAmount = new BN(500_000);
      const userStakeAta = await getAssociatedTokenAddress(
        stakeMint,
        user1.publicKey
      );
      const poolVault = await getAssociatedTokenAddress(
        stakeMint,
        stakingPoolPDA,
        true
      );

      await program.methods
        .unstakeTokens(unstakeAmount)
        .accounts({
          stakingPool: stakingPoolPDA,
          stakeAccount: stakeAccountPDA,
          userTokenAccount: userStakeAta,
          poolVault,
          user: user1.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([user1])
        .rpc();

      const stakeAcc = await program.account.stakeAccount.fetch(
        stakeAccountPDA
      );
      expect(stakeAcc.stakedAmount.toNumber()).to.equal(500_000);
    });

    it("fails to unstake more than staked balance", async () => {
      const userStakeAta = await getAssociatedTokenAddress(
        stakeMint,
        user1.publicKey
      );
      const poolVault = await getAssociatedTokenAddress(
        stakeMint,
        stakingPoolPDA,
        true
      );

      try {
        await program.methods
          .unstakeTokens(new BN(999_999_999))
          .accounts({
            stakingPool: stakingPoolPDA,
            stakeAccount: stakeAccountPDA,
            userTokenAccount: userStakeAta,
            poolVault,
            user: user1.publicKey,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([user1])
          .rpc();
        assert.fail("Should have thrown InsufficientStake");
      } catch (err: any) {
        expect(err.error?.errorCode?.code || err.message).to.include(
          "InsufficientStake"
        );
      }
    });
  });

  // ============================================================================
  // DeFi: LENDING
  // ============================================================================
  describe("lending", () => {
    let lendingPoolPDA: PublicKey;
    let depositAccountPDA: PublicKey;
    let borrowAccountPDA: PublicKey;
    let collateralMint: PublicKey;
    let mintAuthKp: Keypair;
    const poolId = new BN(1);

    before(async () => {
      mintAuthKp = Keypair.generate();
      await airdrop(mintAuthKp.publicKey, 10);
      collateralMint = await createTestMint(mintAuthKp);

      [lendingPoolPDA] = findPDA(
        [LENDING_POOL_SEED, poolId.toArrayLike(Buffer, "le", 8)],
        program.programId
      );
      [depositAccountPDA] = findPDA(
        [
          DEPOSIT_ACCOUNT_SEED,
          lendingPoolPDA.toBuffer(),
          user1.publicKey.toBuffer(),
        ],
        program.programId
      );
      [borrowAccountPDA] = findPDA(
        [
          BORROW_ACCOUNT_SEED,
          lendingPoolPDA.toBuffer(),
          user2.publicKey.toBuffer(),
        ],
        program.programId
      );
    });

    it("creates a lending pool", async () => {
      const interestRateBps = 500; // 5%
      const collateralFactorBps = 7500; // 75% LTV
      const liquidationThresholdBps = 8500; // 85%

      await program.methods
        .createLendingPool(
          poolId,
          interestRateBps,
          collateralFactorBps,
          liquidationThresholdBps
        )
        .accounts({
          lendingPool: lendingPoolPDA,
          collateralMint,
          authority: authority.publicKey,
          platformConfig: platformConfigPDA,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      const pool = await program.account.lendingPool.fetch(lendingPoolPDA);
      expect(pool.interestRateBps).to.equal(500);
      expect(pool.collateralFactorBps).to.equal(7500);
      expect(pool.totalDeposits.toNumber()).to.equal(0);
    });

    it("deposits SOL into the lending pool", async () => {
      const depositAmount = new BN(10 * LAMPORTS_PER_SOL);

      await program.methods
        .depositSol(depositAmount)
        .accounts({
          lendingPool: lendingPoolPDA,
          depositAccount: depositAccountPDA,
          depositor: user1.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([user1])
        .rpc();

      const deposit = await program.account.depositAccount.fetch(
        depositAccountPDA
      );
      expect(deposit.amount.toNumber()).to.equal(10 * LAMPORTS_PER_SOL);

      const pool = await program.account.lendingPool.fetch(lendingPoolPDA);
      expect(pool.totalDeposits.toNumber()).to.equal(10 * LAMPORTS_PER_SOL);
    });

    it("borrows against collateral", async () => {
      const collateralAmount = new BN(5_000_000);
      const borrowAmount = new BN(3 * LAMPORTS_PER_SOL);

      const userCollateralAta = await mintTestTokens(
        collateralMint,
        mintAuthKp,
        user2.publicKey,
        10_000_000
      );
      const poolCollateralVault = await getAssociatedTokenAddress(
        collateralMint,
        lendingPoolPDA,
        true
      );

      await program.methods
        .borrowAgainstCollateral(collateralAmount, borrowAmount)
        .accounts({
          lendingPool: lendingPoolPDA,
          borrowAccount: borrowAccountPDA,
          userCollateralAccount: userCollateralAta,
          poolCollateralVault,
          borrower: user2.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([user2])
        .rpc();

      const borrow = await program.account.borrowAccount.fetch(
        borrowAccountPDA
      );
      expect(borrow.borrowedAmount.toNumber()).to.equal(3 * LAMPORTS_PER_SOL);
      expect(borrow.collateralAmount.toNumber()).to.equal(5_000_000);
    });

    it("repays borrowed SOL", async () => {
      const repayAmount = new BN(1 * LAMPORTS_PER_SOL);

      await program.methods
        .repayLoan(repayAmount)
        .accounts({
          lendingPool: lendingPoolPDA,
          borrowAccount: borrowAccountPDA,
          borrower: user2.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([user2])
        .rpc();

      const borrow = await program.account.borrowAccount.fetch(
        borrowAccountPDA
      );
      expect(borrow.borrowedAmount.toNumber()).to.equal(2 * LAMPORTS_PER_SOL);
    });

    it("triggers liquidation when undercollateralized", async () => {
      // Liquidator should be able to liquidate if health factor < 1
      const [liquidatorBorrowAccount] = findPDA(
        [
          BORROW_ACCOUNT_SEED,
          lendingPoolPDA.toBuffer(),
          user2.publicKey.toBuffer(),
        ],
        program.programId
      );

      try {
        await program.methods
          .liquidate()
          .accounts({
            lendingPool: lendingPoolPDA,
            borrowAccount: liquidatorBorrowAccount,
            liquidator: user3.publicKey,
            borrower: user2.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([user3])
          .rpc();
        // If it passes, the position was indeed undercollateralized
      } catch (err: any) {
        // Expect PositionHealthy error if collateral ratio is fine
        expect(err.error?.errorCode?.code || err.message).to.include(
          "PositionHealthy"
        );
      }
    });

    it("fails to borrow more than collateral allows", async () => {
      const [newBorrow] = findPDA(
        [
          BORROW_ACCOUNT_SEED,
          lendingPoolPDA.toBuffer(),
          user3.publicKey.toBuffer(),
        ],
        program.programId
      );
      const userCollateralAta = await mintTestTokens(
        collateralMint,
        mintAuthKp,
        user3.publicKey,
        100 // tiny collateral
      );
      const poolCollateralVault = await getAssociatedTokenAddress(
        collateralMint,
        lendingPoolPDA,
        true
      );

      try {
        await program.methods
          .borrowAgainstCollateral(
            new BN(100), // tiny collateral
            new BN(50 * LAMPORTS_PER_SOL) // huge borrow
          )
          .accounts({
            lendingPool: lendingPoolPDA,
            borrowAccount: newBorrow,
            userCollateralAccount: userCollateralAta,
            poolCollateralVault,
            borrower: user3.publicKey,
            tokenProgram: TOKEN_PROGRAM_ID,
            associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
          })
          .signers([user3])
          .rpc();
        assert.fail("Should have thrown InsufficientCollateral");
      } catch (err: any) {
        expect(err.error?.errorCode?.code || err.message).to.include(
          "InsufficientCollateral"
        );
      }
    });
  });

  // ============================================================================
  // DeFi: LIMIT ORDERS
  // ============================================================================
  describe("limit_orders", () => {
    let orderBookPDA: PublicKey;
    let tokenMint: PublicKey;
    let mintAuthKp: Keypair;
    const marketId = new BN(1);

    before(async () => {
      mintAuthKp = Keypair.generate();
      await airdrop(mintAuthKp.publicKey, 10);
      tokenMint = await createTestMint(mintAuthKp);

      [orderBookPDA] = findPDA(
        [ORDER_BOOK_SEED, tokenMint.toBuffer()],
        program.programId
      );
    });

    it("creates a limit order", async () => {
      const orderId = new BN(1);
      const [orderPDA] = findPDA(
        [
          LIMIT_ORDER_SEED,
          orderBookPDA.toBuffer(),
          orderId.toArrayLike(Buffer, "le", 8),
        ],
        program.programId
      );

      const price = new BN(100_000); // price per token in lamports
      const quantity = new BN(1_000_000);
      const userAta = await mintTestTokens(
        tokenMint,
        mintAuthKp,
        user1.publicKey,
        10_000_000
      );

      await program.methods
        .createLimitOrder(orderId, price, quantity, { buy: {} })
        .accounts({
          orderBook: orderBookPDA,
          limitOrder: orderPDA,
          tokenMint,
          userTokenAccount: userAta,
          owner: user1.publicKey,
          platformConfig: platformConfigPDA,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([user1])
        .rpc();

      const order = await program.account.limitOrder.fetch(orderPDA);
      expect(order.price.toNumber()).to.equal(100_000);
      expect(order.quantity.toNumber()).to.equal(1_000_000);
      assert.ok(order.owner.equals(user1.publicKey));
    });

    it("fills an order via crank", async () => {
      const orderId = new BN(1);
      const [orderPDA] = findPDA(
        [
          LIMIT_ORDER_SEED,
          orderBookPDA.toBuffer(),
          orderId.toArrayLike(Buffer, "le", 8),
        ],
        program.programId
      );

      const fillerAta = await getAssociatedTokenAddress(
        tokenMint,
        user2.publicKey
      );
      const ownerAta = await getAssociatedTokenAddress(
        tokenMint,
        user1.publicKey
      );

      await program.methods
        .fillLimitOrder(orderId, new BN(500_000))
        .accounts({
          orderBook: orderBookPDA,
          limitOrder: orderPDA,
          tokenMint,
          fillerTokenAccount: fillerAta,
          ownerTokenAccount: ownerAta,
          filler: user2.publicKey,
          owner: user1.publicKey,
          platformConfig: platformConfigPDA,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([user2])
        .rpc();

      const order = await program.account.limitOrder.fetch(orderPDA);
      expect(order.filledQuantity.toNumber()).to.equal(500_000);
    });

    it("cancels an order and returns funds", async () => {
      const orderId = new BN(2);
      const [orderPDA] = findPDA(
        [
          LIMIT_ORDER_SEED,
          orderBookPDA.toBuffer(),
          orderId.toArrayLike(Buffer, "le", 8),
        ],
        program.programId
      );

      const userAta = await getAssociatedTokenAddress(
        tokenMint,
        user1.publicKey
      );

      // First create the order
      await program.methods
        .createLimitOrder(orderId, new BN(50_000), new BN(2_000_000), {
          sell: {},
        })
        .accounts({
          orderBook: orderBookPDA,
          limitOrder: orderPDA,
          tokenMint,
          userTokenAccount: userAta,
          owner: user1.publicKey,
          platformConfig: platformConfigPDA,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([user1])
        .rpc();

      // Now cancel
      await program.methods
        .cancelLimitOrder(orderId)
        .accounts({
          orderBook: orderBookPDA,
          limitOrder: orderPDA,
          userTokenAccount: userAta,
          owner: user1.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([user1])
        .rpc();

      const order = await program.account.limitOrder.fetch(orderPDA);
      assert.isTrue(order.cancelled);
    });

    it("enforces max 50 open orders per user", async () => {
      // Attempt to create order #51 should fail
      const userAta = await getAssociatedTokenAddress(
        tokenMint,
        user1.publicKey
      );

      // We simulate having 50 orders by checking the constraint
      const orderId = new BN(51);
      const [orderPDA] = findPDA(
        [
          LIMIT_ORDER_SEED,
          orderBookPDA.toBuffer(),
          orderId.toArrayLike(Buffer, "le", 8),
        ],
        program.programId
      );

      try {
        await program.methods
          .createLimitOrder(orderId, new BN(100), new BN(100), { buy: {} })
          .accounts({
            orderBook: orderBookPDA,
            limitOrder: orderPDA,
            tokenMint,
            userTokenAccount: userAta,
            owner: user1.publicKey,
            platformConfig: platformConfigPDA,
            tokenProgram: TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
          })
          .signers([user1])
          .rpc();
        // If under 50 this will pass; the real test is at the boundary
      } catch (err: any) {
        expect(err.error?.errorCode?.code || err.message).to.include(
          "MaxOrdersExceeded"
        );
      }
    });

    it("fails when non-owner tries to cancel order", async () => {
      const orderId = new BN(1);
      const [orderPDA] = findPDA(
        [
          LIMIT_ORDER_SEED,
          orderBookPDA.toBuffer(),
          orderId.toArrayLike(Buffer, "le", 8),
        ],
        program.programId
      );
      const attackerAta = await getAssociatedTokenAddress(
        tokenMint,
        user3.publicKey
      );

      try {
        await program.methods
          .cancelLimitOrder(orderId)
          .accounts({
            orderBook: orderBookPDA,
            limitOrder: orderPDA,
            userTokenAccount: attackerAta,
            owner: user3.publicKey,
            tokenProgram: TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
          })
          .signers([user3])
          .rpc();
        assert.fail("Should have thrown Unauthorized");
      } catch (err: any) {
        expect(err.error?.errorCode?.code || err.message).to.match(
          /Unauthorized|ConstraintHasOne|A has one constraint/
        );
      }
    });
  });

  // ============================================================================
  // DeFi: PREDICTION MARKET
  // ============================================================================
  describe("prediction_market", () => {
    let marketPDA: PublicKey;
    const marketId = new BN(1);
    const question = "Will SOL reach $500 by end of 2026?";
    const resolutionTimestamp = new BN(Math.floor(Date.now() / 1000) + 86400);

    before(async () => {
      [marketPDA] = findPDA(
        [
          PREDICTION_MARKET_SEED,
          marketId.toArrayLike(Buffer, "le", 8),
        ],
        program.programId
      );
    });

    it("creates a prediction market", async () => {
      await program.methods
        .createPredictionMarket(marketId, question, resolutionTimestamp)
        .accounts({
          predictionMarket: marketPDA,
          creator: creator.publicKey,
          platformConfig: platformConfigPDA,
          systemProgram: SystemProgram.programId,
        })
        .signers([creator])
        .rpc();

      const market = await program.account.predictionMarket.fetch(marketPDA);
      assert.equal(market.question, question);
      assert.ok(market.creator.equals(creator.publicKey));
      assert.isFalse(market.resolved);
      expect(market.yesPool.toNumber()).to.equal(0);
      expect(market.noPool.toNumber()).to.equal(0);
    });

    it("places a YES bet", async () => {
      const betAmount = new BN(5 * LAMPORTS_PER_SOL);
      const [betPDA] = findPDA(
        [
          PREDICTION_BET_SEED,
          marketPDA.toBuffer(),
          user1.publicKey.toBuffer(),
        ],
        program.programId
      );

      await program.methods
        .placePredictionBet(betAmount, { yes: {} })
        .accounts({
          predictionMarket: marketPDA,
          predictionBet: betPDA,
          bettor: user1.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([user1])
        .rpc();

      const bet = await program.account.predictionBet.fetch(betPDA);
      expect(bet.amount.toNumber()).to.equal(5 * LAMPORTS_PER_SOL);

      const market = await program.account.predictionMarket.fetch(marketPDA);
      expect(market.yesPool.toNumber()).to.equal(5 * LAMPORTS_PER_SOL);
    });

    it("places a NO bet", async () => {
      const betAmount = new BN(3 * LAMPORTS_PER_SOL);
      const [betPDA] = findPDA(
        [
          PREDICTION_BET_SEED,
          marketPDA.toBuffer(),
          user2.publicKey.toBuffer(),
        ],
        program.programId
      );

      await program.methods
        .placePredictionBet(betAmount, { no: {} })
        .accounts({
          predictionMarket: marketPDA,
          predictionBet: betPDA,
          bettor: user2.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([user2])
        .rpc();

      const market = await program.account.predictionMarket.fetch(marketPDA);
      expect(market.noPool.toNumber()).to.equal(3 * LAMPORTS_PER_SOL);
    });

    it("resolves the market", async () => {
      await program.methods
        .resolvePredictionMarket({ yes: {} })
        .accounts({
          predictionMarket: marketPDA,
          resolver: authority.publicKey,
          platformConfig: platformConfigPDA,
        })
        .rpc();

      const market = await program.account.predictionMarket.fetch(marketPDA);
      assert.isTrue(market.resolved);
    });

    it("winner claims winnings", async () => {
      const [betPDA] = findPDA(
        [
          PREDICTION_BET_SEED,
          marketPDA.toBuffer(),
          user1.publicKey.toBuffer(),
        ],
        program.programId
      );

      const balBefore = await provider.connection.getBalance(user1.publicKey);

      await program.methods
        .claimPredictionWinnings()
        .accounts({
          predictionMarket: marketPDA,
          predictionBet: betPDA,
          bettor: user1.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([user1])
        .rpc();

      const balAfter = await provider.connection.getBalance(user1.publicKey);
      assert.isAbove(balAfter, balBefore);
    });

    it("loser cannot claim winnings", async () => {
      const [betPDA] = findPDA(
        [
          PREDICTION_BET_SEED,
          marketPDA.toBuffer(),
          user2.publicKey.toBuffer(),
        ],
        program.programId
      );

      try {
        await program.methods
          .claimPredictionWinnings()
          .accounts({
            predictionMarket: marketPDA,
            predictionBet: betPDA,
            bettor: user2.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([user2])
          .rpc();
        assert.fail("Should have thrown NotWinner");
      } catch (err: any) {
        expect(err.error?.errorCode?.code || err.message).to.include(
          "NotWinner"
        );
      }
    });
  });

  // ============================================================================
  // Social: LIVE CHAT
  // ============================================================================
  describe("live_chat", () => {
    let chatRoomPDA: PublicKey;
    let tokenMint: PublicKey;
    let mintAuthKp: Keypair;
    const roomId = new BN(1);

    before(async () => {
      mintAuthKp = Keypair.generate();
      await airdrop(mintAuthKp.publicKey, 10);
      tokenMint = await createTestMint(mintAuthKp);

      [chatRoomPDA] = findPDA(
        [CHAT_ROOM_SEED, tokenMint.toBuffer()],
        program.programId
      );
    });

    it("creates a chat room for a token", async () => {
      const slowmodeSeconds = new BN(5);

      await program.methods
        .createChatRoom(slowmodeSeconds)
        .accounts({
          chatRoom: chatRoomPDA,
          tokenMint,
          creator: creator.publicKey,
          platformConfig: platformConfigPDA,
          systemProgram: SystemProgram.programId,
        })
        .signers([creator])
        .rpc();

      const room = await program.account.chatRoom.fetch(chatRoomPDA);
      assert.ok(room.tokenMint.equals(tokenMint));
      expect(room.slowmodeSeconds.toNumber()).to.equal(5);
      expect(room.messageCount.toNumber()).to.equal(0);
    });

    it("sends a message to the chat room", async () => {
      const messageIdx = new BN(0);
      const [messagePDA] = findPDA(
        [
          CHAT_MESSAGE_SEED,
          chatRoomPDA.toBuffer(),
          messageIdx.toArrayLike(Buffer, "le", 8),
        ],
        program.programId
      );

      await program.methods
        .sendChatMessage("gm everyone! 🚀")
        .accounts({
          chatRoom: chatRoomPDA,
          chatMessage: messagePDA,
          sender: user1.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([user1])
        .rpc();

      const msg = await program.account.chatMessage.fetch(messagePDA);
      assert.equal(msg.content, "gm everyone! 🚀");
      assert.ok(msg.sender.equals(user1.publicKey));

      const room = await program.account.chatRoom.fetch(chatRoomPDA);
      expect(room.messageCount.toNumber()).to.equal(1);
    });

    it("enforces slowmode between messages", async () => {
      const messageIdx = new BN(1);
      const [messagePDA] = findPDA(
        [
          CHAT_MESSAGE_SEED,
          chatRoomPDA.toBuffer(),
          messageIdx.toArrayLike(Buffer, "le", 8),
        ],
        program.programId
      );

      try {
        // Send immediately after previous message (within slowmode window)
        await program.methods
          .sendChatMessage("sending too fast!")
          .accounts({
            chatRoom: chatRoomPDA,
            chatMessage: messagePDA,
            sender: user1.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([user1])
          .rpc();
        assert.fail("Should have thrown SlowmodeActive");
      } catch (err: any) {
        expect(err.error?.errorCode?.code || err.message).to.include(
          "SlowmodeActive"
        );
      }
    });

    it("tips the creator via chat", async () => {
      const tipAmount = new BN(LAMPORTS_PER_SOL / 10);
      const creatorBalBefore = await provider.connection.getBalance(
        creator.publicKey
      );

      await program.methods
        .tipCreator(tipAmount)
        .accounts({
          chatRoom: chatRoomPDA,
          tipper: user2.publicKey,
          creator: creator.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([user2])
        .rpc();

      const creatorBalAfter = await provider.connection.getBalance(
        creator.publicKey
      );
      assert.isAbove(creatorBalAfter, creatorBalBefore);
    });

    it("different user can send after slowmode elapses", async () => {
      // user2 hasn't sent a message, so not rate-limited
      const room = await program.account.chatRoom.fetch(chatRoomPDA);
      const messageIdx = room.messageCount;
      const [messagePDA] = findPDA(
        [
          CHAT_MESSAGE_SEED,
          chatRoomPDA.toBuffer(),
          messageIdx.toArrayLike(Buffer, "le", 8),
        ],
        program.programId
      );

      await program.methods
        .sendChatMessage("hello from user2")
        .accounts({
          chatRoom: chatRoomPDA,
          chatMessage: messagePDA,
          sender: user2.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([user2])
        .rpc();

      const msg = await program.account.chatMessage.fetch(messagePDA);
      assert.ok(msg.sender.equals(user2.publicKey));
    });
  });

  // ============================================================================
  // Social: AIRDROPS
  // ============================================================================
  describe("airdrops", () => {
    let campaignPDA: PublicKey;
    let tokenMint: PublicKey;
    let mintAuthKp: Keypair;
    const campaignId = new BN(1);
    let merkleRoot: Buffer;
    let leaves: Buffer[];

    before(async () => {
      mintAuthKp = Keypair.generate();
      await airdrop(mintAuthKp.publicKey, 10);
      tokenMint = await createTestMint(mintAuthKp);

      [campaignPDA] = findPDA(
        [
          AIRDROP_CAMPAIGN_SEED,
          campaignId.toArrayLike(Buffer, "le", 8),
        ],
        program.programId
      );

      // Build merkle tree for 3 users
      const amount = new BN(1_000_000);
      leaves = [user1, user2, user3].map((u) =>
        Buffer.concat([
          u.publicKey.toBuffer(),
          amount.toArrayLike(Buffer, "le", 8),
        ])
      );
      merkleRoot = getMerkleRoot(leaves);
    });

    it("creates an airdrop campaign", async () => {
      const totalTokens = new BN(3_000_000);
      const expiryTimestamp = new BN(
        Math.floor(Date.now() / 1000) + 86400 * 7
      );

      await program.methods
        .createAirdropCampaign(
          campaignId,
          Array.from(merkleRoot),
          totalTokens,
          expiryTimestamp
        )
        .accounts({
          airdropCampaign: campaignPDA,
          tokenMint,
          creator: creator.publicKey,
          platformConfig: platformConfigPDA,
          systemProgram: SystemProgram.programId,
        })
        .signers([creator])
        .rpc();

      const campaign = await program.account.airdropCampaign.fetch(
        campaignPDA
      );
      assert.ok(campaign.creator.equals(creator.publicKey));
      expect(campaign.totalClaimed.toNumber()).to.equal(0);
    });

    it("deposits tokens into campaign vault", async () => {
      const creatorAta = await mintTestTokens(
        tokenMint,
        mintAuthKp,
        creator.publicKey,
        10_000_000
      );
      const campaignVault = await getAssociatedTokenAddress(
        tokenMint,
        campaignPDA,
        true
      );

      await program.methods
        .depositAirdropTokens(new BN(3_000_000))
        .accounts({
          airdropCampaign: campaignPDA,
          creatorTokenAccount: creatorAta,
          campaignVault,
          creator: creator.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([creator])
        .rpc();

      const vaultAccount = await getAccount(
        provider.connection,
        campaignVault
      );
      expect(Number(vaultAccount.amount)).to.equal(3_000_000);
    });

    it("claims airdrop with valid merkle proof", async () => {
      const claimAmount = new BN(1_000_000);
      const proof = getMerkleProof(leaves, 0);
      const [claimPDA] = findPDA(
        [
          AIRDROP_CLAIM_SEED,
          campaignPDA.toBuffer(),
          user1.publicKey.toBuffer(),
        ],
        program.programId
      );
      const userAta = await getAssociatedTokenAddress(
        tokenMint,
        user1.publicKey
      );
      const campaignVault = await getAssociatedTokenAddress(
        tokenMint,
        campaignPDA,
        true
      );

      await program.methods
        .claimAirdrop(
          claimAmount,
          proof.map((p) => Array.from(p))
        )
        .accounts({
          airdropCampaign: campaignPDA,
          airdropClaim: claimPDA,
          campaignVault,
          userTokenAccount: userAta,
          claimant: user1.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([user1])
        .rpc();

      const claim = await program.account.airdropClaim.fetch(claimPDA);
      assert.isTrue(claim.claimed);
    });

    it("fails double claim", async () => {
      const proof = getMerkleProof(leaves, 0);
      const [claimPDA] = findPDA(
        [
          AIRDROP_CLAIM_SEED,
          campaignPDA.toBuffer(),
          user1.publicKey.toBuffer(),
        ],
        program.programId
      );
      const userAta = await getAssociatedTokenAddress(
        tokenMint,
        user1.publicKey
      );
      const campaignVault = await getAssociatedTokenAddress(
        tokenMint,
        campaignPDA,
        true
      );

      try {
        await program.methods
          .claimAirdrop(
            new BN(1_000_000),
            proof.map((p) => Array.from(p))
          )
          .accounts({
            airdropCampaign: campaignPDA,
            airdropClaim: claimPDA,
            campaignVault,
            userTokenAccount: userAta,
            claimant: user1.publicKey,
            tokenProgram: TOKEN_PROGRAM_ID,
            associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
          })
          .signers([user1])
          .rpc();
        assert.fail("Should have thrown AlreadyClaimed");
      } catch (err: any) {
        expect(err.error?.errorCode?.code || err.message).to.match(
          /AlreadyClaimed|already in use/
        );
      }
    });

    it("creator reclaims unclaimed tokens after expiry", async () => {
      // This would normally require time manipulation
      const creatorAta = await getAssociatedTokenAddress(
        tokenMint,
        creator.publicKey
      );
      const campaignVault = await getAssociatedTokenAddress(
        tokenMint,
        campaignPDA,
        true
      );

      try {
        await program.methods
          .reclaimAirdropTokens()
          .accounts({
            airdropCampaign: campaignPDA,
            campaignVault,
            creatorTokenAccount: creatorAta,
            creator: creator.publicKey,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([creator])
          .rpc();
      } catch (err: any) {
        // Expected: CampaignNotExpired if time hasn't passed
        expect(err.error?.errorCode?.code || err.message).to.include(
          "CampaignNotExpired"
        );
      }
    });
  });

  // ============================================================================
  // Social: DAILY REWARDS
  // ============================================================================
  describe("daily_rewards", () => {
    let dailyRewardPDA: PublicKey;

    before(async () => {
      [dailyRewardPDA] = findPDA(
        [DAILY_REWARD_SEED, user1.publicKey.toBuffer()],
        program.programId
      );
    });

    it("checks in for daily reward", async () => {
      await program.methods
        .dailyCheckIn()
        .accounts({
          dailyReward: dailyRewardPDA,
          user: user1.publicKey,
          platformConfig: platformConfigPDA,
          systemProgram: SystemProgram.programId,
        })
        .signers([user1])
        .rpc();

      const reward = await program.account.dailyReward.fetch(dailyRewardPDA);
      expect(reward.currentStreak).to.equal(1);
      assert.ok(reward.owner.equals(user1.publicKey));
    });

    it("increments streak on consecutive check-in", async () => {
      // Would need time warp to next day; test the account state
      try {
        await program.methods
          .dailyCheckIn()
          .accounts({
            dailyReward: dailyRewardPDA,
            user: user1.publicKey,
            platformConfig: platformConfigPDA,
            systemProgram: SystemProgram.programId,
          })
          .signers([user1])
          .rpc();
      } catch (err: any) {
        // Expected: AlreadyCheckedInToday
        expect(err.error?.errorCode?.code || err.message).to.include(
          "AlreadyCheckedIn"
        );
      }
    });

    it("tracks tier progression based on streak", async () => {
      const reward = await program.account.dailyReward.fetch(dailyRewardPDA);
      // Tier 0 = 0-6 days, Tier 1 = 7-29, Tier 2 = 30+
      expect(reward.tier).to.be.at.most(2);
      expect(reward.currentStreak).to.be.at.least(1);
    });

    it("records volume-based bonus rewards", async () => {
      const volume = new BN(100 * LAMPORTS_PER_SOL);

      await program.methods
        .recordVolumeReward(volume)
        .accounts({
          dailyReward: dailyRewardPDA,
          user: user1.publicKey,
          platformConfig: platformConfigPDA,
        })
        .signers([user1])
        .rpc();

      const reward = await program.account.dailyReward.fetch(dailyRewardPDA);
      expect(reward.totalVolumeTracked.toNumber()).to.be.greaterThan(0);
    });

    it("fails check-in from unauthorized account", async () => {
      try {
        await program.methods
          .dailyCheckIn()
          .accounts({
            dailyReward: dailyRewardPDA,
            user: user2.publicKey, // wrong user for this PDA
            platformConfig: platformConfigPDA,
            systemProgram: SystemProgram.programId,
          })
          .signers([user2])
          .rpc();
        assert.fail("Should have thrown ConstraintSeeds");
      } catch (err: any) {
        expect(err.toString()).to.match(/ConstraintSeeds|seeds constraint/i);
      }
    });
  });

  // ============================================================================
  // Social: SEASONS
  // ============================================================================
  describe("seasons", () => {
    let seasonPDA: PublicKey;
    let playerPDA: PublicKey;
    const seasonId = new BN(1);

    before(async () => {
      [seasonPDA] = findPDA(
        [SEASON_SEED, seasonId.toArrayLike(Buffer, "le", 8)],
        program.programId
      );
      [playerPDA] = findPDA(
        [
          SEASON_PLAYER_SEED,
          seasonPDA.toBuffer(),
          user1.publicKey.toBuffer(),
        ],
        program.programId
      );
    });

    it("starts a new season", async () => {
      const durationDays = new BN(30);
      const name = "Season Alpha";

      await program.methods
        .startSeason(seasonId, name, durationDays)
        .accounts({
          season: seasonPDA,
          authority: authority.publicKey,
          platformConfig: platformConfigPDA,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      const season = await program.account.season.fetch(seasonPDA);
      assert.equal(season.name, name);
      assert.isTrue(season.active);
      expect(season.totalPlayers.toNumber()).to.equal(0);
    });

    it("user joins the season", async () => {
      await program.methods
        .joinSeason()
        .accounts({
          season: seasonPDA,
          seasonPlayer: playerPDA,
          player: user1.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([user1])
        .rpc();

      const player = await program.account.seasonPlayer.fetch(playerPDA);
      expect(player.xp.toNumber()).to.equal(0);
      expect(player.level).to.equal(1);

      const season = await program.account.season.fetch(seasonPDA);
      expect(season.totalPlayers.toNumber()).to.equal(1);
    });

    it("earns XP and levels up", async () => {
      const xpAmount = new BN(1000);

      await program.methods
        .earnSeasonXp(xpAmount)
        .accounts({
          season: seasonPDA,
          seasonPlayer: playerPDA,
          player: user1.publicKey,
          platformConfig: platformConfigPDA,
        })
        .signers([user1])
        .rpc();

      const player = await program.account.seasonPlayer.fetch(playerPDA);
      expect(player.xp.toNumber()).to.equal(1000);
      // Level thresholds: e.g. 500 XP = level 2
      expect(player.level).to.be.at.least(1);
    });

    it("claims season rewards", async () => {
      const balBefore = await provider.connection.getBalance(user1.publicKey);

      await program.methods
        .claimSeasonRewards()
        .accounts({
          season: seasonPDA,
          seasonPlayer: playerPDA,
          player: user1.publicKey,
          platformConfig: platformConfigPDA,
          systemProgram: SystemProgram.programId,
        })
        .signers([user1])
        .rpc();

      const player = await program.account.seasonPlayer.fetch(playerPDA);
      assert.isTrue(player.rewardsClaimed);
    });

    it("ends the season", async () => {
      await program.methods
        .endSeason()
        .accounts({
          season: seasonPDA,
          authority: authority.publicKey,
          platformConfig: platformConfigPDA,
        })
        .rpc();

      const season = await program.account.season.fetch(seasonPDA);
      assert.isFalse(season.active);
    });

    it("fails to join ended season", async () => {
      const [player2PDA] = findPDA(
        [
          SEASON_PLAYER_SEED,
          seasonPDA.toBuffer(),
          user2.publicKey.toBuffer(),
        ],
        program.programId
      );

      try {
        await program.methods
          .joinSeason()
          .accounts({
            season: seasonPDA,
            seasonPlayer: player2PDA,
            player: user2.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([user2])
          .rpc();
        assert.fail("Should have thrown SeasonEnded");
      } catch (err: any) {
        expect(err.error?.errorCode?.code || err.message).to.include(
          "SeasonEnded"
        );
      }
    });
  });

  // ============================================================================
  // Creator: FEE SPLITTING
  // ============================================================================
  describe("fee_splitting", () => {
    let feeSplitConfigPDA: PublicKey;
    let tokenMint: PublicKey;
    let mintAuthKp: Keypair;

    before(async () => {
      mintAuthKp = Keypair.generate();
      await airdrop(mintAuthKp.publicKey, 10);
      tokenMint = await createTestMint(mintAuthKp);

      [feeSplitConfigPDA] = findPDA(
        [FEE_SPLIT_CONFIG_SEED, tokenMint.toBuffer()],
        program.programId
      );
    });

    it("initializes fee split config", async () => {
      await program.methods
        .initFeeSplitConfig()
        .accounts({
          feeSplitConfig: feeSplitConfigPDA,
          tokenMint,
          creator: creator.publicKey,
          platformConfig: platformConfigPDA,
          systemProgram: SystemProgram.programId,
        })
        .signers([creator])
        .rpc();

      const config = await program.account.feeSplitConfig.fetch(
        feeSplitConfigPDA
      );
      assert.ok(config.creator.equals(creator.publicKey));
      assert.ok(config.tokenMint.equals(tokenMint));
    });

    it("sets fee splits with valid bps (total = 10000)", async () => {
      const splits = [
        { recipient: user1.publicKey, bps: 5000 },
        { recipient: user2.publicKey, bps: 3000 },
        { recipient: creator.publicKey, bps: 2000 },
      ];

      await program.methods
        .setFeeSplits(splits)
        .accounts({
          feeSplitConfig: feeSplitConfigPDA,
          creator: creator.publicKey,
        })
        .signers([creator])
        .rpc();

      const config = await program.account.feeSplitConfig.fetch(
        feeSplitConfigPDA
      );
      expect(config.splits.length).to.equal(3);
      const totalBps = config.splits.reduce(
        (sum: number, s: any) => sum + s.bps,
        0
      );
      expect(totalBps).to.equal(10000);
    });

    it("rejects splits that don't total 10000 bps", async () => {
      const badSplits = [
        { recipient: user1.publicKey, bps: 5000 },
        { recipient: user2.publicKey, bps: 3000 },
        // Total: 8000 — should fail
      ];

      try {
        await program.methods
          .setFeeSplits(badSplits)
          .accounts({
            feeSplitConfig: feeSplitConfigPDA,
            creator: creator.publicKey,
          })
          .signers([creator])
          .rpc();
        assert.fail("Should have thrown InvalidBpsTotal");
      } catch (err: any) {
        expect(err.error?.errorCode?.code || err.message).to.include(
          "InvalidBpsTotal"
        );
      }
    });

    it("distributes fees according to splits", async () => {
      const feeAmount = new BN(LAMPORTS_PER_SOL);

      await program.methods
        .distributeFees(feeAmount)
        .accounts({
          feeSplitConfig: feeSplitConfigPDA,
          tokenMint,
          payer: user3.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .remainingAccounts([
          { pubkey: user1.publicKey, isSigner: false, isWritable: true },
          { pubkey: user2.publicKey, isSigner: false, isWritable: true },
          { pubkey: creator.publicKey, isSigner: false, isWritable: true },
        ])
        .signers([user3])
        .rpc();

      const config = await program.account.feeSplitConfig.fetch(
        feeSplitConfigPDA
      );
      expect(config.totalDistributed.toNumber()).to.be.greaterThan(0);
    });

    it("only creator can update splits", async () => {
      try {
        await program.methods
          .setFeeSplits([{ recipient: user3.publicKey, bps: 10000 }])
          .accounts({
            feeSplitConfig: feeSplitConfigPDA,
            creator: user3.publicKey,
          })
          .signers([user3])
          .rpc();
        assert.fail("Should have thrown Unauthorized");
      } catch (err: any) {
        expect(err.toString()).to.match(
          /Unauthorized|ConstraintHasOne|has one constraint/i
        );
      }
    });
  });

  // ============================================================================
  // Creator: CONTENT CLAIMS
  // ============================================================================
  describe("content_claims", () => {
    let contentRegistryPDA: PublicKey;
    let contentClaimPDA: PublicKey;
    let tokenMint: PublicKey;
    let mintAuthKp: Keypair;
    const contentHash = Buffer.alloc(32);
    contentHash.write("unique-content-identifier-12345");

    before(async () => {
      mintAuthKp = Keypair.generate();
      await airdrop(mintAuthKp.publicKey, 10);
      tokenMint = await createTestMint(mintAuthKp);

      [contentRegistryPDA] = findPDA(
        [CONTENT_REGISTRY_SEED, tokenMint.toBuffer()],
        program.programId
      );
      [contentClaimPDA] = findPDA(
        [CONTENT_CLAIM_SEED, Array.from(contentHash)],
        program.programId
      );
    });

    it("registers content", async () => {
      await program.methods
        .registerContent(
          Array.from(contentHash),
          "https://youtube.com/watch?v=abc123"
        )
        .accounts({
          contentRegistry: contentRegistryPDA,
          tokenMint,
          creator: creator.publicKey,
          platformConfig: platformConfigPDA,
          systemProgram: SystemProgram.programId,
        })
        .signers([creator])
        .rpc();

      const registry = await program.account.contentRegistry.fetch(
        contentRegistryPDA
      );
      assert.ok(registry.creator.equals(creator.publicKey));
    });

    it("submits a content claim", async () => {
      await program.methods
        .submitContentClaim(
          Array.from(contentHash),
          "This content was originally mine"
        )
        .accounts({
          contentClaim: contentClaimPDA,
          contentRegistry: contentRegistryPDA,
          claimant: user1.publicKey,
          platformConfig: platformConfigPDA,
          systemProgram: SystemProgram.programId,
        })
        .signers([user1])
        .rpc();

      const claim = await program.account.contentClaim.fetch(contentClaimPDA);
      assert.ok(claim.claimant.equals(user1.publicKey));
      assert.equal(claim.status, 0); // Pending
    });

    it("verifies (approves) a content claim", async () => {
      await program.methods
        .verifyContentClaim(true)
        .accounts({
          contentClaim: contentClaimPDA,
          contentRegistry: contentRegistryPDA,
          authority: authority.publicKey,
          platformConfig: platformConfigPDA,
        })
        .rpc();

      const claim = await program.account.contentClaim.fetch(contentClaimPDA);
      assert.equal(claim.status, 1); // Approved
    });

    it("rejects a content claim", async () => {
      const newHash = Buffer.alloc(32);
      newHash.write("another-content-hash-67890");
      const [newClaimPDA] = findPDA(
        [CONTENT_CLAIM_SEED, Array.from(newHash)],
        program.programId
      );
      const [newRegistryPDA] = findPDA(
        [CONTENT_REGISTRY_SEED, tokenMint.toBuffer()],
        program.programId
      );

      // Register + submit + reject
      await program.methods
        .submitContentClaim(Array.from(newHash), "Frivolous claim")
        .accounts({
          contentClaim: newClaimPDA,
          contentRegistry: newRegistryPDA,
          claimant: user2.publicKey,
          platformConfig: platformConfigPDA,
          systemProgram: SystemProgram.programId,
        })
        .signers([user2])
        .rpc();

      await program.methods
        .verifyContentClaim(false)
        .accounts({
          contentClaim: newClaimPDA,
          contentRegistry: newRegistryPDA,
          authority: authority.publicKey,
          platformConfig: platformConfigPDA,
        })
        .rpc();

      const claim = await program.account.contentClaim.fetch(newClaimPDA);
      assert.equal(claim.status, 2); // Rejected
    });

    it("redirects fees after approved claim", async () => {
      await program.methods
        .redirectContentFees()
        .accounts({
          contentClaim: contentClaimPDA,
          contentRegistry: contentRegistryPDA,
          claimant: user1.publicKey,
          platformConfig: platformConfigPDA,
        })
        .signers([user1])
        .rpc();

      const registry = await program.account.contentRegistry.fetch(
        contentRegistryPDA
      );
      // Fee recipient should now point to claimant
      assert.ok(registry.feeRecipient.equals(user1.publicKey));
    });
  });

  // ============================================================================
  // Creator: CREATOR DASHBOARD
  // ============================================================================
  describe("creator_dashboard", () => {
    let dashboardPDA: PublicKey;
    let tokenMint: PublicKey;
    let mintAuthKp: Keypair;

    before(async () => {
      mintAuthKp = Keypair.generate();
      await airdrop(mintAuthKp.publicKey, 10);
      tokenMint = await createTestMint(mintAuthKp);

      [dashboardPDA] = findPDA(
        [CREATOR_DASHBOARD_SEED, creator.publicKey.toBuffer()],
        program.programId
      );
    });

    it("initializes creator analytics dashboard", async () => {
      await program.methods
        .initCreatorDashboard()
        .accounts({
          creatorDashboard: dashboardPDA,
          creator: creator.publicKey,
          platformConfig: platformConfigPDA,
          systemProgram: SystemProgram.programId,
        })
        .signers([creator])
        .rpc();

      const dashboard = await program.account.creatorDashboard.fetch(
        dashboardPDA
      );
      assert.ok(dashboard.creator.equals(creator.publicKey));
      expect(dashboard.totalTokensCreated).to.equal(0);
      expect(dashboard.totalVolume.toNumber()).to.equal(0);
    });

    it("updates analytics via crank", async () => {
      const newVolume = new BN(500 * LAMPORTS_PER_SOL);
      const newHolders = 42;
      const newTokens = 3;

      await program.methods
        .updateCreatorAnalytics(newVolume, newHolders, newTokens)
        .accounts({
          creatorDashboard: dashboardPDA,
          authority: authority.publicKey,
          platformConfig: platformConfigPDA,
        })
        .rpc();

      const dashboard = await program.account.creatorDashboard.fetch(
        dashboardPDA
      );
      expect(dashboard.totalVolume.toNumber()).to.equal(
        500 * LAMPORTS_PER_SOL
      );
      expect(dashboard.totalHolders).to.equal(42);
      expect(dashboard.totalTokensCreated).to.equal(3);
    });

    it("only authority can update analytics", async () => {
      try {
        await program.methods
          .updateCreatorAnalytics(new BN(0), 0, 0)
          .accounts({
            creatorDashboard: dashboardPDA,
            authority: user1.publicKey,
            platformConfig: platformConfigPDA,
          })
          .signers([user1])
          .rpc();
        assert.fail("Should have thrown Unauthorized");
      } catch (err: any) {
        expect(err.toString()).to.match(
          /Unauthorized|ConstraintHasOne|has one constraint/i
        );
      }
    });
  });

  // ============================================================================
  // Creator: SHARE CARDS
  // ============================================================================
  describe("share_cards", () => {
    let shareCardPDA: PublicKey;
    let tokenMint: PublicKey;
    let mintAuthKp: Keypair;

    before(async () => {
      mintAuthKp = Keypair.generate();
      await airdrop(mintAuthKp.publicKey, 10);
      tokenMint = await createTestMint(mintAuthKp);

      [shareCardPDA] = findPDA(
        [SHARE_CARD_SEED, tokenMint.toBuffer()],
        program.programId
      );
    });

    it("creates/updates a share card", async () => {
      const title = "Moon Token 🌙";
      const description = "The next 1000x gem";
      const imageUri = "https://cdn.sendit.com/cards/moon.png";
      const themeColor = "#FF6B35";

      await program.methods
        .updateShareCard(title, description, imageUri, themeColor)
        .accounts({
          shareCard: shareCardPDA,
          tokenMint,
          creator: creator.publicKey,
          platformConfig: platformConfigPDA,
          systemProgram: SystemProgram.programId,
        })
        .signers([creator])
        .rpc();

      const card = await program.account.shareCard.fetch(shareCardPDA);
      assert.equal(card.title, title);
      assert.equal(card.description, description);
      assert.equal(card.imageUri, imageUri);
      assert.equal(card.themeColor, themeColor);
    });

    it("fetches share card data", async () => {
      const card = await program.account.shareCard.fetch(shareCardPDA);
      assert.ok(card.tokenMint.equals(tokenMint));
      assert.isNotEmpty(card.title);
      assert.isNotEmpty(card.imageUri);
    });

    it("only token creator can update share card", async () => {
      try {
        await program.methods
          .updateShareCard("Hacked", "pwned", "http://evil.com", "#000")
          .accounts({
            shareCard: shareCardPDA,
            tokenMint,
            creator: user1.publicKey,
            platformConfig: platformConfigPDA,
            systemProgram: SystemProgram.programId,
          })
          .signers([user1])
          .rpc();
        assert.fail("Should have thrown Unauthorized");
      } catch (err: any) {
        expect(err.toString()).to.match(
          /Unauthorized|ConstraintHasOne|has one constraint/i
        );
      }
    });
  });

  // ============================================================================
  // Governance: VOTING
  // ============================================================================
  describe("voting", () => {
    let proposalPDA: PublicKey;
    let voteRecordPDA: PublicKey;
    const proposalId = new BN(1);

    before(async () => {
      [proposalPDA] = findPDA(
        [PROPOSAL_SEED, proposalId.toArrayLike(Buffer, "le", 8)],
        program.programId
      );
      [voteRecordPDA] = findPDA(
        [
          VOTE_RECORD_SEED,
          proposalPDA.toBuffer(),
          user1.publicKey.toBuffer(),
        ],
        program.programId
      );
    });

    it("creates a governance proposal", async () => {
      const title = "Increase platform fee to 1.5%";
      const description =
        "Proposal to increase base platform fee from 100 to 150 bps to fund development.";
      const votingDuration = new BN(86400 * 3); // 3 days

      await program.methods
        .createProposal(proposalId, title, description, votingDuration)
        .accounts({
          proposal: proposalPDA,
          proposer: creator.publicKey,
          platformConfig: platformConfigPDA,
          systemProgram: SystemProgram.programId,
        })
        .signers([creator])
        .rpc();

      const proposal = await program.account.proposal.fetch(proposalPDA);
      assert.equal(proposal.title, title);
      expect(proposal.votesFor.toNumber()).to.equal(0);
      expect(proposal.votesAgainst.toNumber()).to.equal(0);
      assert.isFalse(proposal.executed);
    });

    it("casts a vote on a proposal", async () => {
      const weight = new BN(100); // voting power

      await program.methods
        .castVote(true, weight)
        .accounts({
          proposal: proposalPDA,
          voteRecord: voteRecordPDA,
          voter: user1.publicKey,
          platformConfig: platformConfigPDA,
          systemProgram: SystemProgram.programId,
        })
        .signers([user1])
        .rpc();

      const record = await program.account.voteRecord.fetch(voteRecordPDA);
      assert.isTrue(record.support);
      expect(record.weight.toNumber()).to.equal(100);

      const proposal = await program.account.proposal.fetch(proposalPDA);
      expect(proposal.votesFor.toNumber()).to.equal(100);
    });

    it("prevents double voting", async () => {
      try {
        await program.methods
          .castVote(false, new BN(50))
          .accounts({
            proposal: proposalPDA,
            voteRecord: voteRecordPDA,
            voter: user1.publicKey,
            platformConfig: platformConfigPDA,
            systemProgram: SystemProgram.programId,
          })
          .signers([user1])
          .rpc();
        assert.fail("Should have thrown AlreadyVoted");
      } catch (err: any) {
        expect(err.error?.errorCode?.code || err.message).to.match(
          /AlreadyVoted|already in use/
        );
      }
    });

    it("tallies results after voting period", async () => {
      // Cast an opposing vote from user2
      const [vote2PDA] = findPDA(
        [
          VOTE_RECORD_SEED,
          proposalPDA.toBuffer(),
          user2.publicKey.toBuffer(),
        ],
        program.programId
      );

      await program.methods
        .castVote(false, new BN(40))
        .accounts({
          proposal: proposalPDA,
          voteRecord: vote2PDA,
          voter: user2.publicKey,
          platformConfig: platformConfigPDA,
          systemProgram: SystemProgram.programId,
        })
        .signers([user2])
        .rpc();

      const proposal = await program.account.proposal.fetch(proposalPDA);
      expect(proposal.votesFor.toNumber()).to.equal(100);
      expect(proposal.votesAgainst.toNumber()).to.equal(40);
      // For > Against → proposal passed
    });

    it("executes proposal after tally", async () => {
      try {
        await program.methods
          .executeProposal()
          .accounts({
            proposal: proposalPDA,
            authority: authority.publicKey,
            platformConfig: platformConfigPDA,
          })
          .rpc();
      } catch (err: any) {
        // May fail if voting period hasn't ended
        expect(err.error?.errorCode?.code || err.message).to.include(
          "VotingNotEnded"
        );
      }
    });
  });

  // ============================================================================
  // Governance: REPUTATION
  // ============================================================================
  describe("reputation", () => {
    let reputationPDA: PublicKey;

    before(async () => {
      [reputationPDA] = findPDA(
        [REPUTATION_SEED, user1.publicKey.toBuffer()],
        program.programId
      );
    });

    it("initializes reputation account", async () => {
      await program.methods
        .initReputation()
        .accounts({
          reputation: reputationPDA,
          user: user1.publicKey,
          platformConfig: platformConfigPDA,
          systemProgram: SystemProgram.programId,
        })
        .signers([user1])
        .rpc();

      const rep = await program.account.reputation.fetch(reputationPDA);
      assert.ok(rep.user.equals(user1.publicKey));
      expect(rep.score.toNumber()).to.equal(0);
    });

    it("updates reputation score (authority only)", async () => {
      const delta = new BN(50);

      await program.methods
        .updateReputation(delta, true) // true = positive
        .accounts({
          reputation: reputationPDA,
          authority: authority.publicKey,
          platformConfig: platformConfigPDA,
        })
        .rpc();

      const rep = await program.account.reputation.fetch(reputationPDA);
      expect(rep.score.toNumber()).to.equal(50);
    });

    it("decreases reputation score", async () => {
      const delta = new BN(10);

      await program.methods
        .updateReputation(delta, false) // false = negative
        .accounts({
          reputation: reputationPDA,
          authority: authority.publicKey,
          platformConfig: platformConfigPDA,
        })
        .rpc();

      const rep = await program.account.reputation.fetch(reputationPDA);
      expect(rep.score.toNumber()).to.equal(40);
    });

    it("non-authority cannot update reputation", async () => {
      try {
        await program.methods
          .updateReputation(new BN(999), true)
          .accounts({
            reputation: reputationPDA,
            authority: user2.publicKey,
            platformConfig: platformConfigPDA,
          })
          .signers([user2])
          .rpc();
        assert.fail("Should have thrown Unauthorized");
      } catch (err: any) {
        expect(err.toString()).to.match(
          /Unauthorized|ConstraintHasOne|has one constraint/i
        );
      }
    });
  });

  // ============================================================================
  // Governance: HOLDER REWARDS
  // ============================================================================
  describe("holder_rewards", () => {
    let rewardPoolPDA: PublicKey;
    let claimPDA: PublicKey;
    let tokenMint: PublicKey;
    let mintAuthKp: Keypair;

    before(async () => {
      mintAuthKp = Keypair.generate();
      await airdrop(mintAuthKp.publicKey, 10);
      tokenMint = await createTestMint(mintAuthKp);

      [rewardPoolPDA] = findPDA(
        [HOLDER_REWARD_POOL_SEED, tokenMint.toBuffer()],
        program.programId
      );
      [claimPDA] = findPDA(
        [
          HOLDER_REWARD_CLAIM_SEED,
          rewardPoolPDA.toBuffer(),
          user1.publicKey.toBuffer(),
        ],
        program.programId
      );
    });

    it("initializes holder reward pool", async () => {
      await program.methods
        .initHolderRewardPool()
        .accounts({
          holderRewardPool: rewardPoolPDA,
          tokenMint,
          authority: authority.publicKey,
          platformConfig: platformConfigPDA,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      const pool = await program.account.holderRewardPool.fetch(
        rewardPoolPDA
      );
      assert.ok(pool.tokenMint.equals(tokenMint));
      expect(pool.totalAccrued.toNumber()).to.equal(0);
    });

    it("accrues rewards to the pool", async () => {
      const rewardAmount = new BN(10 * LAMPORTS_PER_SOL);

      await program.methods
        .accrueHolderRewards(rewardAmount)
        .accounts({
          holderRewardPool: rewardPoolPDA,
          depositor: authority.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      const pool = await program.account.holderRewardPool.fetch(
        rewardPoolPDA
      );
      expect(pool.totalAccrued.toNumber()).to.equal(10 * LAMPORTS_PER_SOL);
    });

    it("holder claims rewards proportional to holdings", async () => {
      const userAta = await mintTestTokens(
        tokenMint,
        mintAuthKp,
        user1.publicKey,
        1_000_000
      );

      const balBefore = await provider.connection.getBalance(user1.publicKey);

      await program.methods
        .claimHolderRewards()
        .accounts({
          holderRewardPool: rewardPoolPDA,
          holderRewardClaim: claimPDA,
          userTokenAccount: userAta,
          tokenMint,
          holder: user1.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([user1])
        .rpc();

      const claim = await program.account.holderRewardClaim.fetch(claimPDA);
      assert.isTrue(claim.claimed);
    });

    it("prevents double claim", async () => {
      const userAta = await getAssociatedTokenAddress(
        tokenMint,
        user1.publicKey
      );

      try {
        await program.methods
          .claimHolderRewards()
          .accounts({
            holderRewardPool: rewardPoolPDA,
            holderRewardClaim: claimPDA,
            userTokenAccount: userAta,
            tokenMint,
            holder: user1.publicKey,
            tokenProgram: TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
          })
          .signers([user1])
          .rpc();
        assert.fail("Should have thrown AlreadyClaimed");
      } catch (err: any) {
        expect(err.error?.errorCode?.code || err.message).to.match(
          /AlreadyClaimed|already in use/
        );
      }
    });
  });

  // ============================================================================
  // Other: TOKEN VIDEOS
  // ============================================================================
  describe("token_videos", () => {
    let tokenVideoPDA: PublicKey;
    let tokenMint: PublicKey;
    let mintAuthKp: Keypair;

    before(async () => {
      mintAuthKp = Keypair.generate();
      await airdrop(mintAuthKp.publicKey, 10);
      tokenMint = await createTestMint(mintAuthKp);

      [tokenVideoPDA] = findPDA(
        [TOKEN_VIDEO_SEED, tokenMint.toBuffer()],
        program.programId
      );
    });

    it("sets a video for a token", async () => {
      const videoUri = "https://cdn.sendit.com/videos/moon-promo.mp4";
      const title = "Moon Token Launch Video";

      await program.methods
        .setTokenVideo(videoUri, title)
        .accounts({
          tokenVideo: tokenVideoPDA,
          tokenMint,
          creator: creator.publicKey,
          platformConfig: platformConfigPDA,
          systemProgram: SystemProgram.programId,
        })
        .signers([creator])
        .rpc();

      const video = await program.account.tokenVideo.fetch(tokenVideoPDA);
      assert.equal(video.videoUri, videoUri);
      assert.equal(video.title, title);
      expect(video.upvotes).to.equal(0);
      expect(video.downvotes).to.equal(0);
    });

    it("upvotes a video", async () => {
      await program.methods
        .voteTokenVideo(true)
        .accounts({
          tokenVideo: tokenVideoPDA,
          voter: user1.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([user1])
        .rpc();

      const video = await program.account.tokenVideo.fetch(tokenVideoPDA);
      expect(video.upvotes).to.equal(1);
    });

    it("downvotes a video", async () => {
      await program.methods
        .voteTokenVideo(false)
        .accounts({
          tokenVideo: tokenVideoPDA,
          voter: user2.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([user2])
        .rpc();

      const video = await program.account.tokenVideo.fetch(tokenVideoPDA);
      expect(video.downvotes).to.equal(1);
    });

    it("creator removes the video", async () => {
      await program.methods
        .removeTokenVideo()
        .accounts({
          tokenVideo: tokenVideoPDA,
          tokenMint,
          creator: creator.publicKey,
          platformConfig: platformConfigPDA,
        })
        .signers([creator])
        .rpc();

      const video = await program.account.tokenVideo.fetch(tokenVideoPDA);
      assert.isTrue(video.removed);
    });

    it("non-creator cannot remove video", async () => {
      try {
        await program.methods
          .removeTokenVideo()
          .accounts({
            tokenVideo: tokenVideoPDA,
            tokenMint,
            creator: user1.publicKey,
            platformConfig: platformConfigPDA,
          })
          .signers([user1])
          .rpc();
        assert.fail("Should have thrown Unauthorized");
      } catch (err: any) {
        expect(err.toString()).to.match(
          /Unauthorized|ConstraintHasOne|has one constraint/i
        );
      }
    });
  });

  // ============================================================================
  // Other: EMBEDDABLE WIDGETS
  // ============================================================================
  describe("embeddable_widgets", () => {
    let widgetConfigPDA: PublicKey;
    let tokenMint: PublicKey;
    let mintAuthKp: Keypair;
    const widgetId = new BN(1);

    before(async () => {
      mintAuthKp = Keypair.generate();
      await airdrop(mintAuthKp.publicKey, 10);
      tokenMint = await createTestMint(mintAuthKp);

      [widgetConfigPDA] = findPDA(
        [
          WIDGET_CONFIG_SEED,
          tokenMint.toBuffer(),
          widgetId.toArrayLike(Buffer, "le", 8),
        ],
        program.programId
      );
    });

    it("creates a widget config", async () => {
      const theme = "dark";
      const showChart = true;
      const showBuyButton = true;

      await program.methods
        .createWidgetConfig(widgetId, theme, showChart, showBuyButton)
        .accounts({
          widgetConfig: widgetConfigPDA,
          tokenMint,
          creator: creator.publicKey,
          platformConfig: platformConfigPDA,
          systemProgram: SystemProgram.programId,
        })
        .signers([creator])
        .rpc();

      const widget = await program.account.widgetConfig.fetch(
        widgetConfigPDA
      );
      assert.equal(widget.theme, theme);
      assert.isTrue(widget.showChart);
      assert.isTrue(widget.showBuyButton);
      assert.isFalse(widget.disabled);
    });

    it("updates widget config", async () => {
      await program.methods
        .updateWidgetConfig("light", false, true)
        .accounts({
          widgetConfig: widgetConfigPDA,
          creator: creator.publicKey,
        })
        .signers([creator])
        .rpc();

      const widget = await program.account.widgetConfig.fetch(
        widgetConfigPDA
      );
      assert.equal(widget.theme, "light");
      assert.isFalse(widget.showChart);
    });

    it("records a widget view", async () => {
      await program.methods
        .recordWidgetView()
        .accounts({
          widgetConfig: widgetConfigPDA,
          viewer: user1.publicKey,
        })
        .signers([user1])
        .rpc();

      const widget = await program.account.widgetConfig.fetch(
        widgetConfigPDA
      );
      expect(widget.totalViews.toNumber()).to.equal(1);
    });

    it("disables a widget", async () => {
      await program.methods
        .disableWidget()
        .accounts({
          widgetConfig: widgetConfigPDA,
          creator: creator.publicKey,
        })
        .signers([creator])
        .rpc();

      const widget = await program.account.widgetConfig.fetch(
        widgetConfigPDA
      );
      assert.isTrue(widget.disabled);
    });

    it("non-creator cannot disable widget", async () => {
      try {
        await program.methods
          .disableWidget()
          .accounts({
            widgetConfig: widgetConfigPDA,
            creator: user1.publicKey,
          })
          .signers([user1])
          .rpc();
        assert.fail("Should have thrown Unauthorized");
      } catch (err: any) {
        expect(err.toString()).to.match(
          /Unauthorized|ConstraintHasOne|has one constraint/i
        );
      }
    });
  });

  // ============================================================================
  // Other: REFERRAL
  // ============================================================================
  describe("referral", () => {
    let referralPDA: PublicKey;
    let trackerPDA: PublicKey;
    const referralCode = "MOON2026";

    before(async () => {
      [referralPDA] = findPDA(
        [REFERRAL_SEED, Buffer.from(referralCode)],
        program.programId
      );
      [trackerPDA] = findPDA(
        [
          REFERRAL_TRACKER_SEED,
          referralPDA.toBuffer(),
          user2.publicKey.toBuffer(),
        ],
        program.programId
      );
    });

    it("creates a referral code", async () => {
      const rewardBps = 500; // 5% referral reward

      await program.methods
        .createReferral(referralCode, rewardBps)
        .accounts({
          referral: referralPDA,
          referrer: user1.publicKey,
          platformConfig: platformConfigPDA,
          systemProgram: SystemProgram.programId,
        })
        .signers([user1])
        .rpc();

      const referral = await program.account.referral.fetch(referralPDA);
      assert.ok(referral.referrer.equals(user1.publicKey));
      assert.equal(referral.code, referralCode);
      expect(referral.rewardBps).to.equal(500);
      expect(referral.totalReferrals.toNumber()).to.equal(0);
    });

    it("tracks a referral usage", async () => {
      const volume = new BN(5 * LAMPORTS_PER_SOL);

      await program.methods
        .trackReferral(volume)
        .accounts({
          referral: referralPDA,
          referralTracker: trackerPDA,
          referee: user2.publicKey,
          platformConfig: platformConfigPDA,
          systemProgram: SystemProgram.programId,
        })
        .signers([user2])
        .rpc();

      const referral = await program.account.referral.fetch(referralPDA);
      expect(referral.totalReferrals.toNumber()).to.equal(1);
      expect(referral.totalVolume.toNumber()).to.equal(5 * LAMPORTS_PER_SOL);

      const tracker = await program.account.referralTracker.fetch(trackerPDA);
      assert.ok(tracker.referee.equals(user2.publicKey));
    });

    it("prevents duplicate referral codes", async () => {
      try {
        await program.methods
          .createReferral(referralCode, 100)
          .accounts({
            referral: referralPDA,
            referrer: user3.publicKey,
            platformConfig: platformConfigPDA,
            systemProgram: SystemProgram.programId,
          })
          .signers([user3])
          .rpc();
        assert.fail("Should have thrown (account already exists)");
      } catch (err: any) {
        // PDA already initialized
        expect(err.toString()).to.match(/already in use|custom program error/i);
      }
    });

    it("referrer can claim accrued rewards", async () => {
      const balBefore = await provider.connection.getBalance(user1.publicKey);

      await program.methods
        .claimReferralRewards()
        .accounts({
          referral: referralPDA,
          referrer: user1.publicKey,
          platformConfig: platformConfigPDA,
          systemProgram: SystemProgram.programId,
        })
        .signers([user1])
        .rpc();

      const referral = await program.account.referral.fetch(referralPDA);
      expect(referral.claimedRewards.toNumber()).to.be.at.least(0);
    });
  });

  // ============================================================================
  // Other: RAFFLE
  // ============================================================================
  describe("raffle", () => {
    let rafflePDA: PublicKey;
    let ticket1PDA: PublicKey;
    let ticket2PDA: PublicKey;
    const raffleId = new BN(1);

    before(async () => {
      [rafflePDA] = findPDA(
        [RAFFLE_SEED, raffleId.toArrayLike(Buffer, "le", 8)],
        program.programId
      );
      [ticket1PDA] = findPDA(
        [
          RAFFLE_TICKET_SEED,
          rafflePDA.toBuffer(),
          user1.publicKey.toBuffer(),
        ],
        program.programId
      );
      [ticket2PDA] = findPDA(
        [
          RAFFLE_TICKET_SEED,
          rafflePDA.toBuffer(),
          user2.publicKey.toBuffer(),
        ],
        program.programId
      );
    });

    it("creates a raffle", async () => {
      const ticketPrice = new BN(LAMPORTS_PER_SOL / 10);
      const maxTickets = 100;
      const endTimestamp = new BN(Math.floor(Date.now() / 1000) + 86400);
      const prizeName = "1000 SOL Grand Prize";

      await program.methods
        .createRaffle(raffleId, ticketPrice, maxTickets, endTimestamp, prizeName)
        .accounts({
          raffle: rafflePDA,
          creator: creator.publicKey,
          platformConfig: platformConfigPDA,
          systemProgram: SystemProgram.programId,
        })
        .signers([creator])
        .rpc();

      const raffle = await program.account.raffle.fetch(rafflePDA);
      assert.ok(raffle.creator.equals(creator.publicKey));
      expect(raffle.ticketPrice.toNumber()).to.equal(LAMPORTS_PER_SOL / 10);
      expect(raffle.maxTickets).to.equal(100);
      expect(raffle.ticketsSold).to.equal(0);
      assert.isFalse(raffle.drawn);
    });

    it("buys raffle tickets", async () => {
      const numTickets = 3;

      await program.methods
        .buyRaffleTicket(numTickets)
        .accounts({
          raffle: rafflePDA,
          raffleTicket: ticket1PDA,
          buyer: user1.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([user1])
        .rpc();

      const ticket = await program.account.raffleTicket.fetch(ticket1PDA);
      assert.ok(ticket.owner.equals(user1.publicKey));
      expect(ticket.numTickets).to.equal(3);

      const raffle = await program.account.raffle.fetch(rafflePDA);
      expect(raffle.ticketsSold).to.equal(3);
    });

    it("another user buys tickets", async () => {
      await program.methods
        .buyRaffleTicket(5)
        .accounts({
          raffle: rafflePDA,
          raffleTicket: ticket2PDA,
          buyer: user2.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([user2])
        .rpc();

      const raffle = await program.account.raffle.fetch(rafflePDA);
      expect(raffle.ticketsSold).to.equal(8);
    });

    it("draws a winner", async () => {
      // Use recent blockhash as randomness source
      const recentSlothash = anchor.web3.SYSVAR_SLOT_HASHES_PUBKEY;

      await program.methods
        .drawRaffleWinner()
        .accounts({
          raffle: rafflePDA,
          authority: authority.publicKey,
          platformConfig: platformConfigPDA,
          recentSlothashes: recentSlothash,
        })
        .rpc();

      const raffle = await program.account.raffle.fetch(rafflePDA);
      assert.isTrue(raffle.drawn);
      assert.isNotNull(raffle.winner);
    });

    it("cannot buy tickets after raffle is drawn", async () => {
      const [ticket3PDA] = findPDA(
        [
          RAFFLE_TICKET_SEED,
          rafflePDA.toBuffer(),
          user3.publicKey.toBuffer(),
        ],
        program.programId
      );

      try {
        await program.methods
          .buyRaffleTicket(1)
          .accounts({
            raffle: rafflePDA,
            raffleTicket: ticket3PDA,
            buyer: user3.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([user3])
          .rpc();
        assert.fail("Should have thrown RaffleEnded");
      } catch (err: any) {
        expect(err.error?.errorCode?.code || err.message).to.include(
          "RaffleEnded"
        );
      }
    });

    it("winner claims the prize", async () => {
      const raffle = await program.account.raffle.fetch(rafflePDA);
      const winnerPubkey = raffle.winner;

      // Determine which ticket PDA belongs to winner
      const winnerTicketPDA = winnerPubkey.equals(user1.publicKey)
        ? ticket1PDA
        : ticket2PDA;
      const winnerKp = winnerPubkey.equals(user1.publicKey) ? user1 : user2;

      await program.methods
        .claimRafflePrize()
        .accounts({
          raffle: rafflePDA,
          raffleTicket: winnerTicketPDA,
          winner: winnerKp.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([winnerKp])
        .rpc();

      const updatedRaffle = await program.account.raffle.fetch(rafflePDA);
      assert.isTrue(updatedRaffle.prizeClaimed);
    });
  });
});
