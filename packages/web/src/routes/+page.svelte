<script lang="ts">
	import { onMount } from 'svelte';

	import AccountGroupChart from '$lib/charts/AccountGroupChart.svelte';
	import Sankey from '$lib/charts/Sankey.svelte';
	import SeriesChart from '$lib/charts/SeriesChart.svelte';
	import Treemap from '$lib/charts/Treemap.svelte';
	import { SANKEY_PALETTE, type SankeyNode, type SankeyLink, type TreemapDataItem } from '$lib/charts/echarts';
	import Header from '$lib/Header.svelte';
	import { theme } from '$lib/theme.svelte';
	import { type BalancePoint, type CashflowPoint, type InvestmentPoint, SEMANTIC_COLORS } from '$lib/charts/utils';

	type GroupId = string;

	type AccountSubtype = 'checking' | 'savings' | 'investment';

	type ApiAccount = {
		id: string;
		name: string;
		kind: string;
		provider: string;
		subtype?: AccountSubtype;
		currency: string;
		sortOrder: number;
		active: boolean;
		latestBalance: {
			accountId: string;
			date: string | null;
			balanceMinor: number | null;
		};
	};

	type RunwayPoint = { runwayMonths: number; isNetPositive?: boolean; medianExpenseMinor?: number };

	type ReserveBreakdownPoint = {
		date: string;
		balanceMinor: number;
		taxReserveMinor: number;
		expenseReserveMinor: number;
		availableMinor: number;
	};

	type SankeyFlowData = {
		nodes: Array<{ name: string; category: 'income' | 'asset' | 'expense' }>;
		links: Array<{ source: string; target: string; value: number }>;
	};

	type ExpenseNode = {
		accountId: string;
		name: string;
		totalMinor: number;
		children: ExpenseNode[];
	};

	let { data } = $props();

	// Use UI config from server (loaded from TOML)
	const uiConfig = $derived(data.config.ui);
	const GROUPS = $derived(uiConfig.groups);
	const GROUP_IDS = $derived(Object.keys(GROUPS));
	const GROUP_COLUMN_ORDER = $derived(uiConfig.groupColumnOrder);
	const ACCOUNT_GROUP_CONFIG = $derived(uiConfig.accountGroupConfig);
	const GROUP_METADATA = $derived(uiConfig.groupMetadata ?? {});

	// Initialize group to first configured group (fallback to 'personal' for backwards compat)
	let group: GroupId = $state('');
	let isLargeScreen = $state(false);

	$effect(() => {
		if (!group) {
			group = data.config.ui.groupColumnOrder[0] ?? 'personal';
		}
	});

	// Map SSR data to component state (use $derived to maintain reactivity during hydration)
	const accounts: ApiAccount[] = $derived(data.accounts);
	const apiConfig = $derived(data.config);
	const groupCashflowSeries = $derived(data.groupCashflowSeries as Record<string, CashflowPoint[]>);
	const accountBalanceSeries = $derived(data.accountBalanceSeries as Record<string, BalancePoint[]>);
	const accountContributionSeries = $derived(data.accountContributionSeries as Record<string, InvestmentPoint[]>);
	const groupRunway = $derived(data.groupRunway as Record<string, RunwayPoint | null>);
	const groupReserveBreakdown = $derived(data.groupReserveBreakdown as Record<string, ReserveBreakdownPoint[]>);
	const groupCashFlowData = $derived(data.groupCashFlowData as Record<string, SankeyFlowData>);
	const groupExpenseHierarchy = $derived(data.groupExpenseHierarchy as Record<string, ExpenseNode[]>);

	const colorScheme = $derived(theme.resolved);

	// Build latestBalances map for AccountGroupChart
	const latestBalances = $derived.by(() => {
		const result: Record<string, { balanceMinor: number | null }> = {};
		for (const account of accounts) {
			result[account.id] = { balanceMinor: account.latestBalance.balanceMinor };
		}
		return result;
	});

	function formatMoney(minor: number | null): string {
		if (minor === null) {
			return '—';
		}

		const value = minor / 100;
		return value.toLocaleString('en-GB', { style: 'currency', currency: 'GBP' });
	}

	function formatMoneyRounded(minor: number | null): string {
		if (minor === null) {
			return '—';
		}

		const pounds = minor / 100;
		const rounded = Math.floor(pounds / 100) * 100;
		return rounded.toLocaleString('en-GB', { style: 'currency', currency: 'GBP', maximumFractionDigits: 0 });
	}

	const moneyFormatter = new Intl.NumberFormat('en-GB', { style: 'currency', currency: 'GBP' });

	// Theme-aware line colors for charts
	const lineColors = $derived({
		primary: theme.resolved === 'dark' ? '#e6e6e8' : '#374151',
		muted: theme.resolved === 'dark' ? 'rgba(230, 230, 232, 0.55)' : 'rgba(55, 65, 81, 0.7)',
		faint: theme.resolved === 'dark' ? 'rgba(230, 230, 232, 0.26)' : 'rgba(55, 65, 81, 0.4)',
	});

	function getAccountsForGroup(targetGroup: GroupId): ApiAccount[] {
		const ids = new Set(GROUPS[targetGroup].accountIds);
		return accounts.filter((a) => ids.has(a.id));
	}

	// Helper to get account balance by subtype within a group
	function getPersonalAccountBalances() {
		const personalAccounts = getAccountsForGroup('personal');

		// Sum balances by subtype (accounts can be tagged in config)
		let checkingBalance = 0;
		let savingsBalance = 0;
		let investmentBalance = 0;

		for (const account of personalAccounts) {
			const balance = account.latestBalance.balanceMinor ?? 0;
			switch (account.subtype) {
				case 'checking':
					checkingBalance += balance;
					break;
				case 'savings':
					savingsBalance += balance;
					break;
				case 'investment':
					investmentBalance += balance;
					break;
				default:
					// Accounts without subtype are treated as checking
					checkingBalance += balance;
					break;
			}
		}

		return { checkingBalance, savingsBalance, investmentBalance };
	}

	function getGroupTotalLatestMinor(targetGroup: GroupId): number | null {
		let total = 0;
		let any = false;

		for (const account of getAccountsForGroup(targetGroup)) {
			const balanceMinor = account.latestBalance.balanceMinor;
			if (balanceMinor === null) {
				continue;
			}

			total += balanceMinor;
			any = true;
		}

		return any ? total : null;
	}

	function getLastFullMonthNet(groupId: GroupId): number | null {
		const series = groupCashflowSeries[groupId];
		if (!series || series.length < 2) {
			return null;
		}
		return series[series.length - 2].netMinor;
	}

	function get3MonthTrendPositive(groupId: GroupId): boolean | null {
		const series = groupCashflowSeries[groupId];
		if (!series || series.length < 2) {
			return null;
		}
		const last3 = series.slice(-4, -1);
		if (last3.length === 0) {
			return null;
		}
		const avgNet = last3.reduce((sum, p) => sum + p.netMinor, 0) / last3.length;
		return avgNet > 0;
	}

	function getAnomalyMonths(groupId: GroupId): { count: number; recent: string[] } {
		const series = groupCashflowSeries[groupId];
		if (!series || series.length < 4) {
			return { count: 0, recent: [] };
		}
		// Look at last 12 months (excluding current partial month)
		const last12 = series.slice(-13, -1);
		const anomalies = last12.filter((p) => p.expenseDeviationRatio !== null && (p.expenseDeviationRatio > 1.2 || p.expenseDeviationRatio < 0.8));
		return {
			count: anomalies.length,
			recent: anomalies.slice(-3).map((p) => p.month),
		};
	}

	function getLastTwoMonthsData(groupId: GroupId): { current: CashflowPoint | null; previous: CashflowPoint | null } {
		const series = groupCashflowSeries[groupId];
		if (!series || series.length < 2) {
			return { current: null, previous: null };
		}
		// Last complete month and the one before
		return {
			current: series[series.length - 2] ?? null,
			previous: series[series.length - 3] ?? null,
		};
	}

	function getMonthOverMonthChange(current: number, previous: number): number | null {
		if (previous === 0) return null;
		return Math.round(((current - previous) / Math.abs(previous)) * 100);
	}

	function handleGroupChange(newGroup: GroupId) {
		group = newGroup;
	}

	// Convert SankeyFlowData to component format with colors
	function getSankeyNodes(groupId: GroupId): SankeyNode[] {
		const flowData = groupCashFlowData[groupId];
		if (!flowData) return [];

		const palette = SANKEY_PALETTE[colorScheme];
		return flowData.nodes.map((node) => ({
			name: node.name,
			itemStyle: {
				color: palette[node.category],
			},
		}));
	}

	function getSankeyLinks(groupId: GroupId): SankeyLink[] {
		const flowData = groupCashFlowData[groupId];
		if (!flowData) return [];
		return flowData.links;
	}

	// Convert ExpenseNode to TreemapDataItem
	function nodeToTreemapItem(node: ExpenseNode): TreemapDataItem {
		return {
			name: node.name,
			value: node.totalMinor,
			children: node.children.length > 0 ? node.children.map(nodeToTreemapItem) : undefined,
		};
	}

	function getTreemapData(groupId: GroupId): TreemapDataItem[] {
		const nodes = groupExpenseHierarchy[groupId] ?? [];
		// If the root is "Expenses", return its children directly for a cleaner view
		if (nodes.length === 1 && nodes[0]?.name === 'Expenses') {
			return nodes[0].children.map(nodeToTreemapItem);
		}
		return nodes.map(nodeToTreemapItem);
	}

	// Dynamic grid columns based on number of groups
	const gridColsClass = $derived(
		GROUP_COLUMN_ORDER.length === 2 ? '3xl:grid-cols-2' : '3xl:grid-cols-3'
	);

	onMount(() => {
		// Track large screen for allGroupsActive prop
		const mediaQuery = window.matchMedia('(min-width: 1700px)');
		isLargeScreen = mediaQuery.matches;
		const handleMediaChange = (e: MediaQueryListEvent) => {
			isLargeScreen = e.matches;
		};
		mediaQuery.addEventListener('change', handleMediaChange);

		return () => {
			mediaQuery.removeEventListener('change', handleMediaChange);
		};
	});
