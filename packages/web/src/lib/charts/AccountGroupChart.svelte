<script lang="ts">
	import LineChart from './LineChart.svelte';
	import SeriesChart from './SeriesChart.svelte';
	import { theme } from '$lib/theme.svelte';
	import {
		type AnnualReturns,
		type BalancePoint,
		type InvestmentPoint,
		type ValuePoint,
		projectInvestmentSeries,
	} from './utils';

	type AccountConfig = {
		id: string;
		label: string;
	};

	type Props = {
		label: string;
		accounts: AccountConfig[];
		balanceSeries: Record<string, BalancePoint[]>;
		contributionSeries?: Record<string, InvestmentPoint[]>;
		investmentReturns?: AnnualReturns;
		latestBalances: Record<string, { balanceMinor: number | null }>;
		height?: number;
	};

	let {
		label,
		accounts,
		balanceSeries,
		contributionSeries,
		investmentReturns,
		latestBalances,
		height = 240,
	}: Props = $props();

	// Track selected account (default to first)
	let selectedAccountId = $state('');

	$effect(() => {
		// Reset to first account when accounts prop changes
		selectedAccountId = accounts[0]?.id ?? '';
	});

	// Derive current data for the selected account
	const currentBalanceData = $derived(balanceSeries[selectedAccountId] ?? []);
	const currentContributionData = $derived(contributionSeries?.[selectedAccountId] ?? []);
	const hasContributionData = $derived(currentContributionData.length > 0);
	// Investment accounts are identified by having contribution data
	const isInvestmentAccount = $derived(hasContributionData);

	// Current balance for header
	const currentBalance = $derived(latestBalances[selectedAccountId]?.balanceMinor ?? null);

	// Theme-aware line colors
	const lineColors = $derived({
		primary: theme.resolved === 'dark' ? '#e6e6e8' : '#374151',
		muted: theme.resolved === 'dark' ? 'rgba(230, 230, 232, 0.55)' : 'rgba(55, 65, 81, 0.7)',
		faint: theme.resolved === 'dark' ? 'rgba(230, 230, 232, 0.26)' : 'rgba(55, 65, 81, 0.4)',
	});

	const moneyFormatter = new Intl.NumberFormat('en-GB', { style: 'currency', currency: 'GBP' });

	function formatMoney(minor: number | null): string {
		if (minor === null) return '—';
		return moneyFormatter.format(minor / 100);
	}

	// Helper to convert balance points to value points for investment chart
	function toValuePoints(series: BalancePoint[]): ValuePoint[] {
		return series.map((p) => ({ date: p.date, valueMinor: p.balanceMinor }));
	}

	// Pre-build Map for O(1) lookups by date
	function toDateMap<T extends { date: string }>(items: T[]): Map<string, T> {
		return new Map(items.map((item) => [item.date, item]));
	}

	// Investment projections
	const valuesMap = $derived(toDateMap(toValuePoints(currentBalanceData)));
	const returns = $derived(investmentReturns ?? { low: 0.04, mid: 0.08, high: 0.12 });
	const lowProjection = $derived(projectInvestmentSeries(currentContributionData, returns.low));
	const lowProjectionMap = $derived(toDateMap(lowProjection));
	const highProjection = $derived(projectInvestmentSeries(currentContributionData, returns.high));
	const highProjectionMap = $derived(toDateMap(highProjection));
</script>

<article class="border border-border bg-panel p-2.5 flex flex-col gap-2">
	<header class="flex items-center justify-between gap-2.5">
		<div class="flex gap-3">
			{#each accounts as account (account.id)}
				<button
					type="button"
					class="pb-0.5 border-b text-[12px] uppercase tracking-widest cursor-pointer transition-colors duration-150
						   {selectedAccountId === account.id
						? 'border-text text-text'
						: 'border-transparent text-muted hover:text-text hover:border-text/50'}"
					onclick={() => (selectedAccountId = account.id)}
				>
					{account.label}
				</button>
			{/each}
		</div>
		<div class="text-sm font-normal">{formatMoney(currentBalance)}</div>
	</header>

	{#if isInvestmentAccount && hasContributionData}
		<!-- Investment account with contribution + projection bands -->
		<div class="overflow-hidden" style:height="{height}px">
			<SeriesChart
				data={currentContributionData}
				getDate={(p) => p.date}
				series={[
					{
						key: 'contributions',
						color: lineColors.muted,
						lineStyle: 'dotted',
						lineWidth: 1,
						getValue: (p) => p.contributionsMinor / 100,
						lastValueVisible: false,
						priceLineVisible: false,
					},
					{
						key: 'value',
						color: lineColors.primary,
						getValue: (p) => {
							const match = valuesMap.get(p.date);
							return match ? match.valueMinor / 100 : p.contributionsMinor / 100;
						},
					},
					{
						key: 'low',
						color: lineColors.faint,
						lineStyle: 'dashed',
						lineWidth: 1,
						getValue: (p) => {
							const match = lowProjectionMap.get(p.date);
							return match ? match.valueMinor / 100 : 0;
						},
						lastValueVisible: false,
						priceLineVisible: false,
						crosshairMarkerVisible: false,
					},
					{
						key: 'high',
						color: lineColors.faint,
						lineStyle: 'dashed',
						lineWidth: 1,
						getValue: (p) => {
							const match = highProjectionMap.get(p.date);
							return match ? match.valueMinor / 100 : 0;
						},
						lastValueVisible: false,
						priceLineVisible: false,
						crosshairMarkerVisible: false,
					},
				]}
				formatHover={(p) => {
					const parts = [];
					const valueMatch = valuesMap.get(p.date);
					if (valueMatch) {
						parts.push(`Value ${moneyFormatter.format(valueMatch.valueMinor / 100)}`);
					}
					parts.push(`Contrib ${moneyFormatter.format(p.contributionsMinor / 100)}`);
					return parts.length > 0 ? parts.join(' | ') : '';
				}}
				{height}
				compact
			/>
		</div>
	{:else if currentBalanceData.length > 0}
		<!-- Regular balance chart -->
		<div class="overflow-hidden" style:height="{height}px">
			<LineChart
				data={currentBalanceData}
				getValue={(p) => p.balanceMinor / 100}
				getDate={(p) => p.date}
				formatValue={(v) => moneyFormatter.format(v)}
				lineColor={lineColors.primary}
				{height}
				compact
			/>
		</div>
	{:else}
		<!-- No data state -->
		<div
			class="border border-dashed border-white/[0.16] grid place-items-center text-white/55 text-sm select-none"
			style:height="{height}px"
		>
			No balance data.
		</div>
	{/if}
</article>
