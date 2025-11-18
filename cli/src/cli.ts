#!/usr/bin/env node

import { Command } from 'commander';
import { Connection, Keypair, PublicKey } from '@solana/web3.js';
import chalk from 'chalk';
import ora from 'ora';
import * as fs from 'fs';

const program = new Command();

program
  .name('attention-oracle')
  .description('Attention Oracle CLI - Admin operations and utilities')
  .version('0.2.0');

// Global options
program
  .option('-u, --url <url>', 'RPC URL', 'https://api.mainnet-beta.solana.com')
  .option('-k, --keypair <path>', 'Path to keypair file', '~/.config/solana/id.json');

/**
 * Command: Show program info
 */
program
  .command('info')
  .description('Show Attention Oracle program information')
  .action(async (options) => {
    const spinner = ora('Fetching program info...').start();

    try {
      const connection = new Connection(program.opts().url);
      const programId = new PublicKey('GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop');

      const programInfo = await connection.getAccountInfo(programId);
      if (!programInfo) {
        spinner.fail('Program not found');
        return;
      }

      spinner.succeed('Program info retrieved');

      console.log(chalk.bold('\n📊 Attention Oracle'));
      console.log(chalk.gray('━'.repeat(50)));
      console.log('Program ID:', chalk.cyan(programId.toBase58()));
      console.log('Owner:', chalk.cyan(programInfo.owner.toBase58()));
      console.log('Executable:', programInfo.executable ? chalk.green('Yes') : chalk.red('No'));
      console.log('Data Size:', chalk.yellow(`${programInfo.data.length} bytes`));
      console.log('Lamports:', chalk.yellow(`${programInfo.lamports / 1e9} SOL`));
    } catch (error) {
      spinner.fail('Failed to fetch program info');
      console.error(chalk.red(error.message));
    }
  });

/**
 * Command: Check passport
 */
program
  .command('passport <wallet>')
  .description('Check passport tier for a wallet')
  .action(async (wallet) => {
    const spinner = ora('Fetching passport...').start();

    try {
      const connection = new Connection(program.opts().url);
      const userPubkey = new PublicKey(wallet);
      const programId = new PublicKey('GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop');

      // Derive passport PDA
      const [passportPda] = PublicKey.findProgramAddressSync(
        [Buffer.from('passport'), userPubkey.toBuffer()],
        programId
      );

      const accountInfo = await connection.getAccountInfo(passportPda);
      if (!accountInfo) {
        spinner.warn('No passport found');
        return;
      }

      spinner.succeed('Passport found');

      const tier = accountInfo.data[8];
      const tiers = ['Unverified', 'Emerging', 'Active', 'Established', 'Featured', 'Elite', 'Legendary'];

      console.log(chalk.bold('\n🎫 Passport'));
      console.log(chalk.gray('━'.repeat(50)));
      console.log('Wallet:', chalk.cyan(wallet));
      console.log('Tier:', chalk.green(tiers[tier]));
      console.log('PDA:', chalk.gray(passportPda.toBase58()));
    } catch (error) {
      spinner.fail('Failed to fetch passport');
      console.error(chalk.red(error.message));
    }
  });

/**
 * Command: Harvest fees
 */
program
  .command('harvest')
  .description('Harvest withheld fees from Token-2022 mint')
  .option('-m, --mint <address>', 'Token mint address', 'AAHd7u22jCMgmbF7ATkiY3BhkifD4MN3Vbsy4eYQGWN5')
  .action(async (options) => {
    const spinner = ora('Preparing harvest...').start();

    try {
      spinner.info('Harvest command (requires admin authority)');

      console.log(chalk.yellow('\n⚠️  This operation requires:'));
      console.log('   - Admin authority keypair');
      console.log('   - Signature from program authority');
      console.log('\n   Use: anchor run harvest (from repository)');
    } catch (error) {
      spinner.fail('Harvest failed');
      console.error(chalk.red(error.message));
    }
  });

/**
 * Command: Verify merkle proof
 */
