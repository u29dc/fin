// ============================================================================
// Types
// ============================================================================

export type BalancePoint = {
	date: string;
	balanceMinor: number;
};

export type ProjectionPoint = {
	month: number; // 0 = now, 1 = next month, etc.
	date: string; // YYYY-MM-DD
	balanceMinor: number;
};

export type CashAssetsPoint = {
	date: string;
	accounts: Record<string, number | null>;
};

export type RunwayPoint = {
	date: string;
	runwayMonths: number;
	isNetPositive?: boolean;
};

export type CashflowPoint = {
	month: string;
	incomeMinor: number;
	expenseMinor: number;
	netMinor: number;
	savingsRatePct: number | null;
	rollingMedianExpenseMinor: number | null;
	expenseDeviationRatio: number | null;
};

export type InvestmentPoint = {
	date: string;
	contributionsMinor: number;
};

export type ValuePoint = {
	date: string;
	valueMinor: number;
};

export type MoneyPoint = {
	date: string;
	valueMinor: number;
};

export type AnnualReturns = {
	low: number;
	mid: number;
	high: number;
};

// ============================================================================
// Constants
// ============================================================================

export const DAY_MS = 24 * 60 * 60 * 1000;
export const YEAR_MS = 365.25 * DAY_MS;

export const DEFAULT_CHART_FONT_FAMILY = "'JetBrains Mono', 'SFMono-Regular', Menlo, Monaco, Consolas, 'Liberation Mono', monospace";

export const CHART_COLORS = {
	light: {
		background: '#ffffff',
		text: '#6b7280',
		grid: 'rgba(0, 0, 0, 0.06)',
		crosshair: 'rgba(0, 0, 0, 0.22)',
		labelBackground: '#f3f4f6',
	},
	dark: {
		background: '#111315',
		text: '#9aa0a6',
		grid: 'rgba(230, 230, 232, 0.06)',
		crosshair: 'rgba(230, 230, 232, 0.22)',
		labelBackground: '#1b1e22',
	},
} as const;

// Semantic colors for income/expense visualizations (color-blind safe: teal/orange)
export const SEMANTIC_COLORS = {
	light: {
		income: '#0d9488', // teal-600
		incomeMuted: 'rgba(13, 148, 136, 0.6)',
		incomeFill: 'rgba(13, 148, 136, 0.28)',
		incomeFillFaint: 'rgba(13, 148, 136, 0.04)',
		incomeFillMuted: 'rgba(13, 148, 136, 0.12)',
		incomeFillMutedFaint: 'rgba(13, 148, 136, 0.02)',
		expense: '#ea580c', // orange-600
		expenseMuted: 'rgba(234, 88, 12, 0.6)',
		expenseFill: 'rgba(234, 88, 12, 0.28)',
		expenseFillFaint: 'rgba(234, 88, 12, 0.04)',
		expenseFillMuted: 'rgba(234, 88, 12, 0.12)',
		expenseFillMutedFaint: 'rgba(234, 88, 12, 0.02)',
		warning: 'rgba(217, 119, 6, 0.8)', // amber-700
	},
	dark: {
		income: '#2dd4bf', // teal-400
		incomeMuted: 'rgba(45, 212, 191, 0.6)',
		incomeFill: 'rgba(45, 212, 191, 0.28)',
		incomeFillFaint: 'rgba(45, 212, 191, 0.04)',
		incomeFillMuted: 'rgba(45, 212, 191, 0.12)',
		incomeFillMutedFaint: 'rgba(45, 212, 191, 0.02)',
		expense: '#fb923c', // orange-400
		expenseMuted: 'rgba(251, 146, 60, 0.6)',
		expenseFill: 'rgba(251, 146, 60, 0.28)',
		expenseFillFaint: 'rgba(251, 146, 60, 0.04)',
		expenseFillMuted: 'rgba(251, 146, 60, 0.12)',
		expenseFillMutedFaint: 'rgba(251, 146, 60, 0.02)',
		warning: 'rgba(240, 180, 41, 0.8)', // amber-400
	},
} as const;

// ============================================================================
// Date/Time Utilities
// ============================================================================

export function toUtcMsFromIsoDate(date: string): number | null {
	const ms = Date.parse(`${date}T00:00:00Z`);
	return Number.isFinite(ms) ? ms : null;
}

