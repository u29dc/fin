import { getAllAccountsDailyBalanceSeries, getConsolidatedDailyRunwaySeries, getLatestBalances } from '@fin/core';
import { getAllGroupMetadata, getAssetAccountIds, getConfig, getGroupIds, getLiquidAccountIds } from '@fin/core/config';
import { db } from '$lib/server/db';

const ALL_CHART_ACCOUNT_IDS = getAssetAccountIds();
const LIQUID_CHART_ACCOUNT_IDS = getLiquidAccountIds();

export type ChartAccount = {
	id: string;
	label: string;
	group: string;
	provider: string;
};

export function load() {
	const availableGroups = getGroupIds();
	const config = getConfig();
	const groupMetadataList = getAllGroupMetadata();
	const groupMetadata = Object.fromEntries(groupMetadataList.map((m) => [m.id, { label: m.label, icon: m.icon }]));

	// Get chart accounts with labels from config
	const chartAccounts: ChartAccount[] = config.accounts
		.filter((a) => a.type === 'asset')
		.map((a) => ({
			id: a.id,
			label: a.label ?? a.id.split(':').pop() ?? a.id,
			group: a.group,
			provider: a.provider,
		}));

	// Fetch all balance series in a single batched query
	const accountBalanceSeries = getAllAccountsDailyBalanceSeries(db, ALL_CHART_ACCOUNT_IDS, { limit: 10_000 });

	// Projection data for runway chart
	const latestBalances = getLatestBalances(db, LIQUID_CHART_ACCOUNT_IDS);
	const currentLiquidMinor = latestBalances.reduce((sum, b) => sum + (b.balanceMinor ?? 0), 0);

	const consolidatedRunway = getConsolidatedDailyRunwaySeries(db, { includeGroups: availableGroups });
	const latestRunway = consolidatedRunway[consolidatedRunway.length - 1];
	// Use actual median from data, or 0 if no data yet
	const currentBurnMinor = latestRunway?.burnRateMinor ?? 0;

	return {
		availableGroups,
		groupMetadata,
		chartAccounts,
		accountBalanceSeries,
		projection: {
			currentLiquidMinor,
			currentBurnMinor,
			// Minimum burn is now derived from data rather than hardcoded
			minimumBurnMinor: Math.round(currentBurnMinor * 0.6),
			// Runway chart thresholds from config (convert minor to major units for chart)
			thresholdMajor: config.financial.runway_threshold_minor ? config.financial.runway_threshold_minor / 100 : undefined,
			warningLineMajor: config.financial.runway_warning_minor ? config.financial.runway_warning_minor / 100 : undefined,
		},
	};
}
