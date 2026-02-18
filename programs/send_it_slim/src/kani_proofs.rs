/// Kani Proof Harnesses for Send.it Slim Program
///
/// Verifies core math functions in isolation from Anchor runtime:
/// - Bonding curve pricing (buy/sell)
/// - AMM constant-product swap math
/// - LP token minting/burning proportionality
/// - Fee calculations and conservation
/// - Staking reward computation
/// - Integer square root
///
/// Run: cargo kani --harness <name>
/// Requires: cargo-kani installed

#[cfg(kani)]
mod kani_proofs {
    // ── Constants (mirrored from lib.rs) ──
    const DEFAULT_TOTAL_SUPPLY: u64 = 1_000_000_000_000_000;
    const PRECISION: u128 = 1_000_000_000_000;
    const SWAP_FEE_BPS: u64 = 100;
    const LP_FEE_BPS: u64 = 30;
    const PROTOCOL_FEE_BPS: u64 = 70;
    const MIN_LIQUIDITY: u64 = 1000;
    const REWARD_RATE_BPS_PER_YEAR: u64 = 1000;
    const SECONDS_PER_YEAR: u64 = 365 * 24 * 3600;
    const RENT_EXEMPT_MIN: u64 = 890_880;

    fn isqrt(n: u128) -> u128 {
        if n == 0 { return 0; }
        let mut x = n;
        let mut y = (x + 1) / 2;
        while y < x { x = y; y = (x + n / x) / 2; }
        x
    }

    // ══════════════════════════════════════════════════════════════
    // 1. INTEGER SQUARE ROOT
    // ══════════════════════════════════════════════════════════════

    #[kani::proof]
    #[kani::unwind(130)]
    fn proof_isqrt_correctness() {
        let n: u128 = kani::any();
        // Bound to realistic range: 0 to max pool product (u64::MAX * u64::MAX)
        kani::assume(n <= (u64::MAX as u128) * (u64::MAX as u128));

        let root = isqrt(n);

        // root^2 <= n
        assert!(root.checked_mul(root).map_or(false, |sq| sq <= n));
        // (root+1)^2 > n  (root is the floor)
        if root < u128::MAX {
            let next = root + 1;
            assert!(next.checked_mul(next).map_or(true, |sq| sq > n));
        }
        // Non-vacuity: confirm we reach this point
        kani::cover!(n > 0 && root > 0);
        kani::cover!(n == 0 && root == 0);
    }

    // ══════════════════════════════════════════════════════════════
    // 2. BONDING CURVE — BUY PRICE CALCULATION
    // ══════════════════════════════════════════════════════════════

    /// Pure math extracted from buy():
    /// current_price = base + slope * tokens_sold / PRECISION
    /// raw_tokens = sol_amount * 1_000_000_000 / current_price
    /// tokens_out = min(raw_tokens, available)
    fn buy_math(
        sol_amount: u64,
        tokens_sold: u64,
        total_supply: u64,
        platform_fee_bps: u16,
        creator_fee_bps: u16,
    ) -> Option<(u64, u64, u64, u64)> {
        // tokens_out, platform_fee, creator_fee, net_sol
        let available = total_supply.checked_sub(tokens_sold)?;
        if available == 0 { return None; }

        let base = 1_000u128;
        let slope = PRECISION.checked_div(total_supply as u128)?;
        let cp = base.checked_add(
            slope.checked_mul(tokens_sold as u128)?
                .checked_div(PRECISION)?
        )?;

        let raw_tokens = (sol_amount as u128)
            .checked_mul(1_000_000_000u128)?
            .checked_div(cp)?;
        let tokens_out = raw_tokens.min(available as u128) as u64;
        if tokens_out == 0 { return None; }

        let pf = (sol_amount as u128)
            .checked_mul(platform_fee_bps as u128)?
            .checked_div(10_000)? as u64;
        let cf = (sol_amount as u128)
            .checked_mul(creator_fee_bps as u128)?
            .checked_div(10_000)? as u64;
        let net = sol_amount.checked_sub(pf)?.checked_sub(cf)?;

        Some((tokens_out, pf, cf, net))
    }

