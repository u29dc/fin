import { afterEach, describe, expect, test } from 'bun:test';
import { unlink } from 'node:fs/promises';

import { parseWiseCsv } from '../../../src/import/parsers/wise';

const TEST_FILE = '/tmp/wise-test.csv';

async function writeTestCsv(content: string) {
	await Bun.write(TEST_FILE, content);
}

describe('parseWiseCsv', () => {
	afterEach(async () => {
		try {
			await unlink(TEST_FILE);
		} catch {
			// Ignore if file doesn't exist
		}
	});

	test('parses standard Wise CSV row', async () => {
		const csv = `TransferWise ID,Date Time,Date,Amount,Currency,Description,Payment Reference,Payee Name,Payer Name,Transaction Type,Running Balance
TXN-001,15-01-2024 10:30:00.123,15-01-2024,-1000.50,GBP,Card payment at Coffee Shop,REF123,Coffee Shop,,CARD,5000.00`;

		await writeTestCsv(csv);
		const result = await parseWiseCsv(TEST_FILE, 'Assets:Business:Wise');

		expect(result.chartAccountId).toBe('Assets:Business:Wise');
		expect(result.transactions).toHaveLength(1);
		expect(result.hasBalances).toBe(true);

		const txn = result.transactions[0];
		expect(txn?.providerTxnId).toBe('TXN-001');
		expect(txn?.postedAt).toBe('2024-01-15T10:30:00');
		expect(txn?.amountMinor).toBe(-100050);
		expect(txn?.currency).toBe('GBP');
		expect(txn?.rawDescription).toBe('REF123 - Card payment at Coffee Shop');
		expect(txn?.counterparty).toBe('Coffee Shop');
		expect(txn?.providerCategory).toBe('CARD');
		expect(txn?.balanceMinor).toBe(500000);
	});

	test('handles missing payment reference', async () => {
		const csv = `TransferWise ID,Date Time,Date,Amount,Currency,Description,Payment Reference,Payee Name,Payer Name,Transaction Type,Running Balance
TXN-002,15-01-2024 10:30:00,15-01-2024,-50.00,GBP,Direct debit,,British Gas,,DIRECT_DEBIT,4950.00`;

		await writeTestCsv(csv);
		const result = await parseWiseCsv(TEST_FILE, 'Assets:Business:Wise');

		const txn = result.transactions[0];
		expect(txn?.rawDescription).toBe('Direct debit');
	});

	test('handles payer name when payee is empty', async () => {
		const csv = `TransferWise ID,Date Time,Date,Amount,Currency,Description,Payment Reference,Payee Name,Payer Name,Transaction Type,Running Balance
TXN-003,16-01-2024 14:00:00,16-01-2024,500.00,GBP,Bank transfer,,,"Client Corp",CREDIT,5500.00`;

		await writeTestCsv(csv);
		const result = await parseWiseCsv(TEST_FILE, 'Assets:Business:Wise');

		const txn = result.transactions[0];
		expect(txn?.counterparty).toBe('Client Corp');
		expect(txn?.amountMinor).toBe(50000);
	});

	test('falls back to Date when Date Time is empty', async () => {
		const csv = `TransferWise ID,Date Time,Date,Amount,Currency,Description,Payment Reference,Payee Name,Payer Name,Transaction Type,Running Balance
TXN-004,,16-01-2024,-100.00,GBP,Subscription,,,,,5400.00`;

		await writeTestCsv(csv);
		const result = await parseWiseCsv(TEST_FILE, 'Assets:Business:Wise');

		const txn = result.transactions[0];
		expect(txn?.postedAt).toBe('2024-01-16T00:00:00');
	});

	test('handles empty running balance', async () => {
		const csv = `TransferWise ID,Date Time,Date,Amount,Currency,Description,Payment Reference,Payee Name,Payer Name,Transaction Type,Running Balance
TXN-005,17-01-2024 09:00:00,17-01-2024,-25.00,GBP,Test,,,,CARD,`;

		await writeTestCsv(csv);
		const result = await parseWiseCsv(TEST_FILE, 'Assets:Business:Wise');

		const txn = result.transactions[0];
		expect(txn?.balanceMinor).toBe(null);
	});

	test('parses multiple rows', async () => {
		const csv = `TransferWise ID,Date Time,Date,Amount,Currency,Description,Payment Reference,Payee Name,Payer Name,Transaction Type,Running Balance
TXN-001,15-01-2024 10:00:00,15-01-2024,-100.00,GBP,First,,,,CARD,900.00
TXN-002,16-01-2024 11:00:00,16-01-2024,-200.00,GBP,Second,,,,CARD,700.00
TXN-003,17-01-2024 12:00:00,17-01-2024,500.00,GBP,Third,,,Client,CREDIT,1200.00`;

		await writeTestCsv(csv);
		const result = await parseWiseCsv(TEST_FILE, 'Assets:Business:Wise');

		expect(result.transactions).toHaveLength(3);
		expect(result.transactions[0]?.amountMinor).toBe(-10000);
		expect(result.transactions[1]?.amountMinor).toBe(-20000);
		expect(result.transactions[2]?.amountMinor).toBe(50000);
	});

	test('throws for non-business-wise account', async () => {
		await writeTestCsv('TransferWise ID,Date Time\n');

		await expect(parseWiseCsv(TEST_FILE, 'Assets:Personal:Monzo')).rejects.toThrow('is not configured as a Wise account');
	});

	test('handles commas in amount', async () => {
		const csv = `TransferWise ID,Date Time,Date,Amount,Currency,Description,Payment Reference,Payee Name,Payer Name,Transaction Type,Running Balance
TXN-001,15-01-2024 10:00:00,15-01-2024,"1,234.56",GBP,Large payment,,,,CREDIT,"10,000.00"`;

		await writeTestCsv(csv);
		const result = await parseWiseCsv(TEST_FILE, 'Assets:Business:Wise');

		const txn = result.transactions[0];
		expect(txn?.amountMinor).toBe(123456);
		expect(txn?.balanceMinor).toBe(1000000);
	});
});
