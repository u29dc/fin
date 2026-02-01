/**
 * Demo Data Generator
 * Creates a fake database with realistic financial data for screenshots and demos.
 *
 * This script is fully self-contained - it will:
 * 1. Create the data/ directory if needed
 * 2. Back up any existing config and database
 * 3. Copy the config template to data/fin.config.toml
 * 4. Generate realistic demo transactions
 *
 * Usage: bun run demo:generate
 *
 * To restore your real data afterward:
 *   mv data/fin.config.toml.backup data/fin.config.toml
 *   mv data/fin.db.backup data/fin.db
 */

/* biome-ignore-all lint/suspicious/noConsole: CLI script requires console output */

import { Database } from 'bun:sqlite';
import { copyFileSync, existsSync, mkdirSync, renameSync, unlinkSync } from 'node:fs';
import { resolve } from 'node:path';

import { getConfig, initConfig } from '../config/index';
import { migrateToLatest } from '../db/migrate';

// Paths
const DATA_DIR = resolve(process.cwd(), 'data');
const DB_PATH = resolve(DATA_DIR, 'fin.db');
const CONFIG_PATH = resolve(DATA_DIR, 'fin.config.toml');
const CONFIG_TEMPLATE_PATH = resolve(process.cwd(), 'fin.config.template.toml');

// Date range for demo data
const START_DATE = new Date('2023-01-01');
const END_DATE = new Date('2025-12-31');

// Target ending balances (in minor units - pence)
const TARGET_BALANCES: Record<string, number> = {
	'Assets:Personal:Monzo': 2500000, // 25,000
	'Assets:Personal:Savings': 7500000, // 75,000
	'Assets:Personal:Investments': 10000000, // 100,000
	'Assets:Business:Wise': 25000000, // 250,000
	'Assets:Business:Monzo': 5000000, // 50,000
	'Assets:Joint:Monzo': 4000000, // 40,000
};

// Helper functions
function randomId(): string {
	return `demo_${Date.now()}_${Math.random().toString(36).slice(2, 11)}`;
}

function randomBetween(min: number, max: number): number {
	return Math.floor(Math.random() * (max - min + 1)) + min;
}

function formatDate(date: Date): string {
	const datePart = date.toISOString().split('T')[0] ?? '';
	return `${datePart}T12:00:00`;
}

function addDays(date: Date, days: number): Date {
	const result = new Date(date);
	result.setDate(result.getDate() + days);
	return result;
}

function addMonths(date: Date, months: number): Date {
	const result = new Date(date);
	result.setMonth(result.getMonth() + months);
	return result;
}

function pickRandom<T>(arr: T[]): T {
	return arr[randomBetween(0, arr.length - 1)] as T;
}

// Expense categories with rounded demo amounts
type ExpenseTemplate = { min: number; max: number; frequency: string; names: string[] };
const PERSONAL_EXPENSE_TEMPLATES: Record<string, ExpenseTemplate> = {
	'Expenses:Food:Groceries': { min: 5000, max: 5000, frequency: 'weekly', names: ['Supermarket'] },
	'Expenses:Food:Restaurants': { min: 5000, max: 10000, frequency: 'biweekly', names: ['Restaurant'] },
	'Expenses:Food:Coffee': { min: 500, max: 500, frequency: 'daily', names: ['Coffee Shop'] },
	'Expenses:Food:Delivery': { min: 2500, max: 2500, frequency: 'weekly', names: ['Food Delivery'] },
	'Expenses:Housing:Utilities': { min: 15000, max: 15000, frequency: 'monthly', names: ['Utilities'] },
	'Expenses:Transport:PublicTransport': { min: 15000, max: 15000, frequency: 'monthly', names: ['Transport'] },
	'Expenses:Transport:Taxi': { min: 2000, max: 2000, frequency: 'weekly', names: ['Taxi'] },
	'Expenses:Entertainment:Subscriptions': { min: 5000, max: 5000, frequency: 'monthly', names: ['Subscriptions'] },
	'Expenses:Entertainment:Leisure': { min: 5000, max: 10000, frequency: 'monthly', names: ['Entertainment'] },
	'Expenses:Health:Fitness': { min: 5000, max: 5000, frequency: 'monthly', names: ['Gym'] },
	'Expenses:Shopping:Clothing': { min: 10000, max: 10000, frequency: 'monthly', names: ['Shopping'] },
	'Expenses:Shopping:Electronics': { min: 25000, max: 25000, frequency: 'quarterly', names: ['Electronics'] },
	'Expenses:Personal:Gifts': { min: 5000, max: 5000, frequency: 'monthly', names: ['Gifts'] },
};