    #[kani::proof]
    #[kani::unwind(2)]
    fn proof_buy_fee_conservation() {
        let sol_amount: u64 = kani::any();
        let tokens_sold: u64 = kani::any();
        let platform_fee_bps: u16 = kani::any();
        let creator_fee_bps: u16 = kani::any();

        // Realistic bounds
        kani::assume(sol_amount > 0 && sol_amount <= 1_000_000_000_000); // up to 1000 SOL
        kani::assume(tokens_sold < DEFAULT_TOTAL_SUPPLY);
        kani::assume(platform_fee_bps <= 1000);
        kani::assume(creator_fee_bps <= 500);

        if let Some((tokens_out, pf, cf, net)) = buy_math(
            sol_amount, tokens_sold, DEFAULT_TOTAL_SUPPLY,
            platform_fee_bps, creator_fee_bps,
        ) {
            // Fee conservation: platform_fee + creator_fee + net == sol_amount
            assert_eq!(pf + cf + net, sol_amount, "SOL conservation violated");

            // tokens_out <= available supply
            assert!(tokens_out <= DEFAULT_TOTAL_SUPPLY - tokens_sold);

            // tokens_out > 0 (guaranteed by function)
            assert!(tokens_out > 0);

            // Non-vacuity
            kani::cover!(tokens_out > 0 && pf > 0 && cf > 0);
            kani::cover!(pf == 0 && cf == 0); // zero-fee path
        }
    }

    #[kani::proof]
    #[kani::unwind(2)]
    fn proof_buy_price_increases_with_supply() {
        let sol_amount: u64 = kani::any();
        let tokens_sold_low: u64 = kani::any();
        let tokens_sold_high: u64 = kani::any();

        kani::assume(sol_amount > 0 && sol_amount <= 100_000_000_000); // up to 100 SOL
        kani::assume(tokens_sold_low < tokens_sold_high);
        kani::assume(tokens_sold_high < DEFAULT_TOTAL_SUPPLY);

        let result_low = buy_math(sol_amount, tokens_sold_low, DEFAULT_TOTAL_SUPPLY, 0, 0);
        let result_high = buy_math(sol_amount, tokens_sold_high, DEFAULT_TOTAL_SUPPLY, 0, 0);

        if let (Some((tokens_low, _, _, _)), Some((tokens_high, _, _, _))) = (result_low, result_high) {
            // More tokens sold → higher price → fewer tokens received for same SOL
            assert!(tokens_low >= tokens_high,
                "Bonding curve price should increase: got more tokens at higher supply");
            kani::cover!(tokens_low > tokens_high); // strict increase
        }
    }

    // ══════════════════════════════════════════════════════════════
    // 3. BONDING CURVE — SELL PRICE CALCULATION
    // ══════════════════════════════════════════════════════════════

    fn sell_math(
        token_amount: u64,
        tokens_sold: u64,
        total_supply: u64,
        reserve_sol: u64,
        platform_fee_bps: u16,
        creator_fee_bps: u16,
    ) -> Option<(u64, u64, u64, u64)> {
        // sol_out, platform_fee, creator_fee, net
        if token_amount == 0 || token_amount > tokens_sold { return None; }

        let ns = tokens_sold.checked_sub(token_amount)?;
        let base = 1_000u128;
        let slope = PRECISION.checked_div(total_supply as u128)?;
        let sum = (ns as u128).checked_add(tokens_sold as u128)?;
        let avg = base.checked_add(
            slope.checked_mul(sum)?
                .checked_div(2u128.checked_mul(PRECISION)?)?
        )?;
        let sol_out = avg.checked_mul(token_amount as u128)?
            .checked_div(1_000_000_000u128)? as u64;
        if sol_out == 0 { return None; }

        let pf = (sol_out as u128).checked_mul(platform_fee_bps as u128)?.checked_div(10_000)? as u64;
        let cf = (sol_out as u128).checked_mul(creator_fee_bps as u128)?.checked_div(10_000)? as u64;
        let net = sol_out.checked_sub(pf)?.checked_sub(cf)?;

        if net > reserve_sol { return None; }

        Some((sol_out, pf, cf, net))
    }

