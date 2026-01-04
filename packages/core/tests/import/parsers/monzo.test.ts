import { afterEach, describe, expect, test } from 'bun:test';
import { unlink } from 'node:fs/promises';

import { parseMonzoCsv } from '../../../src/import/parsers/monzo';

const TEST_FILE = '/tmp/monzo-test.csv';

async function writeTestCsv(content: string) {
	await Bun.write(TEST_FILE, content);
}

describe('parseMonzoCsv', () => {
	afterEach(async () => {
		try {
			await unlink(TEST_FILE);
		} catch {
			// Ignore if file doesn't exist
		}
	});

	test('parses standard Monzo CSV row', async () => {
		const csv = `Transaction ID,Date,Time,Amount,Currency,Name,Description,Category,Balance
txn_001,15/01/2024,10:30:45,-25.50,GBP,Tesco,Card payment,groceries,150.00`;

		await writeTestCsv(csv);
		const result = await parseMonzoCsv(TEST_FILE, 'Assets:Personal:Monzo');

		expect(result.chartAccountId).toBe('Assets:Personal:Monzo');
		expect(result.transactions).toHaveLength(1);
		expect(result.hasBalances).toBe(true);

		const txn = result.transactions[0];
		expect(txn?.providerTxnId).toBe('txn_001');
		expect(txn?.postedAt).toBe('2024-01-15T10:30:45');
		expect(txn?.amountMinor).toBe(-2550);
		expect(txn?.currency).toBe('GBP');
		expect(txn?.rawDescription).toBe('Card payment');
		expect(txn?.counterparty).toBe('Tesco');
		expect(txn?.providerCategory).toBe('groceries');
		expect(txn?.balanceMinor).toBe(15000);
	});

	test('uses Name when Description is empty', async () => {
		const csv = `Transaction ID,Date,Time,Amount,Currency,Name,Description,Category,Balance
txn_002,15/01/2024,11:00:00,-15.00,GBP,Coffee Shop,,eating_out,135.00`;

		await writeTestCsv(csv);
		const result = await parseMonzoCsv(TEST_FILE, 'Assets:Personal:Monzo');

		const txn = result.transactions[0];
		expect(txn?.rawDescription).toBe('Coffee Shop');
		expect(txn?.counterparty).toBe('Coffee Shop');
	});

	test('handles missing balance', async () => {
		const csv = `Transaction ID,Date,Time,Amount,Currency,Name,Description,Category,Balance
txn_003,16/01/2024,09:00:00,-10.00,GBP,Shop,Purchase,,`;

		await writeTestCsv(csv);
		const result = await parseMonzoCsv(TEST_FILE, 'Assets:Personal:Monzo');

		const txn = result.transactions[0];
		expect(txn?.balanceMinor).toBe(null);
		expect(result.hasBalances).toBe(false);
	});

	test('parses multiple rows', async () => {
		const csv = `Transaction ID,Date,Time,Amount,Currency,Name,Description,Category,Balance
txn_001,15/01/2024,10:00:00,-100.00,GBP,First,Desc 1,,900.00
txn_002,16/01/2024,11:00:00,-200.00,GBP,Second,Desc 2,,700.00
txn_003,17/01/2024,12:00:00,500.00,GBP,Third,Desc 3,,1200.00`;

		await writeTestCsv(csv);
		const result = await parseMonzoCsv(TEST_FILE, 'Assets:Personal:Monzo');

		expect(result.transactions).toHaveLength(3);
		expect(result.transactions[0]?.amountMinor).toBe(-10000);
		expect(result.transactions[1]?.amountMinor).toBe(-20000);
		expect(result.transactions[2]?.amountMinor).toBe(50000);
	});

	test('skips rows with missing date or time', async () => {
		const csv = `Transaction ID,Date,Time,Amount,Currency,Name,Description,Category,Balance
txn_001,15/01/2024,10:00:00,-50.00,GBP,Valid,Valid row,,100.00
txn_002,,11:00:00,-25.00,GBP,Missing Date,Skip,,75.00
txn_003,16/01/2024,,-30.00,GBP,Missing Time,Skip,,45.00`;

		await writeTestCsv(csv);
		const result = await parseMonzoCsv(TEST_FILE, 'Assets:Personal:Monzo');

		expect(result.transactions).toHaveLength(1);
		expect(result.transactions[0]?.rawDescription).toBe('Valid row');
	});

	test('accepts business-monzo account', async () => {
		const csv = `Transaction ID,Date,Time,Amount,Currency,Name,Description,Category,Balance
txn_001,15/01/2024,10:00:00,-100.00,GBP,Test,Test,,0`;

		await writeTestCsv(csv);
		const result = await parseMonzoCsv(TEST_FILE, 'Assets:Business:Monzo');

		expect(result.chartAccountId).toBe('Assets:Business:Monzo');
	});

	test('accepts joint-monzo account', async () => {
		const csv = `Transaction ID,Date,Time,Amount,Currency,Name,Description,Category,Balance
txn_001,15/01/2024,10:00:00,-100.00,GBP,Test,Test,,0`;

		await writeTestCsv(csv);
		const result = await parseMonzoCsv(TEST_FILE, 'Assets:Joint:Monzo');

		expect(result.chartAccountId).toBe('Assets:Joint:Monzo');
	});

	test('accepts personal-savings account', async () => {
		const csv = `Transaction ID,Date,Time,Amount,Currency,Name,Description,Category,Balance
txn_001,15/01/2024,10:00:00,500.00,GBP,Interest,Monthly interest,,10500.00`;

		await writeTestCsv(csv);
		const result = await parseMonzoCsv(TEST_FILE, 'Assets:Personal:Savings');

		expect(result.chartAccountId).toBe('Assets:Personal:Savings');
	});

	test('throws for invalid account', async () => {
		await writeTestCsv('Transaction ID,Date,Time\n');

		await expect(parseMonzoCsv(TEST_FILE, 'Assets:Business:Wise')).rejects.toThrow('is not configured as a Monzo account');
	});

	test('handles date with slashes', async () => {
		const csv = `Transaction ID,Date,Time,Amount,Currency,Name,Description,Category,Balance
txn_001,15/01/2024,10:30:00,-25.00,GBP,Test,Test,,100.00`;

		await writeTestCsv(csv);
		const result = await parseMonzoCsv(TEST_FILE, 'Assets:Personal:Monzo');

		const txn = result.transactions[0];
		expect(txn?.postedAt).toBe('2024-01-15T10:30:00');
	});

	test('handles time without seconds', async () => {
		const csv = `Transaction ID,Date,Time,Amount,Currency,Name,Description,Category,Balance
txn_001,15/01/2024,10:30,-25.00,GBP,Test,Test,,100.00`;

		await writeTestCsv(csv);
		const result = await parseMonzoCsv(TEST_FILE, 'Assets:Personal:Monzo');

		const txn = result.transactions[0];
		expect(txn?.postedAt).toBe('2024-01-15T10:30:00');
	});

	test('handles empty name', async () => {
		const csv = `Transaction ID,Date,Time,Amount,Currency,Name,Description,Category,Balance
txn_001,15/01/2024,10:00:00,-25.00,GBP,,Direct debit,,100.00`;

		await writeTestCsv(csv);
		const result = await parseMonzoCsv(TEST_FILE, 'Assets:Personal:Monzo');

		const txn = result.transactions[0];
		expect(txn?.rawDescription).toBe('Direct debit');
		expect(txn?.counterparty).toBe(null);
	});
});