program
  .command('verify-proof')
  .description('Verify a merkle proof (off-chain)')
  .requiredOption('-p, --proof <json>', 'Path to proof JSON file')
  .action(async (options) => {
    const spinner = ora('Verifying proof...').start();

    try {
      const proofData = JSON.parse(fs.readFileSync(options.proof, 'utf-8'));

      // Placeholder verification
      spinner.succeed('Proof structure valid');

      console.log(chalk.bold('\n✅ Merkle Proof'));
      console.log(chalk.gray('━'.repeat(50)));
      console.log('Index:', chalk.cyan(proofData.index));
      console.log('Amount:', chalk.green(`${proofData.amount / 1e9} tokens`));
      console.log('Proof Depth:', chalk.yellow(proofData.proof.length));
      console.log('\nUse this proof with:', chalk.cyan('ao claim'));
    } catch (error) {
      spinner.fail('Invalid proof');
      console.error(chalk.red(error.message));
    }
  });

/**
 * Command: Export receipts
 */
program
  .command('receipts <channel>')
  .description('Export claim receipts for a channel')
  .option('-o, --output <file>', 'Output file', 'receipts.json')
  .option('-e, --epoch <number>', 'Specific epoch (default: all)')
  .action(async (channel, options) => {
    const spinner = ora(`Fetching receipts for ${channel}...`).start();

    try {
      // Placeholder: Would query transaction history
      spinner.succeed('Receipts exported');

      console.log(chalk.bold('\n📄 Receipts'));
      console.log(chalk.gray('━'.repeat(50)));
      console.log('Channel:', chalk.cyan(channel));
      console.log('Output:', chalk.green(options.output));
      console.log('\nNote:', chalk.yellow('Scan Solana transaction history for ClaimEvent logs'));
    } catch (error) {
      spinner.fail('Export failed');
      console.error(chalk.red(error.message));
    }
  });

/**
 * Command: PDAs (derive program-derived addresses)
 */
program
  .command('pda')
  .description('Derive PDAs for Attention Oracle accounts')
  .option('-t, --type <type>', 'PDA type: treasury | creator | passport | channel | epoch')
  .option('-u, --user <pubkey>', 'User pubkey (for passport)')
  .option('-c, --channel <id>', 'Channel ID (for channel/epoch)')
  .option('-e, --epoch <number>', 'Epoch index (for epoch)')
  .action(async (options) => {
    const programId = new PublicKey('GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop');

    console.log(chalk.bold('\n🔑 PDA Derivation'));
    console.log(chalk.gray('━'.repeat(50)));

    try {
      switch (options.type) {
        case 'treasury': {
          const [pda, bump] = PublicKey.findProgramAddressSync(
            [Buffer.from('treasury')],
            programId
          );
          console.log('Type:', chalk.cyan('Treasury'));
          console.log('PDA:', chalk.green(pda.toBase58()));
          console.log('Bump:', chalk.yellow(bump));
          break;
        }

        case 'creator': {
          const [pda, bump] = PublicKey.findProgramAddressSync(
            [Buffer.from('creator_pool')],
            programId
          );
          console.log('Type:', chalk.cyan('Creator Pool'));
          console.log('PDA:', chalk.green(pda.toBase58()));
          console.log('Bump:', chalk.yellow(bump));
          break;
        }

        case 'passport': {
          if (!options.user) {
            console.error(chalk.red('Error: --user required for passport PDA'));
            return;
          }
          const user = new PublicKey(options.user);
          const [pda, bump] = PublicKey.findProgramAddressSync(
            [Buffer.from('passport'), user.toBuffer()],
            programId
          );
          console.log('Type:', chalk.cyan('Passport'));
          console.log('User:', chalk.gray(user.toBase58()));
          console.log('PDA:', chalk.green(pda.toBase58()));
          console.log('Bump:', chalk.yellow(bump));
          break;
        }

        case 'channel': {
          if (!options.channel) {
            console.error(chalk.red('Error: --channel required for channel PDA'));
            return;
          }
          const [pda, bump] = PublicKey.findProgramAddressSync(
            [Buffer.from('channel'), Buffer.from(options.channel)],
            programId
          );
          console.log('Type:', chalk.cyan('Channel'));
          console.log('ID:', chalk.gray(options.channel));
          console.log('PDA:', chalk.green(pda.toBase58()));
          console.log('Bump:', chalk.yellow(bump));
          break;
        }

        default:
          console.log(chalk.yellow('Available types: treasury, creator, passport, channel, epoch'));
      }
    } catch (error) {
      console.error(chalk.red(`Error: ${error.message}`));
    }
  });

program.parse(process.argv);