export function toMonthIndex(month: string): number | null {
	if (!/^\d{4}-\d{2}$/.test(month)) {
		return null;
	}

	const year = Number(month.slice(0, 4));
	const monthNum = Number(month.slice(5, 7));
	if (!Number.isFinite(year) || !Number.isFinite(monthNum) || monthNum < 1 || monthNum > 12) {
		return null;
	}

	return year * 12 + (monthNum - 1);
}

// ============================================================================
// Downsampling
// ============================================================================

export function chooseDownsampleStepDays(spanDays: number): number {
	if (spanDays > 365) {
		return 7;
	}
	if (spanDays > 180) {
		return 3;
	}
	if (spanDays > 90) {
		return 2;
	}
	return 1;
}

export function chooseDownsampleStepMonths(spanMonths: number): number {
	if (spanMonths >= 84) {
		return 3;
	}
	if (spanMonths >= 36) {
		return 2;
	}
	return 1;
}

export function downsampleByMinStep<T>(points: T[], getIndex: (point: T) => number | null, minStep: number): T[] {
	if (minStep <= 1 || points.length <= 2) {
		return points;
	}

	const first = points[0];
	const last = points[points.length - 1];
	if (!first || !last) {
		return points;
	}

	const firstIndex = getIndex(first);
	if (firstIndex === null) {
		return points;
	}

	const result: T[] = [first];
	let lastKeptIndex = firstIndex;

	for (let i = 1; i < points.length - 1; i++) {
		const point = points[i];
		if (!point) {
			continue;
		}

		const index = getIndex(point);
		if (index === null) {
			continue;
		}

		if (index - lastKeptIndex >= minStep) {
			result.push(point);
			lastKeptIndex = index;
		}
	}

	if (result[result.length - 1] !== last) {
		result.push(last);
	}

	return result;
}

// ============================================================================
// Search
// ============================================================================

/**
 * Binary search to find the index of the data point with time closest to targetTime.
 * Data must be sorted by time in ascending order.
 *
 * @param data - Array of data points
 * @param getTime - Function to extract ISO date string from a data point
 * @param targetTime - Target time as ISO date string (YYYY-MM-DD)
 * @returns Index of nearest point, or null if data is empty
 */
export function findNearestTimeIndex<T>(data: T[], getTime: (item: T) => string, targetTime: string): number | null {
	if (data.length === 0) {
		return null;
	}

	if (data.length === 1) {
		return 0;
	}

	const targetMs = new Date(targetTime).getTime();

	// Binary search to find insertion point
	let low = 0;
	let high = data.length - 1;

	while (low < high) {
		const mid = Math.floor((low + high) / 2);
		const midItem = data[mid];
		if (!midItem) {
			break;
		}
		const midTime = new Date(getTime(midItem)).getTime();

		if (midTime < targetMs) {
			low = mid + 1;
		} else {
			high = mid;
		}
	}

	// low is now the index of the first element >= targetTime
	// Compare with the element before it to find the closest
	if (low === 0) {
		return 0;
	}

	if (low >= data.length) {
		return data.length - 1;
	}

	const prevItem = data[low - 1];
	const currItem = data[low];
	if (!prevItem || !currItem) {
		return low;
	}

	const prevTime = new Date(getTime(prevItem)).getTime();
	const currTime = new Date(getTime(currItem)).getTime();

	const prevDiff = Math.abs(targetMs - prevTime);
	const currDiff = Math.abs(currTime - targetMs);

	return prevDiff <= currDiff ? low - 1 : low;
}

// ============================================================================
// Color Utilities
// ============================================================================

/**
 * Converts a hex color to rgba with specified alpha.
 * Falls back to returning the original color if not a valid hex format.
 */
export function asRgba(color: string, alpha: number): string {
	if (/^#[0-9a-fA-F]{6}$/.test(color)) {
		const r = Number.parseInt(color.slice(1, 3), 16);
		const g = Number.parseInt(color.slice(3, 5), 16);
		const b = Number.parseInt(color.slice(5, 7), 16);
		return `rgba(${r}, ${g}, ${b}, ${alpha})`;
	}

	return color;
}

// ============================================================================
// Investment Projections
// ============================================================================

/**
 * Projects investment value over time using continuous compounding.
 * Accounts for time-based growth and incremental contributions.
 *
 * @param contributions - Array of contribution points with cumulative amounts
 * @param annualRate - Annual return rate as decimal (e.g., 0.08 for 8%)
 * @returns Array of projected values at each contribution date
 */
