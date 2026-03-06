<script lang="ts">
	import { onMount } from "svelte";
	import { goto } from "$app/navigation";
	import { page } from "$app/state";

	import AccountGroupChart from "$lib/charts/AccountGroupChart.svelte";
	import Sankey from "$lib/charts/Sankey.svelte";
	import SeriesChart from "$lib/charts/SeriesChart.svelte";
	import Treemap from "$lib/charts/Treemap.svelte";
	import { SANKEY_PALETTE } from "$lib/charts/palette";
	import type { SankeyLink, SankeyNode, TreemapDataItem } from "$lib/charts/types";
	import { SEMANTIC_COLORS, type BalancePoint, type CashflowPoint, type InvestmentPoint } from "$lib/charts/utils";
	import Header from "$lib/Header.svelte";
	import { theme } from "$lib/theme.svelte";
	import type {
		DashboardAccount,
		DashboardAllocationSnapshot,
		DashboardGroupSummary,
		DashboardPageData,
		DashboardRunwaySummary,
		ExpenseNode,
		SankeyFlowData,
	} from "$lib/server/dashboard";

	type GroupId = string;

	let { data }: { data: DashboardPageData } = $props();

	const uiConfig = $derived(data.config.ui);
	const GROUPS = $derived(uiConfig.groups);
	const GROUP_COLUMN_ORDER = $derived(uiConfig.groupColumnOrder);
	const ACCOUNT_GROUP_CONFIG = $derived(uiConfig.accountGroupConfig);
	const GROUP_METADATA = $derived(uiConfig.groupMetadata);
	const financeConfig = $derived(data.config.finance);
	const accounts = $derived(data.accounts as DashboardAccount[]);
	const groupCashflowSeries = $derived(data.groupCashflowSeries as Record<string, CashflowPoint[]>);
	const accountBalanceSeries = $derived(data.accountBalanceSeries as Record<string, BalancePoint[]>);
	const accountContributionSeries = $derived(data.accountContributionSeries as Record<string, InvestmentPoint[]>);
	const groupRunway = $derived(data.groupRunway as Record<string, DashboardRunwaySummary | null>);
	const groupAllocationSnapshots = $derived(
		data.groupAllocationSnapshots as Record<string, DashboardAllocationSnapshot | null>,
	);
	const groupCashFlowData = $derived(data.groupCashFlowData as Record<string, SankeyFlowData>);
	const groupExpenseHierarchy = $derived(data.groupExpenseHierarchy as Record<string, ExpenseNode[]>);
	const groupSummary = $derived(data.groupSummary as Record<string, DashboardGroupSummary>);
	const connection = $derived(data.connection);
	const colorScheme = $derived(theme.resolved);

	let group = $state<GroupId>("personal");
	let isLargeScreen = $state(false);
	let isMobile = $state(false);
	let initialized = $state(false);

	const latestBalances = $derived.by(() => {
		const result: Record<string, { balanceMinor: number | null }> = {};
		for (const account of accounts) {
			result[account.id] = { balanceMinor: account.latestBalance.balanceMinor };
		}
		return result;
	});

	const hasRenderableData = $derived.by(() =>
		GROUP_COLUMN_ORDER.some((groupId) => {
			return Boolean(
				groupSummary[groupId] ||
				(groupCashflowSeries[groupId]?.length ?? 0) > 0 ||
				(groupAllocationSnapshots[groupId]?.dashboard.segments.length ?? 0) > 0 ||
				(groupCashFlowData[groupId]?.nodes.length ?? 0) > 0 ||
				(groupExpenseHierarchy[groupId]?.length ?? 0) > 0 ||
				(ACCOUNT_GROUP_CONFIG[groupId]?.accounts.length ?? 0) > 0,
			);
		}),
	);

	const lineColors = $derived({
		primary: theme.resolved === "dark" ? "#e6e6e8" : "#374151",
		muted: theme.resolved === "dark" ? "rgba(230, 230, 232, 0.55)" : "rgba(55, 65, 81, 0.7)",
		faint: theme.resolved === "dark" ? "rgba(230, 230, 232, 0.26)" : "rgba(55, 65, 81, 0.4)",
	});

	const gridColsClass = $derived.by(() => {
		const count = GROUP_COLUMN_ORDER.length;
		if (count <= 1) return "";
		if (count === 2) return "3xl:grid-cols-2";
		if (count === 3) return "3xl:grid-cols-3";
		if (count === 4) return "3xl:grid-cols-4";
		return "3xl:grid-cols-[repeat(auto-fit,minmax(350px,1fr))]";
	});

	$effect.pre(() => {
		if (initialized) {
			return;
		}
		initialized = true;
		group = data.initialGroup ?? GROUP_COLUMN_ORDER[0] ?? "personal";
	});

	function formatMoney(minor: number | null): string {
		if (minor === null) {
			return "—";
		}
		return (minor / 100).toLocaleString("en-GB", { style: "currency", currency: "GBP" });
	}

	function formatMoneyRounded(minor: number | null): string {
		if (minor === null) {
			return "—";
		}
		return (minor / 100).toLocaleString("en-GB", {
			style: "currency",
			currency: "GBP",
			maximumFractionDigits: 0,
		});
	}

	function formatPercent(value: number | null): string {
		if (value === null || Number.isNaN(value)) {
			return "—";
		}
		return `${Math.round(value)}%`;
	}

	function getAccountsForGroup(targetGroup: GroupId): DashboardAccount[] {
		const ids = new Set(GROUPS[targetGroup]?.accountIds ?? []);
		return accounts.filter((account) => ids.has(account.id));
	}

	function getGroupTotalLatestMinor(targetGroup: GroupId): number | null {
		const balances = getAccountsForGroup(targetGroup)
			.map((account) => account.latestBalance.balanceMinor)
			.filter((value): value is number => value !== null);
		if (balances.length === 0) {
			return null;
		}
		return balances.reduce((sum, value) => sum + value, 0);
	}

	function getGroupSummaryEntry(targetGroup: GroupId): DashboardGroupSummary | null {
		return groupSummary[targetGroup] ?? null;
	}

	function getAnomalySummary(targetGroup: GroupId): { count: number; recent: string[] } {
		const summary = getGroupSummaryEntry(targetGroup);
		return {
			count: summary?.anomalyCountLast12Months ?? 0,
			recent: summary?.recentAnomalyMonths ?? [],
		};
	}

	function getLastTwoMonthsData(targetGroup: GroupId): { current: CashflowPoint | null; previous: CashflowPoint | null } {
		const series = groupCashflowSeries[targetGroup] ?? [];
		return {
			current: series.at(-1) ?? null,
			previous: series.at(-2) ?? null,
		};
	}

	function getPeriodSummary(targetGroup: GroupId): { current: CashflowPoint; previous: CashflowPoint | null } | null {
		const monthData = getLastTwoMonthsData(targetGroup);
		if (!monthData.current) {
			return null;
		}
		return {
			current: monthData.current,
			previous: monthData.previous,
		};
	}

	function getMonthOverMonthChange(current: number, previous: number): number | null {
		if (previous === 0) {
			return null;
		}
		return Math.round(((current - previous) / Math.abs(previous)) * 100);
	}

	function handleGroupChange(nextGroup: GroupId) {
		group = nextGroup;
		const url = new URL(page.url);
		url.searchParams.set("group", nextGroup);
		goto(url.toString(), { replaceState: true, noScroll: true });
	}

	function getSankeyNodes(groupId: GroupId): SankeyNode[] {
		const flowData = groupCashFlowData[groupId];
		if (!flowData) {
			return [];
		}
		const palette = SANKEY_PALETTE[colorScheme];
		return flowData.nodes.map((node) => ({
			name: node.name,
			itemStyle: {
				color: palette[node.category],
			},
		}));
	}

	function getSankeyLinks(groupId: GroupId): SankeyLink[] {
		return groupCashFlowData[groupId]?.links ?? [];
	}

	function nodeToTreemapItem(node: ExpenseNode): TreemapDataItem {
		return {
			name: node.name,
			value: node.totalMinor,
			...(node.children.length > 0 ? { children: node.children.map(nodeToTreemapItem) } : {}),
		};
	}

    function getTreemapData(groupId: GroupId): TreemapDataItem[] {
        const nodes = groupExpenseHierarchy[groupId] ?? [];
        if (nodes.length === 1 && nodes[0]?.name === 'Expenses' && nodes[0].children.length > 0) {
            return nodes[0].children.map(nodeToTreemapItem);
        }
        return nodes.map(nodeToTreemapItem);
    }

	function getAllocationSnapshot(groupId: GroupId): DashboardAllocationSnapshot | null {
		return groupAllocationSnapshots[groupId] ?? null;
	}

	function getAllocationSegmentStyle(bucket: string): string {
		const colors = SEMANTIC_COLORS[colorScheme];
		switch (bucket) {
			case "available_cash":
				return colors.incomeMuted;
			case "expense_reserve":
				return theme.resolved === "dark" ? "rgba(255,255,255,0.18)" : "rgba(0,0,0,0.14)";
			case "tax_reserve":
				return theme.resolved === "dark" ? "rgba(255,255,255,0.10)" : "rgba(0,0,0,0.08)";
			case "emergency_fund":
				return theme.resolved === "dark" ? "rgba(255,255,255,0.12)" : "rgba(0,0,0,0.10)";
			case "savings":
				return theme.resolved === "dark" ? "rgba(255,255,255,0.08)" : "rgba(0,0,0,0.06)";
			case "investment":
				return theme.resolved === "dark" ? "rgba(255,255,255,0.06)" : "rgba(0,0,0,0.04)";
			default:
				return theme.resolved === "dark" ? "rgba(255,255,255,0.05)" : "rgba(0,0,0,0.05)";
		}
	}

	onMount(() => {
		const largeScreenQuery = window.matchMedia("(min-width: 1700px)");
		const mobileQuery = window.matchMedia("(max-width: 640px)");

		const updateLargeScreen = (event: MediaQueryList | MediaQueryListEvent) => {
			isLargeScreen = event.matches;
		};
		const updateMobile = (event: MediaQueryList | MediaQueryListEvent) => {
			isMobile = event.matches;
		};

		updateLargeScreen(largeScreenQuery);
		updateMobile(mobileQuery);
		largeScreenQuery.addEventListener("change", updateLargeScreen);
		mobileQuery.addEventListener("change", updateMobile);

		return () => {
			largeScreenQuery.removeEventListener("change", updateLargeScreen);
			mobileQuery.removeEventListener("change", updateMobile);
		};
	});