</script>

<svelte:head>
	<title>Finances | Dashboard</title>
</svelte:head>

<main class="h-dvh overflow-auto box-border px-2.5 pb-2.5 flex flex-col gap-1.5">
	<Header
		activePage="dashboard"
		activeGroup={group}
		onGroupChange={handleGroupChange}
		availableGroups={GROUP_COLUMN_ORDER}
		groupMetadata={GROUP_METADATA}
		allGroupsActive={isLargeScreen}
		loading={false}
		error={null}
	/>

	<section class="grid grid-cols-1 {gridColsClass} gap-1.5 fade-in" data-active-group={group}>
		{#each GROUP_COLUMN_ORDER as groupId (groupId)}
			<div class="flex gap-2 p-2.5 border border-border bg-panel" data-group={groupId}>
				<div class="flex-1 flex flex-col gap-[2px] min-w-0">
					<div class="text-2xs uppercase tracking-widest text-muted">Runway</div>
					<div class="text-lg text-text flex items-center gap-1">
						{#if groupRunway[groupId]?.isNetPositive}
							<span class="text-success font-medium">+</span>
						{:else if groupRunway[groupId]?.runwayMonths !== undefined}
							{Math.floor(groupRunway[groupId]?.runwayMonths ?? 0)}mo
						{:else}
							—
						{/if}
					</div>
				</div>
				<div class="flex-1 flex flex-col gap-[2px] min-w-0">
					<div class="text-2xs uppercase tracking-widest text-muted">Last Month</div>
					<div class="text-lg text-text flex items-center gap-1">
						{#if get3MonthTrendPositive(groupId) === true}
							<span class="size-1.5 shrink-0 bg-success"></span>
						{:else if get3MonthTrendPositive(groupId) === false}
							<span class="size-1.5 shrink-0 bg-error"></span>
						{/if}
						{formatMoneyRounded(getLastFullMonthNet(groupId))}
					</div>
				</div>
				<div class="flex-1 flex flex-col gap-[2px] min-w-0">
					<div class="text-2xs uppercase tracking-widest text-muted">Net Worth</div>
					<div class="text-lg text-text flex items-center gap-1">{formatMoneyRounded(getGroupTotalLatestMinor(groupId))}</div>
				</div>
				<div class="flex-1 flex flex-col gap-[2px] min-w-0">
					<div class="text-2xs uppercase tracking-widest text-muted">Med Spend</div>
					<div class="text-lg text-text flex items-center gap-1">{formatMoneyRounded(groupRunway[groupId]?.medianExpenseMinor ?? null)}</div>
				</div>
			</div>
		{/each}
	</section>

	<section class="grid grid-cols-1 {gridColsClass} gap-1.5 items-start" data-active-group={group}>
		{#each GROUP_COLUMN_ORDER as groupId (groupId)}
			<!-- Only render group sections if visible (large screen OR active tab) to avoid mounting hidden charts -->
			{#if isLargeScreen || groupId === group}
			<section class="min-w-0 flex flex-col gap-1.5 fade-in fade-in-delay-1" data-group={groupId} aria-label={GROUPS[groupId].label}>
				<!-- Asset Allocation -->
				{#if groupId === 'personal'}
					{@const personalBalances = getPersonalAccountBalances()}
					{@const checkingBalance = personalBalances.checkingBalance}
					{@const savingsBalance = personalBalances.savingsBalance}
					{@const investmentBalance = personalBalances.investmentBalance}
					{@const expenseBuffer = (groupRunway[groupId]?.medianExpenseMinor ?? 0) * 3}
					{@const availableCash = Math.max(0, checkingBalance - expenseBuffer)}
					{@const totalAssets = checkingBalance + savingsBalance + investmentBalance}
					{@const availPct = totalAssets > 0 ? (availableCash / totalAssets) * 100 : 0}
					{@const expPct = totalAssets > 0 ? (Math.min(expenseBuffer, checkingBalance) / totalAssets) * 100 : 0}
					{@const emergencyPct = totalAssets > 0 ? (savingsBalance / totalAssets) * 100 : 0}
					{@const investPct = totalAssets > 0 ? (investmentBalance / totalAssets) * 100 : 0}
					{@const isUnderBuffer = checkingBalance < expenseBuffer}
					<article class="border border-border bg-panel p-2.5 flex flex-col gap-2">
						<header class="flex items-center justify-between gap-2.5">
							<div class="font-normal text-sm uppercase tracking-widest">Asset Allocation</div>
							<div class="text-lg font-normal flex items-center gap-1">
								{#if isUnderBuffer}
									<span class="size-2 bg-error rounded-full"></span>
								{/if}
								{formatMoney(availableCash)}
							</div>
						</header>

						<!-- Stacked bar: Available | Expense Buffer | Emergency | Investment -->
						<div class="h-7 flex overflow-hidden">
							{#if availPct > 0}
								<div
									class="h-full flex items-center justify-center text-2xs font-medium gap-1 px-1"
									style:background={SEMANTIC_COLORS[colorScheme].incomeMuted}
									style:width="{availPct}%"
									title="Available: {formatMoney(availableCash)}"
								>
									{#if availPct > 20}{Math.round(availPct)}% ({formatMoneyRounded(availableCash)}){:else if availPct > 12}{Math.round(availPct)}%{/if}
								</div>
							{/if}
							{#if expPct > 0}
								<div
									class="h-full flex items-center justify-center text-2xs font-medium gap-1 px-1"
									style:width="{expPct}%"
									style:background={theme.resolved === 'dark' ? 'rgba(255,255,255,0.2)' : 'rgba(0,0,0,0.15)'}
									title="Expense Buffer (3mo): {formatMoney(expenseBuffer)}"
								>
									{#if expPct > 20}{Math.round(expPct)}% ({formatMoneyRounded(expenseBuffer)}){:else if expPct > 12}{Math.round(expPct)}%{/if}
								</div>
							{/if}
							{#if emergencyPct > 0}
								<div
									class="h-full flex items-center justify-center text-2xs font-medium gap-1 px-1"
									style:width="{emergencyPct}%"
									style:background={theme.resolved === 'dark' ? 'rgba(255,255,255,0.12)' : 'rgba(0,0,0,0.09)'}
									title="Emergency Fund: {formatMoney(savingsBalance)}"
								>
									{#if emergencyPct > 20}{Math.round(emergencyPct)}% ({formatMoneyRounded(savingsBalance)}){:else if emergencyPct > 12}{Math.round(emergencyPct)}%{/if}
								</div>
							{/if}
							{#if investPct > 0}
								<div
									class="h-full flex items-center justify-center text-2xs font-medium gap-1 px-1"
									style:width="{investPct}%"
									style:background={theme.resolved === 'dark' ? 'rgba(255,255,255,0.06)' : 'rgba(0,0,0,0.04)'}
									title="Investment: {formatMoney(investmentBalance)}"
								>
									{#if investPct > 20}{Math.round(investPct)}% ({formatMoneyRounded(investmentBalance)}){:else if investPct > 12}{Math.round(investPct)}%{/if}
								</div>
							{/if}
						</div>
					</article>
				{:else if groupReserveBreakdown[groupId].length > 0}
					{@const latestReserve = groupReserveBreakdown[groupId][groupReserveBreakdown[groupId].length - 1]}
					{@const totalBalance = latestReserve?.balanceMinor ?? 0}
					{@const taxReserve = latestReserve?.taxReserveMinor ?? 0}
					{@const expReserve = latestReserve?.expenseReserveMinor ?? 0}
					{@const available = latestReserve?.availableMinor ?? 0}
					{@const totalReserves = taxReserve + expReserve + available}
					{@const taxPct = totalReserves > 0 ? (taxReserve / totalReserves) * 100 : 0}
					{@const expPct = totalReserves > 0 ? (expReserve / totalReserves) * 100 : 0}
					{@const availPct = totalReserves > 0 ? (available / totalReserves) * 100 : 0}
					{@const isUnderBuffer = totalBalance < expReserve + taxReserve}
					<article class="border border-border bg-panel p-2.5 flex flex-col gap-2">
						<header class="flex items-center justify-between gap-2.5">
							<div class="font-normal text-sm uppercase tracking-widest">Asset Allocation</div>
							<div class="text-lg font-normal flex items-center gap-1">
								{#if isUnderBuffer}
									<span class="size-2 bg-error rounded-full"></span>
								{/if}
								{formatMoney(available)}
							</div>
						</header>

						<!-- Stacked bar with inline labels -->
						<div class="h-7 flex overflow-hidden">
							{#if availPct > 0}
								<div
									class="h-full flex items-center justify-center text-2xs font-medium gap-1 px-1"
									style:background={SEMANTIC_COLORS[colorScheme].incomeMuted}
									style:width="{availPct}%"
									title="Available: {formatMoney(available)}"
								>
									{#if availPct > 20}{Math.round(availPct)}% ({formatMoneyRounded(available)}){:else if availPct > 12}{Math.round(availPct)}%{/if}
								</div>
							{/if}
							{#if expPct > 0}
								<div
									class="h-full flex items-center justify-center text-2xs font-medium gap-1 px-1"
									style:width="{expPct}%"
									style:background={theme.resolved === 'dark' ? 'rgba(255,255,255,0.15)' : 'rgba(0,0,0,0.12)'}
									title="Expense Buffer: {formatMoney(expReserve)}"
								>
									{#if expPct > 20}{Math.round(expPct)}% ({formatMoneyRounded(expReserve)}){:else if expPct > 12}{Math.round(expPct)}%{/if}
								</div>
							{/if}
							{#if taxPct > 0}
								<div
									class="h-full flex items-center justify-center text-2xs font-medium gap-1 px-1"
									style:width="{taxPct}%"
									style:background={theme.resolved === 'dark' ? 'rgba(255,255,255,0.08)' : 'rgba(0,0,0,0.06)'}
									title="Tax Reserve: {formatMoney(taxReserve)}"
								>
									{#if taxPct > 20}{Math.round(taxPct)}% ({formatMoneyRounded(taxReserve)}){:else if taxPct > 12}{Math.round(taxPct)}%{/if}
								</div>
							{/if}
						</div>
					</article>
				{/if}

				<!-- Period Summary Card -->
				{#if getLastTwoMonthsData(groupId).current}
					{@const monthData = getLastTwoMonthsData(groupId)}
					{@const incomeChange = monthData.previous ? getMonthOverMonthChange(monthData.current.incomeMinor, monthData.previous.incomeMinor) : null}
					{@const expenseChange = monthData.previous ? getMonthOverMonthChange(monthData.current.expenseMinor, monthData.previous.expenseMinor) : null}
					{@const netChange = monthData.previous ? getMonthOverMonthChange(monthData.current.netMinor, monthData.previous.netMinor) : null}
					<article class="border border-border bg-panel p-2.5 flex flex-col gap-2">
						<header class="flex items-center justify-between gap-2.5">
							<div class="font-normal text-sm uppercase tracking-widest">Last Month</div>
							<div class="text-xs text-muted">{monthData.current.month}</div>
						</header>
						<div class="grid grid-cols-3 gap-2">
							<div class="flex flex-col gap-0.5">
								<div class="text-2xs uppercase tracking-widest text-muted">Income</div>
								<div class="text-sm tabular-nums text-success">{formatMoneyRounded(monthData.current.incomeMinor)}</div>
								{#if incomeChange !== null}
									<div class="text-2xs tabular-nums" class:text-success={incomeChange >= 0} class:text-error={incomeChange < 0}>
										{incomeChange >= 0 ? '+' : ''}{incomeChange}%
									</div>
								{/if}
							</div>
							<div class="flex flex-col gap-0.5">
								<div class="text-2xs uppercase tracking-widest text-muted">Expenses</div>
								<div class="text-sm tabular-nums text-error">{formatMoneyRounded(monthData.current.expenseMinor)}</div>
								{#if expenseChange !== null}
									<div class="text-2xs tabular-nums" class:text-error={expenseChange > 0} class:text-success={expenseChange <= 0}>
										{expenseChange >= 0 ? '+' : ''}{expenseChange}%
									</div>
								{/if}
							</div>
							<div class="flex flex-col gap-0.5">
								<div class="text-2xs uppercase tracking-widest text-muted">Net</div>
								<div class="text-sm tabular-nums" class:text-success={monthData.current.netMinor >= 0} class:text-error={monthData.current.netMinor < 0}>
									{monthData.current.netMinor >= 0 ? '+' : ''}{formatMoneyRounded(monthData.current.netMinor)}
								</div>
								{#if monthData.current.savingsRatePct !== null}
									<div class="text-2xs tabular-nums text-muted">
										{monthData.current.savingsRatePct >= 0 ? '+' : ''}{monthData.current.savingsRatePct.toFixed(0)}% rate
									</div>
								{/if}
							</div>
						</div>
					</article>
				{/if}

				<!-- Monthly Cashflow Trends -->
				<section class="border border-border bg-panel p-2.5 flex flex-col gap-2 min-h-[385px]">
					<header class="flex items-start justify-between gap-2.5">
						<div>
							<div class="font-normal text-sm uppercase tracking-widest">{GROUPS[groupId].label} cashflow</div>
							<div class="text-sm mt-0.5 leading-snug uppercase tracking-wider text-muted">Income / expense / net (monthly)</div>
						</div>
						{#if getAnomalyMonths(groupId).count > 0}
							{@const anomalies = getAnomalyMonths(groupId)}
							<div class="flex items-center gap-1.5 text-xs" title="Months with spending >20% above or below 6-month median">
								<span class="size-2 bg-pending rounded-full animate-pulse"></span>
								<span class="text-muted">{anomalies.count} anomal{anomalies.count === 1 ? 'y' : 'ies'}</span>
							</div>
						{/if}
					</header>

					{#if (groupCashflowSeries[groupId]?.length ?? 0) > 0}
						<div class="overflow-hidden h-[325px]">
							<SeriesChart
								data={groupCashflowSeries[groupId] ?? []}
								getDate={(p) => `${p.month}-01`}
								series={[
									{
										key: 'income',
										color: SEMANTIC_COLORS[colorScheme].income,
										getValue: (p) => p.incomeMinor / 100,
									},
									{
										key: 'expense',
										color: SEMANTIC_COLORS[colorScheme].expense,
										lineStyle: 'dashed',
										getValue: (p) => p.expenseMinor / 100,
									},
									{
										key: 'net',
										color: lineColors.primary,
										getValue: (p) => p.netMinor / 100,
									},
								]}
								formatHover={(p) => {
									const parts = [];
									parts.push(`In ${moneyFormatter.format(p.incomeMinor / 100)}`);
									parts.push(`Out ${moneyFormatter.format(p.expenseMinor / 100)}`);
									const net = p.netMinor / 100;
									const prefix = net >= 0 ? '+' : '';
									parts.push(`Net ${prefix}${moneyFormatter.format(net)}`);
									if (p.savingsRatePct !== null) {
										const ratePrefix = p.savingsRatePct >= 0 ? '+' : '';
										parts.push(`Rate ${ratePrefix}${p.savingsRatePct.toFixed(0)}%`);
									}
									if (p.expenseDeviationRatio !== null) {
										const devPct = Math.round((p.expenseDeviationRatio - 1) * 100);
										const devPrefix = devPct >= 0 ? '+' : '';
										const isAnomaly = p.expenseDeviationRatio > 1.2 || p.expenseDeviationRatio < 0.8;
										parts.push(`Spend ${devPrefix}${devPct}%${isAnomaly ? ' !' : ''}`);
									}
									return parts.join(' | ');
								}}
								timeUnit="month"
								height={325}
							/>
						</div>
					{:else}
						<div class="border border-dashed border-white/[0.16] h-[325px] grid place-items-center text-white/55 text-sm select-none">
							No cashflow data yet.
						</div>
					{/if}
				</section>

				<!-- Cash Flow Distribution (Sankey) -->
				{#if getSankeyNodes(groupId).length > 0 && getSankeyLinks(groupId).length > 0}
					<article class="border border-border bg-panel p-2.5 flex flex-col gap-2">
						<header class="font-normal text-sm uppercase tracking-widest">{GROUPS[groupId].label} Distribution</header>
						<div class="h-[350px]">
							<Sankey nodes={getSankeyNodes(groupId)} links={getSankeyLinks(groupId)} {colorScheme} />
						</div>
					</article>
				{/if}

				<!-- Expense Breakdown (Treemap) -->
				{#if getTreemapData(groupId).length > 0}
					<article class="border border-border bg-panel p-2.5 flex flex-col gap-2">
						<header class="font-normal text-sm uppercase tracking-widest">Expense Breakdown</header>
						<div class="h-[350px]">
							<Treemap data={getTreemapData(groupId)} {colorScheme} />
						</div>
					</article>
				{/if}

				<!-- Account Balances -->
				<AccountGroupChart
					label={ACCOUNT_GROUP_CONFIG[groupId].label}
					accounts={ACCOUNT_GROUP_CONFIG[groupId].accounts}
					balanceSeries={accountBalanceSeries}
					contributionSeries={accountContributionSeries}
					investmentReturns={apiConfig?.finance.investmentProjectionAnnualReturns}
					{latestBalances}
				/>
			</section>
			{/if}
		{/each}
	</section>
</main>

<style>
	@media (max-width: 1699px) {
		section[data-active-group] > section[data-group] {
			display: none;
		}

		section[data-active-group='personal'] > section[data-group='personal'] {
			display: flex;
		}

		section[data-active-group='business'] > section[data-group='business'] {
			display: flex;
		}

		section[data-active-group='joint'] > section[data-group='joint'] {
			display: flex;
		}

		section[data-active-group] > div[data-group] {
			display: none;
		}

		section[data-active-group='business'] > div[data-group='business'],
		section[data-active-group='personal'] > div[data-group='personal'],
		section[data-active-group='joint'] > div[data-group='joint'] {
			display: flex;
		}
	}
</style>