// Business expenses - ~10K/month total
const BUSINESS_EXPENSE_TEMPLATES: Record<string, ExpenseTemplate> = {
	'Expenses:Business:Software': { min: 300000, max: 300000, frequency: 'monthly', names: ['AWS'] },
	'Expenses:Business:Software:2': { min: 100000, max: 100000, frequency: 'monthly', names: ['Vercel'] },
	'Expenses:Business:Software:3': { min: 50000, max: 50000, frequency: 'monthly', names: ['GitHub'] },
	'Expenses:Business:Software:4': { min: 50000, max: 50000, frequency: 'monthly', names: ['Other SaaS'] },
	'Expenses:Business:Services': { min: 300000, max: 300000, frequency: 'monthly', names: ['Contractor'] },
	'Expenses:Business:BankFees': { min: 10000, max: 10000, frequency: 'monthly', names: ['Bank Fees'] },
	'Expenses:Business:Accounting': { min: 100000, max: 100000, frequency: 'monthly', names: ['Accountant'] },
};

// Business income sources - rounded amounts
const BUSINESS_CLIENTS = [
	{ name: 'Client A', amount: 3000000 }, // 30,000
	{ name: 'Client B', amount: 2000000 }, // 20,000
	{ name: 'Client C', amount: 1000000 }, // 10,000
];

type Transaction = {
	journalId: string;
	postingId: string;
	postedAt: string;
	description: string;
	accountId: string;
	amount: number;
	counterAccountId: string;
	providerTxnId: string;
	balance: number;
};

class DemoDataGenerator {
	private db: Database;
	private transactions: Transaction[] = [];
	private balances: Record<string, number> = {};
	private journalStmt: ReturnType<Database['prepare']>;
	private postingStmt: ReturnType<Database['prepare']>;

	constructor(db: Database) {
		this.db = db;

		// Initialize balances at zero
		for (const accountId of Object.keys(TARGET_BALANCES)) {
			this.balances[accountId] = 0;
		}

		this.journalStmt = db.prepare(`
			INSERT INTO journal_entries (id, posted_at, description, raw_description, counterparty, source_file)
			VALUES (?, ?, ?, ?, ?, ?)
		`);

		this.postingStmt = db.prepare(`
			INSERT INTO postings (id, journal_entry_id, account_id, amount_minor, currency, provider_txn_id, provider_balance_minor)
			VALUES (?, ?, ?, ?, ?, ?, ?)
		`);
	}

	private addTransaction(postedAt: Date, description: string, accountId: string, amount: number, counterAccountId: string): void {
		const journalId = randomId();
		const postingId = randomId();
		const providerTxnId = randomId();

		this.balances[accountId] = (this.balances[accountId] ?? 0) + amount;
		const balance = this.balances[accountId] ?? 0;

		this.transactions.push({
			journalId,
			postingId,
			postedAt: formatDate(postedAt),
			description,
			accountId,
			amount,
			counterAccountId,
			providerTxnId,
			balance,
		});
	}

	private getYearFactor(year: number): number {
		if (year === 2023) return 0.5;
		if (year === 2024) return 0.75;
		return 1.0;
	}

	private getBusinessTargetAccount(): string {
		return Math.random() < 0.8 ? 'Assets:Business:Wise' : 'Assets:Business:Monzo';
	}

	private processClientPayment(client: (typeof BUSINESS_CLIENTS)[number], paymentDate: Date): void {
		const yearFactor = this.getYearFactor(paymentDate.getFullYear());
		const adjustedAmount = Math.round(client.amount * yearFactor);
		const targetAccount = this.getBusinessTargetAccount();
		this.addTransaction(paymentDate, client.name, targetAccount, adjustedAmount, 'Income:Other');
	}