export function projectInvestmentSeries(contributions: InvestmentPoint[], annualRate: number): MoneyPoint[] {
	const first = contributions[0];
	if (!first) {
		return [];
	}

	let prevMs = toUtcMsFromIsoDate(first.date);
	if (prevMs === null) {
		return contributions.map((p) => ({ date: p.date, valueMinor: p.contributionsMinor }));
	}

	let prevContribMinor = first.contributionsMinor;
	let valueMinor = first.contributionsMinor;

	const result: MoneyPoint[] = [{ date: first.date, valueMinor }];

	for (let i = 1; i < contributions.length; i++) {
		const point = contributions[i];
		if (!point) {
			continue;
		}

		const ms = toUtcMsFromIsoDate(point.date);
		if (ms === null) {
			continue;
		}

		const dt = ms - prevMs;
		const growthFactor = dt > 0 ? Math.exp(annualRate * (dt / YEAR_MS)) : 1;

		const deltaMinor = point.contributionsMinor - prevContribMinor;
		valueMinor = Math.round(valueMinor * growthFactor) + deltaMinor;

		result.push({ date: point.date, valueMinor });

		prevMs = ms;
		prevContribMinor = point.contributionsMinor;
	}

	return result;
}

// ============================================================================
// Runway Projection
// ============================================================================

/**
 * Projects balance forward over time assuming a constant monthly burn rate.
 *
 * @param currentBalanceMinor - Starting balance in minor units (pence)
 * @param monthlyBurnMinor - Monthly burn rate in minor units
 * @param months - Number of months to project forward (default 24)
 * @returns Array of projection points with month index, date, and projected balance
 */
export function projectRunway(currentBalanceMinor: number, monthlyBurnMinor: number, months = 24): ProjectionPoint[] {
	const points: ProjectionPoint[] = [];
	const now = new Date();

	for (let i = 0; i <= months; i++) {
		const projectedDate = new Date(now);
		projectedDate.setMonth(projectedDate.getMonth() + i);

		points.push({
			month: i,
			date: projectedDate.toISOString().slice(0, 10),
			balanceMinor: Math.max(0, currentBalanceMinor - monthlyBurnMinor * i),
		});
	}

	return points;
}

// ============================================================================
// Cash Assets Merging
// ============================================================================

function collectUniqueDates(seriesMap: Record<string, BalancePoint[] | undefined>, accountIds: string[]): string[] {
	const dateSet = new Set<string>();
	for (const accountId of accountIds) {
		const series = seriesMap[accountId];
		if (series) {
			for (const point of series) {
				dateSet.add(point.date);
			}
		}
	}
	return Array.from(dateSet).sort();
}

function buildBalanceLookups(seriesMap: Record<string, BalancePoint[] | undefined>, accountIds: string[]): Map<string, Map<string, number>> {
	const lookups = new Map<string, Map<string, number>>();
	for (const accountId of accountIds) {
		const lookup = new Map<string, number>();
		const series = seriesMap[accountId];
		if (series) {
			for (const point of series) {
				lookup.set(point.date, point.balanceMinor);
			}
		}
		lookups.set(accountId, lookup);
	}
	return lookups;
}

/**
 * Merges multiple balance series into a unified timeline for multi-line charts.
 * Uses forward-fill: if an account has no data for a given date, the last known
 * balance carries forward. Returns null for accounts with no data at all on a date.
 *
 * @param seriesMap - Map of account IDs to their balance series
 * @returns Unified array of points with per-account balances
 */
export function mergeBalanceSeries(seriesMap: Record<string, BalancePoint[] | undefined>): CashAssetsPoint[] {
	const accountIds = Object.keys(seriesMap);
	if (accountIds.length === 0) {
		return [];
	}

	const dates = collectUniqueDates(seriesMap, accountIds);
	if (dates.length === 0) {
		return [];
	}

	const lookups = buildBalanceLookups(seriesMap, accountIds);
	const lastKnown = new Map<string, number | null>(accountIds.map((id) => [id, null]));
	const result: CashAssetsPoint[] = [];

	for (const date of dates) {
		const accounts: Record<string, number | null> = {};
		for (const accountId of accountIds) {
			const lookup = lookups.get(accountId);
			const balance = lookup?.get(date);
			if (balance !== undefined) {
				lastKnown.set(accountId, balance);
			}
			accounts[accountId] = lastKnown.get(accountId) ?? null;
		}
		result.push({ date, accounts });
	}

	return result;
}