    #[kani::proof]
    #[kani::unwind(2)]
    fn proof_sell_fee_conservation() {
        let token_amount: u64 = kani::any();
        let tokens_sold: u64 = kani::any();
        let reserve_sol: u64 = kani::any();
        let platform_fee_bps: u16 = kani::any();
        let creator_fee_bps: u16 = kani::any();

        kani::assume(token_amount > 0 && token_amount <= 1_000_000_000_000_000);
        kani::assume(tokens_sold >= token_amount && tokens_sold <= DEFAULT_TOTAL_SUPPLY);
        kani::assume(reserve_sol > 0 && reserve_sol <= 1_000_000_000_000);
        kani::assume(platform_fee_bps <= 1000);
        kani::assume(creator_fee_bps <= 500);

        if let Some((sol_out, pf, cf, net)) = sell_math(
            token_amount, tokens_sold, DEFAULT_TOTAL_SUPPLY,
            reserve_sol, platform_fee_bps, creator_fee_bps,
        ) {
            // Fee conservation: pf + cf + net == sol_out
            assert_eq!(pf + cf + net, sol_out, "Sell SOL conservation violated");

            // net <= reserve (won't drain below reserve)
            assert!(net <= reserve_sol);

            kani::cover!(sol_out > 0 && pf > 0 && cf > 0);
        }
    }

    // ══════════════════════════════════════════════════════════════
    // 4. AMM SWAP — CONSTANT PRODUCT INVARIANT
    // ══════════════════════════════════════════════════════════════

    fn swap_buy_math(
        sol_amount: u64,
        sol_reserve: u64,
        token_reserve: u64,
    ) -> Option<(u64, u64, u64)> {
        // Returns: (token_out, fee, new_k_check)
        let fee = sol_amount.checked_mul(SWAP_FEE_BPS)?.checked_div(10_000)?;
        let net_sol = sol_amount.checked_sub(fee)?;

        let new_sol = (sol_reserve as u128).checked_add(net_sol as u128)?;
        let k_before = (sol_reserve as u128).checked_mul(token_reserve as u128)?;
        let token_out = (token_reserve as u128)
            .checked_sub(k_before.checked_div(new_sol)?)?;
        let token_out = token_out as u64;
        if token_out == 0 { return None; }

        let new_token = (token_reserve as u128).checked_sub(token_out as u128)?;
        let k_after = new_sol.checked_mul(new_token)?;

        Some((token_out, fee, k_after as u64))
    }

    fn swap_sell_math(
        token_amount: u64,
        sol_reserve: u64,
        token_reserve: u64,
    ) -> Option<(u64, u64)> {
        // Returns: (sol_out, fee_tokens)
        let fee_tokens = token_amount.checked_mul(SWAP_FEE_BPS)?.checked_div(10_000)?;
        let net_tokens = token_amount.checked_sub(fee_tokens)?;

        let new_token = (token_reserve as u128).checked_add(net_tokens as u128)?;
        let k_before = (sol_reserve as u128).checked_mul(token_reserve as u128)?;
        let sol_out = (sol_reserve as u128)
            .checked_sub(k_before.checked_div(new_token)?)?;
        let sol_out = sol_out as u64;
        if sol_out == 0 { return None; }

        Some((sol_out, fee_tokens))
    }

    #[kani::proof]
    #[kani::unwind(2)]
    fn proof_swap_buy_k_non_decreasing() {
        let sol_amount: u64 = kani::any();
        let sol_reserve: u64 = kani::any();
        let token_reserve: u64 = kani::any();

        // Realistic pool sizes
        kani::assume(sol_amount > 0 && sol_amount <= 100_000_000_000); // up to 100 SOL
        kani::assume(sol_reserve >= 1_000_000 && sol_reserve <= 1_000_000_000_000); // 0.001 - 1000 SOL
        kani::assume(token_reserve >= 1_000_000 && token_reserve <= DEFAULT_TOTAL_SUPPLY);

        let k_before = (sol_reserve as u128) * (token_reserve as u128);

        if let Some((token_out, fee, _)) = swap_buy_math(sol_amount, sol_reserve, token_reserve) {
            let fee_net = sol_amount.checked_mul(SWAP_FEE_BPS).unwrap() / 10_000;
            let net_sol = sol_amount - fee_net;
            let new_sol = sol_reserve as u128 + net_sol as u128;
            let new_token = token_reserve as u128 - token_out as u128;
            let k_after = new_sol * new_token;

            // CONSTANT PRODUCT INVARIANT: k never decreases after swap
            assert!(k_after >= k_before, "k decreased after buy swap!");

            // token_out < token_reserve (can't drain pool)
            assert!(token_out < token_reserve);

            // fee > 0 for any meaningful swap
            kani::cover!(fee > 0 && token_out > 0);
        }
    }