	private generateBusinessIncome(): void {
		console.log('Generating business income...');
		let currentDate = new Date(START_DATE);

		while (currentDate <= END_DATE) {
			for (let i = 0; i < BUSINESS_CLIENTS.length; i++) {
				const client = BUSINESS_CLIENTS[i];
				if (!client) continue;

				const dayOfMonth = 5 + i * 7;
				const paymentDate = new Date(currentDate.getFullYear(), currentDate.getMonth(), dayOfMonth);

				const isInRange = paymentDate >= START_DATE && paymentDate <= END_DATE;
				if (isInRange) {
					this.processClientPayment(client, paymentDate);
				}
			}

			currentDate = addMonths(currentDate, 1);
		}
	}

	private generateSalaryAndDividends(): void {
		console.log('Generating salary and dividends...');
		let currentDate = new Date(START_DATE);

		while (currentDate <= END_DATE) {
			// Monthly salary - 4000
			const salaryDate = new Date(currentDate.getFullYear(), currentDate.getMonth() + 1, 0);
			if (salaryDate <= END_DATE) {
				this.addTransaction(salaryDate, 'Salary', 'Assets:Personal:Monzo', 400000, 'Income:Salary');
			}

			// Monthly dividends - 6000
			const dividendDate = new Date(currentDate.getFullYear(), currentDate.getMonth(), 15);
			if (dividendDate >= START_DATE && dividendDate <= END_DATE) {
				this.addTransaction(dividendDate, 'Dividend', 'Assets:Personal:Monzo', 600000, 'Income:Dividends');
			}

			currentDate = addMonths(currentDate, 1);
		}
	}

	private shouldGenerateExpense(frequency: string, date: Date): { generate: boolean; iterations: number } {
		switch (frequency) {
			case 'daily':
				return { generate: true, iterations: randomBetween(0, 2) };
			case 'weekly':
				return { generate: date.getDay() === 6, iterations: 1 }; // Saturdays
			case 'biweekly':
				return { generate: date.getDate() === 7 || date.getDate() === 21, iterations: 1 };
			case 'monthly':
				return { generate: date.getDate() === 1, iterations: 1 };
			case 'quarterly':
				return { generate: [0, 3, 6, 9].includes(date.getMonth()) && date.getDate() === 15, iterations: 1 };
			default:
				return { generate: false, iterations: 0 };
		}
	}

	private generatePersonalExpenses(): void {
		console.log('Generating personal expenses...');
		let currentDate = new Date(START_DATE);

		while (currentDate <= END_DATE) {
			for (const [categoryId, template] of Object.entries(PERSONAL_EXPENSE_TEMPLATES)) {
				const { generate, iterations } = this.shouldGenerateExpense(template.frequency, currentDate);

				if (generate) {
					for (let i = 0; i < iterations; i++) {
						const amount = -randomBetween(template.min, template.max);
						const name = pickRandom(template.names);
						this.addTransaction(currentDate, name, 'Assets:Personal:Monzo', amount, categoryId);
					}
				}
			}

			currentDate = addDays(currentDate, 1);
		}
	}

	private generateJointExpenses(): void {
		console.log('Generating joint expenses...');
		let currentDate = new Date(START_DATE);

		while (currentDate <= END_DATE) {
			// Joint pays rent on 1st of month - 2000
			if (currentDate.getDate() === 1) {
				this.addTransaction(currentDate, 'Rent', 'Assets:Joint:Monzo', -200000, 'Expenses:Housing:Rent');
			}

			// Joint pays utilities on 5th - 200
			if (currentDate.getDate() === 5) {
				this.addTransaction(currentDate, 'Utilities', 'Assets:Joint:Monzo', -20000, 'Expenses:Housing:Utilities');
			}

			// Joint pays groceries weekly - 100
			if (currentDate.getDay() === 6) {
				this.addTransaction(currentDate, 'Groceries', 'Assets:Joint:Monzo', -10000, 'Expenses:Food:Groceries');
			}

			currentDate = addDays(currentDate, 1);
		}
	}

