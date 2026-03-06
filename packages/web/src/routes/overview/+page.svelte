<script lang="ts">
	import { onMount } from "svelte";

	import ProjectionChart from "$lib/charts/ProjectionChart.svelte";
	import SeriesChart from "$lib/charts/SeriesChart.svelte";
	import { mergeBalanceSeries, type CashAssetsPoint, type ProjectionPoint } from "$lib/charts/utils";
	import Header from "$lib/Header.svelte";
	import { theme } from "$lib/theme.svelte";
	import type { OverviewChartAccount, OverviewPageData, OverviewProjectionScenario } from "$lib/server/overview";

	type GroupId = string;
	type ProjectionStatus = {
		label: string;
		detail: string;
		toneClass: string;
	};

	let { data }: { data: OverviewPageData } = $props();

	const availableGroups = $derived(data.availableGroups as GroupId[]);
	const groupMetadata = $derived(data.groupMetadata);
	const connection = $derived(data.connection);
	const chartAccounts = $derived(data.chartAccounts as OverviewChartAccount[]);
	const totalBalanceSeries = $derived(data.totalBalanceSeries);
	const accountBalanceSeries = $derived(data.accountBalanceSeries);
	const projection = $derived(data.projection);
	const totalSeriesId = $derived(data.totalSeriesId);
	const colorScheme = $derived(theme.resolved);

	let isMobile = $state(false);

	const moneyFormatter = new Intl.NumberFormat("en-GB", {
		style: "currency",
		currency: "GBP",
		maximumFractionDigits: 0,
	});

	const linePalette = $derived.by(() => {
		const isDark = colorScheme === "dark";
		return {
			total: isDark ? "#f8fafc" : "#111827",
			personal: isDark ? ["#93c5fd", "#60a5fa", "#3b82f6"] : ["#1d4ed8", "#2563eb", "#60a5fa"],
			joint: isDark ? ["#5eead4", "#2dd4bf", "#14b8a6"] : ["#0f766e", "#0d9488", "#2dd4bf"],
			business: isDark ? ["#fcd34d", "#f59e0b", "#f97316"] : ["#b45309", "#d97706", "#f59e0b"],
			other: isDark ? ["#cbd5e1", "#94a3b8", "#64748b"] : ["#334155", "#475569", "#94a3b8"],
		};
	});

	const hasOverviewData = $derived.by(() => {
		return totalBalanceSeries.length > 0 || chartAccounts.length > 0 || projection !== null;
	});

	const latestTotalBalanceMinor = $derived(totalBalanceSeries.at(-1)?.balanceMinor ?? null);
	const historyCoverage = $derived.by(() => {
		const first = totalBalanceSeries[0]?.date;
		const last = totalBalanceSeries.at(-1)?.date;
		if (!first || !last) {
			return "No balance history yet";
		}
		return `${formatMonthYear(first)} to ${formatMonthYear(last)}`;
	});

	const balanceChartData = $derived.by(() => {
		const seriesMap: Record<string, { date: string; balanceMinor: number }[]> = {};
		if (totalBalanceSeries.length > 0) {
			seriesMap[totalSeriesId] = totalBalanceSeries;
		}
		for (const account of chartAccounts) {
			const series = accountBalanceSeries[account.id] ?? [];
			if (series.length > 0) {
				seriesMap[account.id] = series;
			}
		}
		return mergeBalanceSeries(seriesMap);
	});

	const balanceSeriesDefinitions = $derived.by(() => {
		const groupIndices = new Map<string, number>();
		const definitions = [] as Array<{
			key: string;
			color: string;
			lineWidth?: number;
			getValue: (point: CashAssetsPoint) => number;
		}>;

		if (totalBalanceSeries.length > 0) {
			definitions.push({
				key: totalSeriesId,
				color: linePalette.total,
				lineWidth: 3,
				getValue: (point: CashAssetsPoint) => (point.accounts[totalSeriesId] ?? 0) / 100,
			});
		}

		for (const account of chartAccounts) {
			if ((accountBalanceSeries[account.id]?.length ?? 0) === 0) {
				continue;
			}
			const nextIndex = groupIndices.get(account.groupId) ?? 0;
			groupIndices.set(account.groupId, nextIndex + 1);
			definitions.push({
				key: account.id,
				color: getAccountColor(account.groupId, nextIndex),
				lineWidth: 2,
				getValue: (point: CashAssetsPoint) => (point.accounts[account.id] ?? 0) / 100,
			});
		}

		return definitions;
	});

	const currentScenario = $derived(projection?.currentBurn ?? null);
	const minimumScenario = $derived(projection?.minimumBurn ?? null);
	const projectionStatus = $derived(getProjectionStatus(currentScenario));
	const includedGroupsLabel = $derived.by(() => {
		if (!projection || projection.groups.length === 0) {
			return "No groups selected";
		}
		return projection.groups.map((groupId) => groupMetadata[groupId]?.label ?? groupId).join(" + ");
	});
	const accountList = $derived.by(() => {
		return chartAccounts.map((account) => ({
			...account,
			pointCount: accountBalanceSeries[account.id]?.length ?? 0,
			groupLabel: groupMetadata[account.groupId]?.label ?? account.groupId,
		}));
	});

	onMount(() => {
		const mobileQuery = window.matchMedia("(max-width: 640px)");
		const updateMobile = (event: MediaQueryList | MediaQueryListEvent) => {
			isMobile = event.matches;
		};
		updateMobile(mobileQuery);
		mobileQuery.addEventListener("change", updateMobile);

		return () => {
			mobileQuery.removeEventListener("change", updateMobile);
		};
	});

	function getAccountColor(groupId: string, groupIndex: number): string {
		const palette = linePalette[groupId as keyof typeof linePalette] ?? linePalette.other;
		return palette[Math.min(groupIndex, palette.length - 1)] ?? linePalette.other[0];
	}

	function formatMoney(minor: number | null | undefined): string {
		if (minor === null || minor === undefined) {
			return "—";
		}
		return moneyFormatter.format(minor / 100);
	}

	function formatMonthYear(date: string): string {
		return new Date(`${date}T00:00:00Z`).toLocaleDateString("en-GB", {
			month: "short",
			year: "numeric",
		});
	}

	function formatLongDate(date: string | null | undefined): string {
		if (!date) {
			return "—";
		}
		return new Date(`${date}T00:00:00Z`).toLocaleDateString("en-GB", {
			day: "numeric",
			month: "short",
			year: "numeric",
		});
	}

	function formatMonths(months: number | null | undefined): string {
		if (months === null || months === undefined) {
			return "—";
		}
		return `${months}m`;
	}

	function formatPercent(value: number | null | undefined): string {
		if (value === null || value === undefined) {
			return "—";
		}
		return `${Math.round(value * 100)}%`;
	}

	function formatBalanceHover(point: CashAssetsPoint): string {
		const lines = [formatLongDate(point.date)];
		const total = point.accounts[totalSeriesId];
		if (total !== null && total !== undefined) {
			lines.push(`All assets: ${formatMoney(total)}`);
		}
		for (const account of chartAccounts) {
			const value = point.accounts[account.id];
			if (value === null || value === undefined) {
				continue;
			}
			const label = groupMetadata[account.groupId]?.label ?? account.groupId;
			lines.push(`${account.label} (${label}): ${formatMoney(value)}`);
		}
		return lines.join("\n");
	}

	function formatProjectionHover(current: ProjectionPoint, minimum: ProjectionPoint): string {
		const lines = [formatLongDate(current.date)];
		lines.push(`Current burn: ${formatMoney(current.balanceMinor)}`);
		lines.push(`Minimum burn: ${formatMoney(minimum.balanceMinor)}`);
		return lines.join("\n");
	}

	function getProjectionStatus(scenario: OverviewProjectionScenario | null): ProjectionStatus {
		if (!scenario) {
			return {
				label: "Projection unavailable",
				detail: "Run the daemon against a valid ledger to restore runway history.",
				toneClass: "text-muted",
			};
		}
		if (scenario.isNetPositive) {
			return {
				label: "Net positive",
				detail: "Current burn stays above zero across the full projection horizon.",
				toneClass: "text-success",
			};
		}
		if (scenario.zeroBalanceCrossing) {
			const monthIndex = scenario.zeroBalanceCrossing.monthIndex;
			return {
				label: formatMonthYear(scenario.zeroBalanceCrossing.date),
				detail: `Zero balance in ${formatMonths(monthIndex)} under the current burn assumption.`,
				toneClass: monthIndex <= 6 ? "text-error" : monthIndex <= 12 ? "text-pending" : "text-text",
			};
		}
		const lastPoint = scenario.points.at(-1);
		return {
			label: lastPoint ? formatMonthYear(lastPoint.date) : "Projected",
			detail: "No zero-balance crossing appears within the projection window.",
			toneClass: "text-success",
		};
	}
