import { expect, test } from 'bun:test';

import { parseWiseDateTime, toIsoLocalDateTime } from '../../src/utils/datetime';

test('toIsoLocalDateTime normalizes DD/MM/YYYY + HH:MM:SS', () => {
	expect(toIsoLocalDateTime('11/12/2025', '08:08:09')).toBe('2025-12-11T08:08:09');
});

test('parseWiseDateTime normalizes DD-MM-YYYY HH:MM:SS.mmm', () => {
	expect(parseWiseDateTime('30-11-2025 07:46:38.928')).toBe('2025-11-30T07:46:38');
});
