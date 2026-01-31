<script lang="ts">
	import { onMount } from 'svelte';

	import ProjectionChart from '$lib/charts/ProjectionChart.svelte';
	import SeriesChart from '$lib/charts/SeriesChart.svelte';
	import Header from '$lib/Header.svelte';
	import { theme } from '$lib/theme.svelte';
	import { type BalancePoint, type CashAssetsPoint, mergeBalanceSeries, projectRunway } from '$lib/charts/utils';
	import type { ChartAccount } from './+page.server';

	type GroupId = string;

	let { data } = $props();

	let isMobile = $state(false);

	onMount(() => {
		const mobileQuery = window.matchMedia('(max-width: 640px)');
		isMobile = mobileQuery.matches;
		const handleMobileChange = (e: MediaQueryListEvent) => {
			isMobile = e.matches;
		};
		mobileQuery.addEventListener('change', handleMobileChange);

		return () => {
			mobileQuery.removeEventListener('change', handleMobileChange);
		};
	});

	const availableGroups = $derived(data.availableGroups);
	const groupMetadata = $derived(data.groupMetadata ?? {});
	const chartAccounts = $derived(data.chartAccounts as ChartAccount[]);

	// Build account lines config dynamically from server data
	const accountLines = $derived.by(() => {
		const lines: Record<string, { label: string; shade: number }> = {};
		const total = chartAccounts.length;
		chartAccounts.forEach((account, index) => {
			// Shade from 1.0 (first) to 0.2 (last)
			const shade = total > 1 ? 1.0 - (index * 0.8) / (total - 1) : 1.0;
			lines[account.id] = { label: account.label, shade };
		});
		return lines;
	});

	const accountBalanceSeries = $derived(data.accountBalanceSeries as Record<string, BalancePoint[]>);

	let overviewChartHeight = $state(400);
	let projectionChartHeight = $state(400);

	const moneyFormatter = new Intl.NumberFormat('en-GB', { style: 'currency', currency: 'GBP' });

	function getLineColor(shade: number): string {
		return theme.resolved === 'dark' ? `rgba(230, 230, 232, ${shade})` : `rgba(55, 65, 81, ${shade})`;
	}

	// Merge all account balance series into a unified timeline
	const mergedSeries = $derived.by(() => {
		const seriesMap: Record<string, BalancePoint[] | undefined> = {};
		for (const account of chartAccounts) {
			seriesMap[account.id] = accountBalanceSeries[account.id];
		}
		return mergeBalanceSeries(seriesMap);
	});

	// Build series definitions for the chart
	const seriesDefinitions = $derived(
		chartAccounts.map((account) => {
			const config = accountLines[account.id];
			return {
				key: account.id,
				color: getLineColor(config?.shade ?? 0.5),
				getValue: (p: CashAssetsPoint) => (p.accounts[account.id] ?? 0) / 100,
			};
		}),
	);

	function formatHover(point: CashAssetsPoint): string {
		const dateStr = new Date(point.date).toLocaleDateString('en-GB', {
			day: 'numeric',
			month: 'short',
			year: 'numeric',
		});

		const lines = chartAccounts
			.filter((account) => point.accounts[account.id] !== null)
			.map((account) => {
				const config = accountLines[account.id];
				const value = moneyFormatter.format((point.accounts[account.id] ?? 0) / 100);
				return `${config?.label ?? account.id}: ${value}`;
			});

		return [dateStr, ...lines].join('\n');
	}

	// Runway projection
	const projection = $derived(data.projection as {
		currentLiquidMinor: number;
		currentBurnMinor: number;
		minimumBurnMinor: number;
		thresholdMajor?: number;
		warningLineMajor?: number;
	});

	const currentBurnProjection = $derived(projectRunway(projection.currentLiquidMinor, projection.currentBurnMinor, 24));
	const minimumBurnProjection = $derived(projectRunway(projection.currentLiquidMinor, projection.minimumBurnMinor, 24));
</script>

<svelte:head>
	<title>Finances | Overview</title>
</svelte:head>

<main class="h-svh overflow-auto box-border px-2.5 pb-2.5 flex flex-col gap-2">
	<h1 class="sr-only">Financial Overview</h1>
	<Header activePage="overview" activeGroup={availableGroups[0]} onGroupChange={() => {}} availableGroups={availableGroups} {groupMetadata} allGroupsActive />

	<article class="border border-border bg-panel p-2.5 flex flex-col gap-2 fade-in flex-1 min-h-0">
		<header class="flex items-center justify-between gap-2.5">
			<div>
				<h2 class="font-normal text-sm uppercase tracking-widest">Account Balances</h2>
				<div class="text-sm mt-0.5 leading-snug uppercase tracking-wider text-muted">
					All accounts over time
				</div>
			</div>
		</header>

		<div class="flex-1 min-h-0 overflow-hidden" bind:clientHeight={overviewChartHeight}>
			<SeriesChart
				data={mergedSeries}
				getDate={(p: CashAssetsPoint) => p.date}
				series={seriesDefinitions}
				formatHover={formatHover}
				height={overviewChartHeight}
				compact={isMobile}
				curve={true}
				showRawOverlay={false}
			/>
		</div>
	</article>

	<article class="border border-border bg-panel p-2.5 flex flex-col gap-2 fade-in flex-1 min-h-0">
		<header class="flex items-center justify-between gap-2.5">
			<div>
				<h2 class="font-normal text-sm uppercase tracking-widest">Runway Projection</h2>
				<div class="text-sm mt-0.5 leading-snug uppercase tracking-wider text-muted">
					24-month forward projection
				</div>
			</div>
			<div class="text-lg font-normal">
				{moneyFormatter.format(projection.currentLiquidMinor / 100)}
			</div>
		</header>

		<div class="flex-1 min-h-0 overflow-hidden" bind:clientHeight={projectionChartHeight}>
			<ProjectionChart
				currentBurn={currentBurnProjection}
				minimumBurn={minimumBurnProjection}
				{...(projection.thresholdMajor !== undefined ? { threshold: projection.thresholdMajor } : {})}
				{...(projection.warningLineMajor !== undefined ? { warningLine: projection.warningLineMajor } : {})}
				height={projectionChartHeight}
				compact={isMobile}
			/>
		</div>
	</article>
</main>