</script>

<svelte:head>
	<title>Finances | Overview</title>
</svelte:head>

<main class="h-svh overflow-auto box-border px-2.5 pb-2.5 flex flex-col gap-2">
	<h1 class="sr-only">Financial Overview</h1>
	<Header
		activePage="overview"
		activeGroup={availableGroups[0] ?? "personal"}
		onGroupChange={() => {}}
		{availableGroups}
		{groupMetadata}
		allGroupsActive
		loading={connection.loading}
		error={connection.error}
	/>

	{#if hasOverviewData}
		<section class="grid gap-2 sm:grid-cols-2 xl:grid-cols-4 fade-in">
			<article class="border border-border bg-panel p-2.5 flex flex-col gap-1.5">
				<div class="text-2xs uppercase tracking-widest text-muted">All assets</div>
				<div class="text-2xl font-normal tabular-nums">{formatMoney(latestTotalBalanceMinor)}</div>
				<div class="text-2xs uppercase tracking-widest text-muted">{historyCoverage}</div>
			</article>
			<article class="border border-border bg-panel p-2.5 flex flex-col gap-1.5">
				<div class="text-2xs uppercase tracking-widest text-muted">Liquid balance</div>
				<div class="text-2xl font-normal tabular-nums">{formatMoney(projection?.liquidBalanceMinor)}</div>
				<div class="text-2xs uppercase tracking-widest text-muted">{includedGroupsLabel}</div>
			</article>
			<article class="border border-border bg-panel p-2.5 flex flex-col gap-1.5">
				<div class="text-2xs uppercase tracking-widest text-muted">Current burn</div>
				<div class="text-2xl font-normal tabular-nums">{formatMoney(projection?.currentBurnMinor)}</div>
				<div class="text-2xs uppercase tracking-widest text-muted">
					Minimum burn {formatMoney(projection?.minimumBurnMinor)}
				</div>
			</article>
			<article class="border border-border bg-panel p-2.5 flex flex-col gap-1.5">
				<div class="text-2xs uppercase tracking-widest text-muted">Runway status</div>
				<div class={`text-2xl font-normal tabular-nums ${projectionStatus.toneClass}`}>{projectionStatus.label}</div>
				<div class="text-2xs uppercase tracking-widest text-muted">{projectionStatus.detail}</div>
			</article>
		</section>

		<article class="border border-border bg-panel p-2.5 flex flex-col gap-3 fade-in">
			<header class="flex flex-col gap-1.5 xl:flex-row xl:items-end xl:justify-between">
				<div>
					<h2 class="font-normal text-sm uppercase tracking-widest">Account balance history</h2>
					<div class="text-sm mt-0.5 leading-snug uppercase tracking-wider text-muted">
						Merged daily history across all configured asset accounts
					</div>
				</div>
				<div class="text-sm uppercase tracking-widest text-muted">{chartAccounts.length} chart accounts</div>
			</header>

			<div class="grid gap-3 xl:grid-cols-[minmax(0,1.8fr)_minmax(300px,1fr)] xl:items-start">
				<div class="border border-border-subtle bg-panel/40 p-2 min-h-[280px]">
					{#if balanceChartData.length > 0 && balanceSeriesDefinitions.length > 0}
						<SeriesChart
							data={balanceChartData}
							getDate={(point: CashAssetsPoint) => point.date}
							series={balanceSeriesDefinitions}
							formatHover={formatBalanceHover}
							height={isMobile ? 280 : 420}
							compact={isMobile}
							curve={true}
							showRawOverlay={false}
						/>
					{:else}
						<div class="h-full min-h-[280px] flex items-center justify-center text-sm text-muted">
							No balance history available yet.
						</div>
					{/if}
				</div>

				<div class="border border-border-subtle bg-panel/40 divide-y divide-border-subtle overflow-hidden">
					<div class="p-2 flex items-center justify-between gap-2 text-2xs uppercase tracking-widest text-muted">
						<span>Account</span>
						<span>Latest</span>
					</div>
					{#if accountList.length > 0}
						<div class="max-h-[420px] overflow-auto">
							{#each accountList as account (account.id)}
								<div class="p-2 flex flex-col gap-1.5 border-t border-border-subtle first:border-t-0">
									<div class="flex items-start justify-between gap-2">
										<div>
											<div class="text-sm leading-snug">{account.label}</div>
											<div class="text-2xs uppercase tracking-widest text-muted">
												{account.groupLabel} · {account.provider}
											</div>
										</div>
										<div class="text-sm tabular-nums">{formatMoney(account.latestBalanceMinor)}</div>
									</div>
									<div class="text-2xs uppercase tracking-widest text-muted">
										{account.pointCount} points · updated {formatLongDate(account.updatedAt)}
									</div>
								</div>
							{/each}
						</div>
					{:else}
						<div class="p-3 text-sm text-muted">No asset accounts were available from fin-api.</div>
					{/if}
				</div>
			</div>
		</article>

		<article class="border border-border bg-panel p-2.5 flex flex-col gap-3 fade-in">
			<header class="flex flex-col gap-1.5 xl:flex-row xl:items-end xl:justify-between">
				<div>
					<h2 class="font-normal text-sm uppercase tracking-widest">Runway projection</h2>
					<div class="text-sm mt-0.5 leading-snug uppercase tracking-wider text-muted">
						Server-defined current-burn and minimum-burn scenarios
					</div>
				</div>
				<div class="text-sm uppercase tracking-widest text-muted">
					As of {formatLongDate(projection?.assumptions.asOfDate)}
				</div>
			</header>

			<div class="grid gap-3 xl:grid-cols-[minmax(0,1.8fr)_minmax(300px,1fr)] xl:items-start">
				<div class="border border-border-subtle bg-panel/40 p-2 min-h-[280px]">
					{#if currentScenario && minimumScenario}
						<ProjectionChart
							currentBurn={currentScenario.points}
							minimumBurn={minimumScenario.points}
							threshold={projection?.thresholds.thresholdMinor !== null && projection?.thresholds.thresholdMinor !== undefined
								? projection.thresholds.thresholdMinor / 100
								: null}
							warningLine={projection?.thresholds.warningMinor !== null && projection?.thresholds.warningMinor !== undefined
								? projection.thresholds.warningMinor / 100
								: null}
							formatHover={formatProjectionHover}
							height={isMobile ? 280 : 420}
							compact={isMobile}
						/>
					{:else}
						<div class="h-full min-h-[280px] flex items-center justify-center text-sm text-muted">
							No projection data available yet.
						</div>
					{/if}
				</div>

				<div class="border border-border-subtle bg-panel/40 divide-y divide-border-subtle overflow-hidden">
					<div class="p-2 flex flex-col gap-1">
						<div class="text-2xs uppercase tracking-widest text-muted">Projection assumptions</div>
						<div class="text-sm leading-relaxed text-muted">
							{projection?.assumptions.projectionMonths ?? 0}-month horizon · trailing window {projection?.assumptions.trailingOutflowWindowMonths ?? 0} months
						</div>
					</div>
					<div class="p-2 grid gap-2 text-sm">
						<div class="flex items-center justify-between gap-2">
							<span class="text-muted">Median monthly expense</span>
							<span class="tabular-nums">{formatMoney(projection?.medianMonthlyExpenseMinor)}</span>
						</div>
						<div class="flex items-center justify-between gap-2">
							<span class="text-muted">Warning line</span>
							<span class="tabular-nums">{formatMoney(projection?.thresholds.warningMinor)}</span>
						</div>
						<div class="flex items-center justify-between gap-2">
							<span class="text-muted">Threshold</span>
							<span class="tabular-nums">{formatMoney(projection?.thresholds.thresholdMinor)}</span>
						</div>
						<div class="flex items-center justify-between gap-2">
							<span class="text-muted">Minimum burn ratio</span>
							<span>{formatPercent(projection?.assumptions.minimumBurnRatio)}</span>
						</div>
					</div>
					<div class="p-2 grid gap-3 text-sm">
						<div>
							<div class="text-2xs uppercase tracking-widest text-muted">Current burn scenario</div>
							<div class="mt-1 text-sm leading-relaxed">{projectionStatus.detail}</div>
							<div class={`mt-1 text-2xs uppercase tracking-widest ${projectionStatus.toneClass}`}>
								{projectionStatus.label}
							</div>
						</div>
						<div>
							<div class="text-2xs uppercase tracking-widest text-muted">Minimum burn scenario</div>
							<div class="mt-1 text-sm leading-relaxed">
								{#if minimumScenario?.isNetPositive}
									Net positive across the full horizon.
								{:else if minimumScenario?.zeroBalanceCrossing}
									Zero balance at {formatMonthYear(minimumScenario.zeroBalanceCrossing.date)}.
								{:else}
									No zero-balance crossing appears within the projection window.
								{/if}
							</div>
							<div class="mt-1 text-2xs uppercase tracking-widest text-muted">
								Burn {formatMoney(minimumScenario?.burnRateMinor)} · updated {formatLongDate(projection?.assumptions.asOfDate)}
							</div>
						</div>
					</div>
				</div>
			</div>
		</article>
	{:else}
		<article class="border border-border bg-panel p-3 flex flex-col gap-3 fade-in">
			<div>
				<h2 class="font-normal text-sm uppercase tracking-widest">Overview awaiting ledger data</h2>
				<div class="text-sm mt-1 leading-relaxed text-muted">
					The overview surface is wired to `fin-api`. Once the daemon can load config and ledger data, this page will show consolidated balance history and runway projection.
				</div>
			</div>
			<div class="text-2xs uppercase tracking-widest text-muted">{connection.detail}</div>
		</article>
	{/if}

	<footer class="px-0.5 text-2xs uppercase tracking-widest text-muted">
		{connection.detail}
	</footer>
</main>