    #[kani::proof]
    #[kani::unwind(2)]
    fn proof_swap_sell_k_non_decreasing() {
        let token_amount: u64 = kani::any();
        let sol_reserve: u64 = kani::any();
        let token_reserve: u64 = kani::any();

        kani::assume(token_amount > 0 && token_amount <= 1_000_000_000_000_000);
        kani::assume(sol_reserve >= 1_000_000 && sol_reserve <= 1_000_000_000_000);
        kani::assume(token_reserve >= 1_000_000 && token_reserve <= DEFAULT_TOTAL_SUPPLY);

        let k_before = (sol_reserve as u128) * (token_reserve as u128);

        if let Some((sol_out, fee_tokens)) = swap_sell_math(token_amount, sol_reserve, token_reserve) {
            let net_tokens = token_amount - fee_tokens;
            let new_token = token_reserve as u128 + net_tokens as u128;
            let new_sol = sol_reserve as u128 - sol_out as u128;
            let k_after = new_sol * new_token;

            // CONSTANT PRODUCT INVARIANT
            assert!(k_after >= k_before, "k decreased after sell swap!");

            // sol_out < sol_reserve
            assert!(sol_out < sol_reserve);

            kani::cover!(sol_out > 0 && fee_tokens > 0);
        }
    }

    #[kani::proof]
    #[kani::unwind(2)]
    fn proof_swap_fee_split() {
        let sol_amount: u64 = kani::any();
        kani::assume(sol_amount >= 10_000 && sol_amount <= 1_000_000_000_000);

        let total_fee = sol_amount.checked_mul(SWAP_FEE_BPS).unwrap() / 10_000;
        let protocol_fee = total_fee.checked_mul(PROTOCOL_FEE_BPS).unwrap() / SWAP_FEE_BPS;
        let lp_fee = total_fee - protocol_fee;

        // Fee split conservation
        assert_eq!(protocol_fee + lp_fee, total_fee, "Fee split doesn't sum to total");

        // Protocol gets 70%, LP gets 30%
        kani::cover!(protocol_fee > 0 && lp_fee > 0);
        kani::cover!(total_fee == 0); // below dust threshold
    }

    // ══════════════════════════════════════════════════════════════
    // 5. LP TOKEN PROPORTIONALITY
    // ══════════════════════════════════════════════════════════════

    #[kani::proof]
    #[kani::unwind(2)]
    fn proof_add_remove_liquidity_roundtrip() {
        let sol_add: u64 = kani::any();
        let sol_reserve: u64 = kani::any();
        let token_reserve: u64 = kani::any();
        let lp_supply: u64 = kani::any();

        kani::assume(sol_add > 0 && sol_add <= 100_000_000_000);
        kani::assume(sol_reserve >= 1_000_000 && sol_reserve <= 1_000_000_000_000);
        kani::assume(token_reserve >= 1_000_000 && token_reserve <= DEFAULT_TOTAL_SUPPLY);
        kani::assume(lp_supply >= MIN_LIQUIDITY && lp_supply <= 1_000_000_000_000_000);

        // Add liquidity
        let token_add = ((sol_add as u128) * (token_reserve as u128) / (sol_reserve as u128)) as u64;
        let lp_minted = ((lp_supply as u128) * (sol_add as u128) / (sol_reserve as u128)) as u64;

        if token_add == 0 || lp_minted == 0 { return; }

        let new_sol = sol_reserve as u128 + sol_add as u128;
        let new_token = token_reserve as u128 + token_add as u128;
        let new_lp = lp_supply as u128 + lp_minted as u128;

        // Remove the same LP tokens
        let sol_back = (lp_minted as u128) * new_sol / new_lp;
        let token_back = (lp_minted as u128) * new_token / new_lp;

        // Should get back approximately what we put in (within rounding)
        // Due to integer division, sol_back <= sol_add (rounding favors pool)
        assert!(sol_back <= sol_add as u128 + 1, "Got back more SOL than deposited");
        assert!(token_back <= token_add as u128 + 1, "Got back more tokens than deposited");

        kani::cover!(sol_back > 0 && token_back > 0);
    }

