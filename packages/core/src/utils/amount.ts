export function parseAmountMinor(raw: string): number {
	const cleaned = raw.trim().replaceAll(',', '');
	if (cleaned.length === 0) {
		throw new Error('Empty amount');
	}

	const value = Number(cleaned);
	if (!Number.isFinite(value)) {
		throw new Error(`Invalid amount: ${raw}`);
	}

	return Math.round(value * 100);
}
