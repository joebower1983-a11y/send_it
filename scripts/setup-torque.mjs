#!/usr/bin/env node
/**
 * Send.it × Torque Setup Script
 * 
 * Registers as a Torque publisher, creates campaigns,
 * and registers custom events for on-chain action tracking.
 * 
 * Usage:
 *   TORQUE_API_KEY=<key> node scripts/setup-torque.mjs [--dry-run]
 * 
 * Prerequisites:
 *   - Torque account at https://app.torque.so
 *   - API key from Torque dashboard
 *   - Deployer keypair at ../deployer.json or $DEPLOYER_KEYPAIR
 */

import { 
  initTorqueWithKeypair, 
  createAllCampaigns, 
  registerCustomEvents,
  checkTorqueHealth,
  CAMPAIGN_TEMPLATES,
} from '../lib/torque.mjs';

import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

async function main() {
  const dryRun = process.argv.includes('--dry-run');
  const apiKey = process.env.TORQUE_API_KEY;
  const keypairPath = process.env.DEPLOYER_KEYPAIR || 
    path.resolve(__dirname, '../../deployer.json');

  console.log('╔══════════════════════════════════════╗');
  console.log('║  Send.it × Torque Campaign Setup     ║');
  console.log('╚══════════════════════════════════════╝');
  console.log();

  // Health check
  console.log('🔍 Checking Torque API health...');
  const healthy = await checkTorqueHealth();
  if (!healthy) {
    console.log('⚠️  Torque API is unreachable. Running in dry-run mode.');
    console.log('   The API at https://api.torque.so may be temporarily down.');
    console.log();
    showDryRun();
    return;
  }
  console.log('✅ Torque API is reachable');
  console.log();

  if (!apiKey) {
    console.log('⚠️  No TORQUE_API_KEY set. Running in dry-run mode.');
    console.log('   Get your API key at https://app.torque.so');
    console.log();
    showDryRun();
    return;
  }

  if (dryRun) {
    showDryRun();
    return;
  }

  // Initialize SDK
  console.log('🔑 Initializing Torque SDK...');
  try {
    const { sdk } = await initTorqueWithKeypair(keypairPath, apiKey, {
      publisherHandle: 'sendit',
      network: 'devnet',
    });
    console.log('✅ SDK initialized');
    console.log();

    // Register custom events first
    console.log('📋 Registering custom events...');
    const eventResults = await registerCustomEvents(sdk);
    console.log(`   ${eventResults.filter(r => r.success).length}/${eventResults.length} events registered`);
    console.log();

    // Create campaigns
    console.log('🏆 Creating campaigns...');
    const campaignResults = await createAllCampaigns(sdk);
    console.log(`   ${campaignResults.filter(r => r.success).length}/${campaignResults.length} campaigns created`);
    console.log();

    // Summary
    console.log('═══ Summary ═══');
    for (const r of [...eventResults, ...campaignResults]) {
      const icon = r.success ? '✅' : '❌';
      const name = r.name || r.key;
      console.log(`${icon} ${name}${r.error ? ': ' + r.error : ''}`);
    }
  } catch (err) {
    console.error('❌ Setup failed:', err.message);
    process.exit(1);
  }
}

function showDryRun() {
  console.log('═══ Dry Run — Campaign Preview ═══');
  console.log();
  
  for (const [key, template] of Object.entries(CAMPAIGN_TEMPLATES)) {
    console.log(`📦 ${template.campaignName}`);
    console.log(`   Type: ${template.campaignType}`);
    console.log(`   Landing: ${template.landingPage}`);
    console.log(`   Conversions: ${template.conversionCount}`);
    console.log(`   Reward: ${template.userPayoutPerConversion} ${template.userRewardType}`);
    console.log(`   Events: ${template.eventConfig.map(e => e.type).join(', ')}`);
    console.log();
  }

  console.log('═══ Custom Events ═══');
  console.log('  • sendit_add_liquidity (sol_amount, pool, wallet)');
  console.log('  • sendit_stake (amount, duration_days, wallet)');
  console.log('  • sendit_unstake (amount, wallet)');
  console.log('  • sendit_launch_token (token_name, mint, wallet)');
  console.log('  • sendit_create_pool (token_mint, initial_sol, wallet)');
  console.log();
  console.log('To run for real: TORQUE_API_KEY=<key> node scripts/setup-torque.mjs');
}

main().catch(console.error);