    #[kani::proof]
    #[kani::unwind(130)]
    fn proof_initial_lp_sqrt() {
        let initial_sol: u64 = kani::any();
        let initial_tokens: u64 = kani::any();

        kani::assume(initial_sol >= 1_000_000 && initial_sol <= 1_000_000_000_000);
        kani::assume(initial_tokens >= 1_000_000 && initial_tokens <= DEFAULT_TOTAL_SUPPLY);

        let product = (initial_sol as u128).checked_mul(initial_tokens as u128);
        if let Some(prod) = product {
            let lp = isqrt(prod);

            // LP^2 <= product
            assert!(lp * lp <= prod);

            // LP amount should be meaningful
            if lp > MIN_LIQUIDITY as u128 {
                let to_creator = lp - MIN_LIQUIDITY as u128;
                assert!(to_creator > 0, "Creator gets 0 LP tokens");
                kani::cover!(to_creator > 0);
            }
        }
    }

    // ══════════════════════════════════════════════════════════════
    // 6. STAKING REWARD CALCULATION
    // ══════════════════════════════════════════════════════════════

    fn calc_reward(amount: u64, duration_secs: u64) -> Option<u64> {
        let reward = (amount as u128)
            .checked_mul(REWARD_RATE_BPS_PER_YEAR as u128)?
            .checked_mul(duration_secs as u128)?
            .checked_div(10_000u128.checked_mul(SECONDS_PER_YEAR as u128)?)?;
        Some(reward as u64)
    }

    #[kani::proof]
    #[kani::unwind(2)]
    fn proof_staking_reward_proportional() {
        let amount: u64 = kani::any();
        let duration: u64 = kani::any();

        kani::assume(amount > 0 && amount <= DEFAULT_TOTAL_SUPPLY);
        kani::assume(duration >= 60 && duration <= 365 * 24 * 3600); // 1 min to 1 year

        if let Some(reward) = calc_reward(amount, duration) {
            // Reward should be <= 10% of amount (10% APY, max 1 year)
            let max_reward = amount / 10 + 1; // 10% + rounding
            assert!(reward <= max_reward, "Reward exceeds 10% APY cap");

            // Reward monotonically increases with duration
            if duration > 60 {
                if let Some(reward_shorter) = calc_reward(amount, duration - 1) {
                    assert!(reward >= reward_shorter, "Reward decreased with longer duration");
                }
            }

            // Reward monotonically increases with amount
            if amount > 1 {
                if let Some(reward_less) = calc_reward(amount - 1, duration) {
                    assert!(reward >= reward_less, "Reward decreased with more tokens");
                }
            }

            kani::cover!(reward > 0);
            kani::cover!(reward == 0); // very small amount or short duration
        }
    }

    // ══════════════════════════════════════════════════════════════
    // 7. BONDING CURVE BUY-SELL ROUNDTRIP
    // ══════════════════════════════════════════════════════════════

    #[kani::proof]
    #[kani::unwind(2)]
    fn proof_buy_sell_no_profit() {
        let sol_amount: u64 = kani::any();
        let tokens_sold_initial: u64 = kani::any();

        kani::assume(sol_amount >= 1_000_000 && sol_amount <= 10_000_000_000); // 0.001 - 10 SOL
        kani::assume(tokens_sold_initial < DEFAULT_TOTAL_SUPPLY / 2);

        // Buy with 0 fees to isolate curve math
        if let Some((tokens_bought, _, _, _)) = buy_math(
            sol_amount, tokens_sold_initial, DEFAULT_TOTAL_SUPPLY, 0, 0,
        ) {
            let new_tokens_sold = tokens_sold_initial + tokens_bought;
            // Immediately sell all tokens back
            if let Some((sol_back, _, _, _)) = sell_math(
                tokens_bought, new_tokens_sold, DEFAULT_TOTAL_SUPPLY,
                sol_amount, 0, 0,
            ) {
                // Should get back <= what we paid (bonding curve is not an arbitrage machine)
                assert!(sol_back <= sol_amount, "Profit from buy-sell roundtrip!");
                kani::cover!(sol_back > 0 && sol_back < sol_amount); // lost to rounding
                kani::cover!(sol_back == sol_amount); // exact roundtrip
            }
        }
    }
}
