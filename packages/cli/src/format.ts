/**
 * Formatters for amounts, dates, and percentages.
 * All amounts are stored as minor units (pence) and displayed as GBP.
 */

// Cached formatter - created once, reused for all calls
const gbpFormatter = new Intl.NumberFormat('en-GB', {
	minimumFractionDigits: 2,
	maximumFractionDigits: 2,
});

/**
 * Format a minor unit amount (pence) as GBP string.
 * Examples: 123456 -> "1,234.56", -50000 -> "-500.00"
 */
export function formatAmount(minor: number | null | undefined): string {
	if (minor === null || minor === undefined) return '-';

	const pounds = minor / 100;
	const abs = Math.abs(pounds);
	const formatted = gbpFormatter.format(abs);

	return pounds < 0 ? `-${formatted}` : formatted;
}

/**
 * Format a minor unit amount with currency symbol.
 * Examples: 123456 -> "£1,234.56", -50000 -> "-£500.00"
 */
export function formatAmountWithSymbol(minor: number | null | undefined): string {
	if (minor === null || minor === undefined) return '-';

	const pounds = minor / 100;
	const abs = Math.abs(pounds);
	const formatted = gbpFormatter.format(abs);

	return pounds < 0 ? `-£${formatted}` : `£${formatted}`;
}

/**
 * Format an ISO date string to YYYY-MM-DD.
 * Examples: "2024-01-15T10:30:00Z" -> "2024-01-15"
 */
export function formatDate(iso: string | null | undefined): string {
	if (!iso) return '-';
	return iso.slice(0, 10);
}

/**
 * Format an ISO date string to YYYY-MM (month only).
 */
export function formatMonth(iso: string | null | undefined): string {
	if (!iso) return '-';
	return iso.slice(0, 7);
}

/**
 * Format a percentage value.
 * Examples: 0.385 -> "38.5%", null -> "-"
 */
export function formatPercent(value: number | null | undefined, decimals = 1): string {
	if (value === null || value === undefined) return '-';
	return `${(value * 100).toFixed(decimals)}%`;
}

/**
 * Format a percentage value that's already in percentage form (not decimal).
 * Examples: 38.5 -> "38.5%"
 */
export function formatPercentRaw(value: number | null | undefined, decimals = 1): string {
	if (value === null || value === undefined) return '-';
	return `${value.toFixed(decimals)}%`;
}

/**
 * Format a number of months (runway).
 * Examples: 12.5 -> "12.5 mo", 120 -> "120+ mo"
 */
export function formatMonths(months: number | null | undefined, maxCap = 120): string {
	if (months === null || months === undefined) return '-';
	if (months >= maxCap) return `${maxCap}+ mo`;
	return `${months.toFixed(1)} mo`;
}

/**
 * Format a count with optional singular/plural.
 */
export function formatCount(count: number, singular: string, plural?: string): string {
	const label = count === 1 ? singular : (plural ?? `${singular}s`);
	return `${count} ${label}`;
}