	private generateBusinessExpenses(): void {
		console.log('Generating business expenses...');
		let currentDate = new Date(START_DATE);

		while (currentDate <= END_DATE) {
			for (const [templateId, template] of Object.entries(BUSINESS_EXPENSE_TEMPLATES)) {
				// Map template IDs to actual category IDs (strip :2, :3 suffixes)
				const categoryId = templateId.replace(/:\d+$/, '');

				const { generate } = this.shouldGenerateExpense(template.frequency, currentDate);

				if (generate) {
					const amount = -randomBetween(template.min, template.max);
					const name = pickRandom(template.names);
					this.addTransaction(currentDate, name, 'Assets:Business:Wise', amount, categoryId);
				}
			}

			currentDate = addDays(currentDate, 1);
		}
	}

	private generateTransfers(): void {
		console.log('Generating transfers...');
		let currentDate = new Date(START_DATE);

		while (currentDate <= END_DATE) {
			// Monthly savings - 1000
			if (currentDate.getDate() === 15) {
				this.addTransaction(currentDate, 'To Savings', 'Assets:Personal:Monzo', -100000, 'Equity:Transfers');
				this.addTransaction(currentDate, 'From Current', 'Assets:Personal:Savings', 100000, 'Equity:Transfers');
			}

			// Monthly transfer to Joint for rent/bills - 1500 (your share of 2000 rent + bills)
			if (currentDate.getDate() === 28) {
				this.addTransaction(currentDate, 'To Joint Account', 'Assets:Personal:Monzo', -150000, 'Equity:Transfers');
				this.addTransaction(currentDate, 'From Personal', 'Assets:Joint:Monzo', 150000, 'Equity:Transfers');
			}

			// Quarterly investment - 2500
			if ([2, 5, 8, 11].includes(currentDate.getMonth()) && currentDate.getDate() === 20) {
				this.addTransaction(currentDate, 'To Investments', 'Assets:Personal:Savings', -250000, 'Equity:Transfers');
				this.addTransaction(currentDate, 'Fund Purchase', 'Assets:Personal:Investments', 250000, 'Equity:Transfers');
			}

			// Occasional business to personal - 5000
			if (currentDate.getDate() === 10 && Math.random() < 0.3) {
				this.addTransaction(currentDate, 'Director Loan', 'Assets:Business:Wise', -500000, 'Equity:Transfers');
				this.addTransaction(currentDate, 'From Business', 'Assets:Personal:Monzo', 500000, 'Equity:Transfers');
			}

			currentDate = addDays(currentDate, 1);
		}
	}

	private generateInvestmentGrowth(): void {
		console.log('Generating investment growth...');
		let currentDate = addMonths(new Date(START_DATE), 1);

		while (currentDate <= END_DATE) {
			const currentBalance = this.balances['Assets:Personal:Investments'] ?? 0;
			if (currentBalance > 0) {
				const monthlyReturnRate = Math.random() * 0.05 - 0.02;
				const growth = Math.floor(currentBalance * monthlyReturnRate);

				if (growth !== 0) {
					const label = growth > 0 ? 'Investment Growth' : 'Investment Decline';
					this.addTransaction(currentDate, label, 'Assets:Personal:Investments', growth, 'Income:Interest');
				}
			}

			currentDate = addMonths(currentDate, 1);
		}
	}

	private writeToDatabase(): void {
		console.log(`Writing ${this.transactions.length} transactions to database...`);

		this.transactions.sort((a, b) => a.postedAt.localeCompare(b.postedAt));

		const runningBalances: Record<string, number> = {};

		const transaction = this.db.transaction(() => {
			for (const txn of this.transactions) {
				runningBalances[txn.accountId] = (runningBalances[txn.accountId] ?? 0) + txn.amount;
				const balance = runningBalances[txn.accountId] ?? 0;
				const counterparty = txn.description.split(' - ')[0] ?? txn.description;

				this.journalStmt.run(txn.journalId, txn.postedAt, txn.description, txn.description, counterparty, 'demo-generator');
				this.postingStmt.run(txn.postingId, txn.journalId, txn.accountId, txn.amount, 'GBP', txn.providerTxnId, balance);
				this.postingStmt.run(randomId(), txn.journalId, txn.counterAccountId, -txn.amount, 'GBP', null, null);
			}
		});

		transaction();

		console.log('\nFinal account balances:');
		for (const [accountId, balance] of Object.entries(runningBalances)) {
			const formatted = (balance / 100).toLocaleString('en-GB', { style: 'currency', currency: 'GBP' });
			console.log(`  ${accountId}: ${formatted}`);
		}
	}

