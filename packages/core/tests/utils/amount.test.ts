import { expect, test } from 'bun:test';

import { parseAmountMinor } from '../../src/utils/amount';

test('parseAmountMinor converts decimal strings to pence', () => {
	expect(parseAmountMinor('10.00')).toBe(1000);
	expect(parseAmountMinor('-1.50')).toBe(-150);
	expect(parseAmountMinor('0.38')).toBe(38);
});