</script>

<svelte:head>
	<title>Finances | Dashboard</title>
</svelte:head>

<main class="h-svh overflow-auto box-border px-2.5 pb-2.5 flex flex-col gap-1.5">
	<h1 class="sr-only">Financial Dashboard</h1>
	<Header
		activePage="dashboard"
		activeGroup={group}
		onGroupChange={handleGroupChange}
		availableGroups={GROUP_COLUMN_ORDER}
		groupMetadata={GROUP_METADATA}
		allGroupsActive={isLargeScreen}
		loading={connection.loading}
		error={connection.error}
		detail={connection.detail}
	/>

	{#if !hasRenderableData}
		<section class="border border-border bg-panel p-3 flex flex-col gap-2 fade-in">
			<h2 class="font-normal text-sm uppercase tracking-widest">Dashboard Waiting On Data</h2>
			<p class="text-sm leading-relaxed text-muted">
				{connection.detail}
			</p>
			<p class="text-2xs uppercase tracking-widest text-muted">
				No dashboard payloads are available yet. Start `fin-api` and ensure config and ledger health checks pass.
			</p>
		</section>
	{/if}

	<section class="grid grid-cols-1 {gridColsClass} gap-1.5 fade-in" data-active-group={group}>
		{#each GROUP_COLUMN_ORDER as groupId (groupId)}
			{#if isLargeScreen || groupId === group}
				{@const summary = getGroupSummaryEntry(groupId)}
				<div class="flex gap-2 p-2.5 border border-border bg-panel" data-group={groupId}>
					<div class="flex-1 flex flex-col gap-[2px] min-w-0">
						<h2 class="text-2xs uppercase tracking-widest text-muted">Runway</h2>
						<div class="text-lg text-text flex items-center gap-1 tabular-nums">
							{#if groupRunway[groupId]?.isNetPositive}
								<span class="text-success font-medium">+</span>
							{:else if groupRunway[groupId]}
								{Math.floor(groupRunway[groupId]?.runwayMonths ?? 0)}mo
							{:else}
								—
							{/if}
						</div>
					</div>
					<div class="flex-1 flex flex-col gap-[2px] min-w-0">
						<h2 class="text-2xs uppercase tracking-widest text-muted">Last Month</h2>
						<div class="text-lg text-text flex items-center gap-1 tabular-nums">
							{#if summary?.shortTermTrend === "positive"}
								<span class="size-1.5 shrink-0 bg-success"></span>
							{:else if summary?.shortTermTrend === "negative"}
								<span class="size-1.5 shrink-0 bg-error"></span>
							{:else if summary?.shortTermTrend === "flat"}
								<span class="size-1.5 shrink-0 bg-muted"></span>
							{/if}
							{formatMoneyRounded(summary?.lastFullMonthNetMinor ?? null)}
						</div>
					</div>
					<div class="flex-1 flex flex-col gap-[2px] min-w-0">
						<h2 class="text-2xs uppercase tracking-widest text-muted">Net Worth</h2>
						<div class="text-lg text-text flex items-center gap-1 tabular-nums">
							{formatMoneyRounded(summary?.netWorthMinor ?? getGroupTotalLatestMinor(groupId))}
						</div>
					</div>
					<div class="flex-1 flex flex-col gap-[2px] min-w-0">
						<h2 class="text-2xs uppercase tracking-widest text-muted">Med Spend</h2>
						<div class="text-lg text-text flex items-center gap-1 tabular-nums">
							{formatMoneyRounded(summary?.medianSpendMinor ?? groupRunway[groupId]?.medianExpenseMinor ?? null)}
						</div>
					</div>
				</div>
			{/if}
		{/each}
	</section>

	<section class="grid grid-cols-1 {gridColsClass} gap-1.5 items-start" data-active-group={group}>
		{#each GROUP_COLUMN_ORDER as groupId (groupId)}
			{#if isLargeScreen || groupId === group}
				{@const allocation = getAllocationSnapshot(groupId)}
				<section class="min-w-0 flex flex-col gap-1.5 fade-in fade-in-delay-1" data-group={groupId} aria-label={GROUPS[groupId]?.label ?? groupId}>
					{#if allocation}
						<article class="border border-border bg-panel p-2.5 flex flex-col gap-2">
							<header class="flex items-center justify-between gap-2.5">
								<div>
									<h2 class="font-normal text-sm uppercase tracking-widest">Asset Allocation</h2>
									<div class="text-2xs uppercase tracking-widest text-muted mt-0.5">
										{allocation.dashboard.basis === "personal_buffer" ? "Available / reserve / investment" : "Available / expense / tax"}
									</div>
								</div>
								<div class="text-sm font-normal flex items-center gap-1 tabular-nums">
									{#if allocation.dashboard.underReserved}
										<span class="size-2 bg-error rounded-full"></span>
									{/if}
									{formatMoney(allocation.dashboard.availableMinor)}
								</div>
							</header>

							<div class="h-7 flex overflow-hidden">
								{#each allocation.dashboard.segments as segment (segment.label)}
									{#if segment.sharePct > 0}
										<div
											class="h-full flex items-center justify-center text-2xs font-medium gap-1 px-1 min-w-0"
											style:background={getAllocationSegmentStyle(segment.bucket)}
											style:width={`${segment.sharePct}%`}
											title={`${segment.label}: ${formatMoney(segment.amountMinor)}`}
										>
											{#if segment.sharePct >= 20}
												<span class="truncate">{segment.label}</span>
												<span class="tabular-nums">{formatPercent(segment.sharePct)}</span>
											{:else if segment.sharePct >= 12}
												<span class="tabular-nums">{formatPercent(segment.sharePct)}</span>
											{/if}
										</div>
									{/if}
								{/each}
							</div>

							<div class="grid grid-cols-2 gap-2 md:grid-cols-3">
								{#each allocation.dashboard.segments as segment (segment.label)}
									<div class="flex items-start gap-2 min-w-0">
										<span class="size-2 rounded-full mt-1 shrink-0" style:background={getAllocationSegmentStyle(segment.bucket)}></span>
										<div class="min-w-0">
											<div class="text-2xs uppercase tracking-widest text-muted truncate">{segment.label}</div>
											<div class="text-sm tabular-nums truncate">{formatMoneyRounded(segment.amountMinor)}</div>
											<div class="text-2xs uppercase tracking-widest text-muted">{formatPercent(segment.sharePct)}</div>
										</div>
									</div>
								{/each}
							</div>
						</article>
					{:else}
						<article class="border border-border bg-panel p-2.5 flex flex-col gap-2">
							<h2 class="font-normal text-sm uppercase tracking-widest">Asset Allocation</h2>
							<div class="border border-dashed border-border-subtle min-h-32 grid place-items-center text-sm text-muted">
								No allocation data yet.
							</div>
						</article>
					{/if}

					{#if getPeriodSummary(groupId)}
						{@const periodSummary = getPeriodSummary(groupId)!}
						{@const current = periodSummary.current}
						{@const previous = periodSummary.previous}
						{@const incomeChange = previous ? getMonthOverMonthChange(current.incomeMinor, previous.incomeMinor) : null}
						{@const expenseChange = previous ? getMonthOverMonthChange(current.expenseMinor, previous.expenseMinor) : null}
						{@const netChange = previous ? getMonthOverMonthChange(current.netMinor, previous.netMinor) : null}
						<article class="border border-border bg-panel p-2.5 flex flex-col gap-2">
							<header class="flex items-center justify-between gap-2.5">
								<h2 class="font-normal text-sm uppercase tracking-widest">Last Month</h2>
								<div class="text-xs text-muted uppercase tracking-widest">{current.month}</div>
							</header>
							<div class="grid grid-cols-3 gap-2">
								<div class="flex flex-col gap-0.5">
									<div class="text-2xs uppercase tracking-widest text-muted">Income</div>
									<div class="text-sm tabular-nums text-success">{formatMoneyRounded(current.incomeMinor)}</div>
									{#if incomeChange !== null}
										<div class:text-success={incomeChange >= 0} class:text-error={incomeChange < 0} class="text-2xs tabular-nums">
											{incomeChange >= 0 ? "+" : ""}{incomeChange}%
										</div>
									{/if}
								</div>
								<div class="flex flex-col gap-0.5">
									<div class="text-2xs uppercase tracking-widest text-muted">Expenses</div>
									<div class="text-sm tabular-nums text-error">{formatMoneyRounded(current.expenseMinor)}</div>
									{#if expenseChange !== null}
										<div class:text-error={expenseChange > 0} class:text-success={expenseChange <= 0} class="text-2xs tabular-nums">
											{expenseChange >= 0 ? "+" : ""}{expenseChange}%
										</div>
									{/if}
								</div>
								<div class="flex flex-col gap-0.5">
									<div class="text-2xs uppercase tracking-widest text-muted">Net</div>
									<div class:text-success={current.netMinor >= 0} class:text-error={current.netMinor < 0} class="text-sm tabular-nums">
										{current.netMinor >= 0 ? "+" : ""}{formatMoneyRounded(current.netMinor)}
									</div>
									{#if netChange !== null}
										<div class:text-success={netChange >= 0} class:text-error={netChange < 0} class="text-2xs tabular-nums">
											{netChange >= 0 ? "+" : ""}{netChange}%
										</div>
									{:else if current.savingsRatePct !== null}
										<div class="text-2xs tabular-nums text-muted">{current.savingsRatePct >= 0 ? "+" : ""}{current.savingsRatePct.toFixed(0)}% rate</div>
									{/if}
								</div>
							</div>
						</article>
					{/if}

					<section class="border border-border bg-panel p-2.5 flex flex-col gap-2" style:min-height={isMobile ? "280px" : "385px"}>
						<header class="flex items-start justify-between gap-2.5">
							<div>
								<h2 class="font-normal text-sm uppercase tracking-widest">{GROUPS[groupId]?.label ?? groupId} cashflow</h2>
								<div class="text-sm mt-0.5 leading-snug uppercase tracking-wider text-muted">Income / expense / net (monthly)</div>
							</div>
							{#if getAnomalySummary(groupId).count > 0}
								{@const anomalies = getAnomalySummary(groupId)}
								<div class="flex items-center gap-1.5 text-xs" title={`Recent anomalies: ${anomalies.recent.join(", ") || "none"}`}>
									<span class="size-2 bg-pending rounded-full animate-pulse"></span>
									<span class="text-muted">{anomalies.count} anomal{anomalies.count === 1 ? "y" : "ies"}</span>
								</div>
							{/if}
						</header>

						{#if (groupCashflowSeries[groupId]?.length ?? 0) > 0}
							<div class="overflow-hidden" style:height={isMobile ? "220px" : "325px"}>
								<SeriesChart
									data={groupCashflowSeries[groupId] ?? []}
									getDate={(point: CashflowPoint) => `${point.month}-01`}
									series={[
										{
											key: "income",
											color: SEMANTIC_COLORS[colorScheme].income,
											getValue: (point: CashflowPoint) => point.incomeMinor / 100,
										},
										{
											key: "expense",
											color: SEMANTIC_COLORS[colorScheme].expense,
											lineStyle: "dashed",
											getValue: (point: CashflowPoint) => point.expenseMinor / 100,
										},
										{
											key: "net",
											color: lineColors.primary,
											getValue: (point: CashflowPoint) => point.netMinor / 100,
										},
									]}
									formatHover={(point: CashflowPoint) => {
										const parts = [
											`In ${formatMoney(point.incomeMinor)}`,
											`Out ${formatMoney(point.expenseMinor)}`,
											`Net ${point.netMinor >= 0 ? "+" : ""}${formatMoney(point.netMinor)}`,
										];
										if (point.savingsRatePct !== null) {
											parts.push(`Rate ${point.savingsRatePct >= 0 ? "+" : ""}${point.savingsRatePct.toFixed(0)}%`);
										}
										if (point.expenseDeviationRatio !== null) {
											const deviation = Math.round((point.expenseDeviationRatio - 1) * 100);
											parts.push(`Spend ${deviation >= 0 ? "+" : ""}${deviation}%`);
										}
										return parts.join(" | ");
									}}
									timeUnit="month"
									height={isMobile ? 220 : 325}
									compact={isMobile}
								/>
							</div>
						{:else}
							<div class="border border-dashed border-border-subtle h-[325px] grid place-items-center text-sm text-muted select-none">
								No cashflow data yet.
							</div>
						{/if}
					</section>

                    <article class="border border-border bg-panel p-2.5 flex flex-col gap-2">
                        <h2 class="font-normal text-sm uppercase tracking-widest">
                            {GROUPS[groupId]?.label ?? groupId} Distribution <span class="text-muted">[6MO AVG]</span>
                        </h2>
                        {#if getSankeyNodes(groupId).length > 0 && getSankeyLinks(groupId).length > 0}
                            <div style:height={isMobile ? "280px" : "350px"}>
                                <Sankey nodes={getSankeyNodes(groupId)} links={getSankeyLinks(groupId)} {colorScheme} compact={isMobile} />
                            </div>
                        {:else}
                            <div class="border border-dashed border-border-subtle min-h-[280px] grid place-items-center text-sm text-muted">
                                No distribution flow to show for this window.
                            </div>
                        {/if}
                    </article>

                    <article class="border border-border bg-panel p-2.5 flex flex-col gap-2">
                        <h2 class="font-normal text-sm uppercase tracking-widest">
                            Expense Breakdown <span class="text-muted">[6MO AVG]</span>
                        </h2>
                        {#if getTreemapData(groupId).length > 0}
                            <div style:height={isMobile ? "280px" : "350px"}>
                                <Treemap data={getTreemapData(groupId)} {colorScheme} compact={isMobile} />
                            </div>
                        {:else}
                            <div class="border border-dashed border-border-subtle min-h-[280px] grid place-items-center text-sm text-muted">
                                No categorized spend in this window.
                            </div>
                        {/if}
                    </article>

					{#if (ACCOUNT_GROUP_CONFIG[groupId]?.accounts.length ?? 0) > 0}
						<AccountGroupChart
							label={ACCOUNT_GROUP_CONFIG[groupId].label}
							accounts={ACCOUNT_GROUP_CONFIG[groupId].accounts}
							balanceSeries={accountBalanceSeries}
							contributionSeries={accountContributionSeries}
							investmentReturns={financeConfig.investmentProjectionAnnualReturns ?? undefined}
							{latestBalances}
						/>
					{/if}
				</section>
			{/if}
		{/each}
	</section>
</main>