	generate(): void {
		this.generateBusinessIncome();
		this.generateSalaryAndDividends();
		this.generatePersonalExpenses();
		this.generateJointExpenses();
		this.generateBusinessExpenses();
		this.generateTransfers();
		this.generateInvestmentGrowth();
		// Skip opening balances - they create negative adjustments that skew cashflow
		// this.generateOpeningBalances();
		this.writeToDatabase();
	}
}

function getBackupTimestamp(): string {
	const now = new Date();
	const ts = now.toISOString().replace(/[-:]/g, '').replace('T', '-').split('.')[0];
	return ts ?? 'unknown';
}

function backupFile(filePath: string, label: string): string | null {
	if (!existsSync(filePath)) {
		return null;
	}

	const timestamp = getBackupTimestamp();
	const backupPath = `${filePath}.backup.${timestamp}`;
	const backupName = backupPath.split('/').pop() ?? backupPath;
	console.log(`  Backing up ${label} to ${backupName}`);
	renameSync(filePath, backupPath);
	return backupPath;
}

function log(message: string): void {
	console.log(message);
}

function logError(message: string): void {
	console.error(message);
}

function setupDataDirectory(): void {
	log('Step 1: Setting up data directory...');
	if (!existsSync(DATA_DIR)) {
		log(`  Creating ${DATA_DIR}`);
		mkdirSync(DATA_DIR, { recursive: true });
	} else {
		log('  Data directory exists');
	}
	log('');
}

function cleanupWalFiles(dbBackup: string | null): void {
	if (!dbBackup) return;
	for (const ext of ['-shm', '-wal']) {
		const walPath = DB_PATH + ext;
		if (existsSync(walPath)) {
			unlinkSync(walPath);
		}
	}
}

function printRestoreInstructions(configBackup: string | null, dbBackup: string | null): void {
	if (!configBackup && !dbBackup) return;
	log('To restore your real data:');
	if (configBackup) {
		const configBackupName = configBackup.split('/').pop() ?? configBackup;
		log(`  mv ${configBackupName} data/fin.config.toml`);
	}
	if (dbBackup) {
		const dbBackupName = dbBackup.split('/').pop() ?? dbBackup;
		log(`  mv ${dbBackupName} data/fin.db`);
	}
	log('');
}

async function main(): Promise<void> {
	log('');
	log('Demo Data Generator');
	log('===================');
	log('');

	setupDataDirectory();

	log('Step 2: Backing up existing files...');
	const configBackup = backupFile(CONFIG_PATH, 'config');
	const dbBackup = backupFile(DB_PATH, 'database');
	cleanupWalFiles(dbBackup);
	if (!configBackup && !dbBackup) {
		log('  No existing files to back up');
	}
	log('');

	log('Step 3: Setting up config...');
	if (!existsSync(CONFIG_TEMPLATE_PATH)) {
		logError(`  Error: Config template not found at ${CONFIG_TEMPLATE_PATH}`);
		logError('  Make sure you are running this from the repository root.');
		process.exit(1);
	}
	log('  Copying config template to data/fin.config.toml');
	copyFileSync(CONFIG_TEMPLATE_PATH, CONFIG_PATH);
	log('');

	log('Step 4: Loading config...');
	initConfig();
	const config = getConfig();
	log(`  Loaded ${config.accounts.length} accounts`);
	log(`  Groups: ${config.groups?.map((g) => g.id).join(', ') ?? 'default'}`);
	log('');

	log('Step 5: Creating database...');
	const db = new Database(DB_PATH, { create: true });

	db.exec(`
		PRAGMA foreign_keys = ON;
		PRAGMA journal_mode = WAL;
		PRAGMA synchronous = NORMAL;
	`);

	log('  Running migrations...');
	migrateToLatest(db);
	log('');

	log('Step 6: Generating demo data...');
	log('');
	const generator = new DemoDataGenerator(db);
	generator.generate();

	db.close();

	log('');
	log('===================');
	log('Demo setup complete!');
	log('===================');
	log('');
	log('You can now run the app:');
	log('  bun run dev');
	log('');

	printRestoreInstructions(configBackup, dbBackup);
}

main().catch((error) => {
	logError(String(error));
	process.exit(1);
});
