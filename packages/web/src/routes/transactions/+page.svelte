<script lang="ts">
	import { onDestroy } from "svelte";

	import Header from "$lib/Header.svelte";
	import {
		type TransactionDetail,
		type TransactionListItem,
		type TransactionListState,
		type TransactionsSortColumn,
		type TransactionsSortDirection,
	} from "$lib/transactions";
	import type { TransactionsPageData } from "$lib/server/transactions";

	type GroupId = string;

	type TransactionsApiListResponse = {
		list?: TransactionListState;
		error?: string;
	};

	type TransactionsApiDetailResponse = {
		detail?: TransactionDetail;
		error?: string;
	};

	const SORT_COLUMNS: Array<{ id: TransactionsSortColumn; label: string }> = [
		{ id: "postedAt", label: "Date" },
		{ id: "cleanDescription", label: "Title" },
		{ id: "pairAccountId", label: "Pair account" },
		{ id: "amountMinor", label: "Amount" },
	];

	const EMPTY_LIST: TransactionListState = {
		items: [],
		loadedCount: 0,
		totalCount: 0,
		limit: 10_000,
		truncated: false,
	};

	const listCache = new Map<string, TransactionListState>();
	const detailCache = new Map<string, TransactionDetail>();

	let { data: pageData }: { data: TransactionsPageData } = $props();

	const availableGroups = $derived(pageData.availableGroups as GroupId[]);
	const groupMetadata = $derived(pageData.groupMetadata);
	const connection = $derived(pageData.connection);

	let currentGroup = $state<GroupId>("personal");
	let currentSort = $state<TransactionsSortColumn>("postedAt");
	let currentDir = $state<TransactionsSortDirection>("desc");
	let selectedPostingId = $state<string | null>(null);
	let didInitializeState = $state(false);

	let listState = $state<TransactionListState>(EMPTY_LIST);
	let listLoading = $state(true);
	let listRefreshing = $state(false);
	let listError = $state<string | null>(null);

	let selectedTransaction = $state<TransactionDetail | null>(null);
	let detailLoading = $state(false);
	let detailError = $state<string | null>(null);

	let listAbortController: AbortController | null = null;
	let detailAbortController: AbortController | null = null;

	const effectiveGroup = $derived(didInitializeState ? currentGroup : pageData.initialGroup);
	const filteredItems = $derived(listState.items);
	const activeGroupLabel = $derived(groupMetadata[effectiveGroup]?.label ?? effectiveGroup);
	const listSummary = $derived.by(() => {
		if (listLoading && listState.loadedCount === 0) {
			return `Loading transactions for ${activeGroupLabel}`;
		}
		if (listState.truncated) {
			return `Showing ${filteredItems.length.toLocaleString("en-GB")} loaded of ${listState.totalCount.toLocaleString("en-GB")} for ${activeGroupLabel}`;
		}
		return `Showing ${filteredItems.length.toLocaleString("en-GB")} of ${listState.totalCount.toLocaleString("en-GB")} for ${activeGroupLabel}`;
	});
	const fetchStatusLabel = $derived.by(() => {
		if (listLoading && listState.loadedCount === 0) {
			return "Loading";
		}
		if (listRefreshing) {
			return "Refreshing";
		}
		if (listState.truncated) {
			return `Loaded first ${listState.loadedCount.toLocaleString("en-GB")}`;
		}
		return `${listState.loadedCount.toLocaleString("en-GB")} loaded`;
	});

	$effect(() => {
		if (didInitializeState) {
			return;
		}
		currentGroup = pageData.initialGroup;
		currentSort = pageData.initialSort;
		currentDir = pageData.initialDir;
		selectedPostingId = pageData.selectedPostingId;
		didInitializeState = true;
	});

	$effect(() => {
		if (typeof window === "undefined") {
			return;
		}
		if (!didInitializeState) {
			return;
		}
		void loadList(currentGroup, currentSort, currentDir);
	});

	$effect(() => {
		if (!didInitializeState) {
			return;
		}
		const visibleIds = new Set(filteredItems.map((item) => item.postingId));
		if (selectedPostingId && visibleIds.has(selectedPostingId)) {
			return;
		}
		selectedPostingId = filteredItems[0]?.postingId ?? null;
	});

	$effect(() => {
		if (typeof window === "undefined") {
			return;
		}
		if (!didInitializeState) {
			return;
		}
		const postingId = selectedPostingId;
		if (!postingId) {
			selectedTransaction = null;
			detailError = null;
			detailLoading = false;
			syncUrl();
			return;
		}
		void loadDetail(postingId);
	});

	$effect(() => {
		if (typeof window === "undefined") {
			return;
		}
		if (!didInitializeState) {
			return;
		}
		syncUrl();
	});

	onDestroy(() => {
		listAbortController?.abort();
		detailAbortController?.abort();
	});

	function handleGroupChange(nextGroup: GroupId) {
		if (nextGroup === currentGroup) {
			return;
		}
		currentGroup = nextGroup;
		selectedPostingId = null;
		selectedTransaction = null;
	}

	function handleSort(nextSort: TransactionsSortColumn) {
		const nextDirection: TransactionsSortDirection =
			currentSort === nextSort
				? currentDir === "asc"
					? "desc"
					: "asc"
				: nextSort === "cleanDescription" || nextSort === "pairAccountId"
					? "asc"
					: "desc";

		currentSort = nextSort;
		currentDir = nextDirection;
		selectedPostingId = null;
		selectedTransaction = null;
	}

	function handleSelect(postingId: string) {
		selectedPostingId = postingId;
	}

	function syncUrl() {
		const url = new URL(window.location.href);
		url.searchParams.set("group", currentGroup);
		url.searchParams.set("sort", currentSort);
		url.searchParams.set("dir", currentDir);
		url.searchParams.delete("search");
		if (selectedPostingId) {
			url.searchParams.set("selected", selectedPostingId);
		} else {
			url.searchParams.delete("selected");
		}
		window.history.replaceState(window.history.state, "", url);
	}

	async function loadList(group: GroupId, sort: TransactionsSortColumn, dir: TransactionsSortDirection) {
		const cacheKey = buildListCacheKey(group, sort, dir);
		const cached = listCache.get(cacheKey);
		if (cached) {
			listState = cached;
			listLoading = false;
			listRefreshing = false;
			listError = null;
		}

		listAbortController?.abort();
		const controller = new AbortController();
		listAbortController = controller;

		if (!cached) {
			listLoading = true;
		} else {
			listRefreshing = true;
		}

		try {
			const url = new URL("/api/transactions", window.location.origin);
			url.searchParams.set("group", group);
			url.searchParams.set("sort", sort);
			url.searchParams.set("dir", dir);

			const response = await fetch(url, {
				headers: { accept: "application/json" },
				cache: "no-store",
				signal: controller.signal,
			});
			const payload = (await response.json()) as TransactionsApiListResponse;
			if (!response.ok || !payload.list) {
				throw new Error(payload.error ?? "Failed to load transactions");
			}

			listState = payload.list;
			listCache.set(cacheKey, payload.list);
			listError = null;
		} catch (error) {
			if (error instanceof DOMException && error.name === "AbortError") {
				return;
			}
			listError = error instanceof Error ? error.message : "Failed to load transactions";
			if (!cached) {
				listState = EMPTY_LIST;
			}
		} finally {
			if (listAbortController === controller && !controller.signal.aborted) {
				listLoading = false;
				listRefreshing = false;
			}
		}
	}

	async function loadDetail(postingId: string) {
		const cached = detailCache.get(postingId);
		if (cached) {
			selectedTransaction = cached;
			detailError = null;
			detailLoading = false;
			return;
		}

		detailAbortController?.abort();
		const controller = new AbortController();
		detailAbortController = controller;
		detailLoading = true;
		detailError = null;

		try {
			const response = await fetch(`/api/transactions/${encodeURIComponent(postingId)}`, {
				headers: { accept: "application/json" },
				cache: "no-store",
				signal: controller.signal,
			});
			const payload = (await response.json()) as TransactionsApiDetailResponse;
			if (!response.ok || !payload.detail) {
				throw new Error(payload.error ?? "Failed to load transaction detail");
			}
			detailCache.set(postingId, payload.detail);
			if (selectedPostingId === postingId) {
				selectedTransaction = payload.detail;
			}
		} catch (error) {
			if (error instanceof DOMException && error.name === "AbortError") {
				return;
			}
			detailError = error instanceof Error ? error.message : "Failed to load transaction detail";
			if (selectedPostingId === postingId) {
				selectedTransaction = null;
			}
		} finally {
			if (detailAbortController === controller && !controller.signal.aborted) {
				detailLoading = false;
			}
		}
	}

	function buildListCacheKey(group: GroupId, sort: TransactionsSortColumn, dir: TransactionsSortDirection): string {
		return `${group}:${sort}:${dir}`;
	}

	function formatMoney(minor: number, currency = "GBP"): string {
		return (minor / 100).toLocaleString("en-GB", {
			style: "currency",
			currency,
			maximumFractionDigits: 2,
		});
	}

	function formatDate(date: string): string {
		return new Date(`${date}T00:00:00Z`).toLocaleDateString("en-GB", {
			day: "numeric",
			month: "short",
			year: "numeric",
		});
	}

	function formatDateTime(isoDateTime: string): string {
		return new Date(isoDateTime).toLocaleString("en-GB", {
			day: "numeric",
			month: "short",
			year: "numeric",
			hour: "2-digit",
			minute: "2-digit",
		});
	}

	function getSortArrow(column: TransactionsSortColumn): string {
		if (currentSort !== column) {
			return "";
		}
		return currentDir === "asc" ? "↑" : "↓";
	}

	function getPairLabel(item: TransactionListItem): string {
		if (item.pairAccountIds.length > 0) {
			return item.pairAccountIds.join(" · ");
		}
		if (item.counterparty) {
			return item.counterparty;
		}
		return "—";
	}

	function getDetailRows(detail: TransactionDetail | null): Array<{ label: string; value: string }> {
		if (!detail) {
			return [];
		}
		return [
			{ label: "Posted", value: formatDateTime(detail.postedAt) },
			{ label: "Account", value: detail.chartAccountId },
			{ label: "Counterparty", value: detail.counterparty ?? "—" },
			{ label: "Source file", value: detail.sourceFile ?? "—" },
			{ label: "Entry", value: detail.journalEntryId },
			{ label: "Posting", value: detail.postingId },
		];
	}
</script>

<svelte:head>
	<title>Finances | Transactions</title>
</svelte:head>

<main class="h-svh overflow-hidden box-border px-2.5 pb-2.5 flex flex-col gap-2">
	<h1 class="sr-only">Transactions</h1>
	<Header
		activePage="transactions"
		activeGroup={effectiveGroup}
		onGroupChange={handleGroupChange}
		{availableGroups}
		{groupMetadata}
		loading={connection.loading}
		error={connection.error}
		detail={connection.detail}
	/>

	<section class="border border-border bg-panel flex-1 min-h-0 grid xl:grid-cols-[minmax(0,1.9fr)_minmax(320px,1fr)] fade-in overflow-hidden">
			<div class="min-h-0 flex flex-col">
				<header class="border-b border-border p-2.5 flex flex-col gap-2">
					<div class="flex flex-col gap-1 lg:flex-row lg:items-end lg:justify-between">
						<div>
							<h2 class="font-normal text-2xs uppercase tracking-widest">Transactions</h2>
							<div class="text-2xs mt-0.5 leading-snug uppercase tracking-wider text-muted">
								{listSummary}
							</div>
						</div>
						<div class="text-2xs uppercase tracking-widest text-muted">{fetchStatusLabel}</div>
					</div>

					<div class="flex items-center gap-2 text-2xs uppercase tracking-widest text-muted">
						{#if listRefreshing}
							<span class="text-pending">Updating</span>
						{/if}
						{#if listState.truncated}
							<span>Limit {listState.limit.toLocaleString("en-GB")}</span>
						{/if}
					</div>
				</header>

			<div class="border-b border-border bg-panel">
				<div class="hidden md:grid md:grid-cols-[118px_minmax(0,1.75fr)_minmax(0,1.15fr)_132px] md:gap-3 md:px-2.5">
					{#each SORT_COLUMNS as column}
						<button
							type="button"
							class={`min-h-[38px] min-w-0 text-2xs uppercase tracking-widest text-muted font-normal cursor-pointer hover:text-text transition-colors bg-transparent border-0 appearance-none ${
								column.id === "amountMinor" ? "text-right" : "text-left"
							}`}
							onclick={() => handleSort(column.id)}
						>
							{column.label}
							{#if getSortArrow(column.id)}
								<span class="ml-1">{getSortArrow(column.id)}</span>
							{/if}
						</button>
					{/each}
				</div>
			</div>

			<div class="flex-1 overflow-auto min-h-0" aria-busy={listLoading || listRefreshing}>
				{#if listLoading && listState.loadedCount === 0}
					<div class="divide-y divide-border-subtle">
						{#each Array.from({ length: 10 }) as _, index (`skeleton-${index}`)}
							<div class="px-2.5 py-2.5 grid md:grid-cols-[118px_minmax(0,1.75fr)_minmax(0,1.15fr)_132px] gap-3 animate-pulse">
								<div class="h-3 bg-border-subtle/70"></div>
								<div class="h-3 bg-border-subtle/70"></div>
								<div class="h-3 bg-border-subtle/50"></div>
								<div class="h-3 bg-border-subtle/50"></div>
							</div>
						{/each}
					</div>
				{:else if listError}
					<div class="h-full flex flex-col items-center justify-center gap-2 p-6 text-center">
						<div class="text-sm uppercase tracking-widest text-error">Transactions unavailable</div>
						<div class="text-sm text-muted leading-relaxed max-w-xl">{listError}</div>
					</div>
				{:else if filteredItems.length === 0}
					<div class="h-full flex flex-col items-center justify-center gap-2 p-6 text-center">
						<div class="text-sm uppercase tracking-widest">No transactions found</div>
						<div class="text-sm text-muted leading-relaxed max-w-xl">
							Adjust the selected group to load a different transaction stream.
						</div>
					</div>
				{:else}
					<div class="divide-y divide-border-subtle">
						{#each filteredItems as item (item.postingId)}
							<button
								type="button"
								class="w-full text-left px-2.5 py-2 transition-colors hover:bg-panel-subtle"
								class:bg-panel-subtle={selectedPostingId === item.postingId}
								aria-pressed={selectedPostingId === item.postingId}
								onclick={() => handleSelect(item.postingId)}
							>
								<div class="flex items-center justify-between gap-3 md:hidden">
									<div class="min-w-0 flex-1">
										<div class="text-sm leading-snug truncate">{item.cleanDescription}</div>
										<div class="text-2xs uppercase tracking-widest text-muted truncate mt-1">
											{formatDate(item.postedDate)} · {getPairLabel(item)}
										</div>
									</div>
									<div
										class="text-sm tabular-nums whitespace-nowrap"
										class:text-success={item.amountMinor > 0}
										class:text-error={item.amountMinor < 0}
									>
										{formatMoney(item.amountMinor, item.currency)}
									</div>
								</div>

								<div class="hidden md:grid md:grid-cols-[118px_minmax(0,1.75fr)_minmax(0,1.15fr)_132px] md:items-center md:gap-3">
									<div class="text-2xs text-muted tabular-nums whitespace-nowrap">{formatDate(item.postedDate)}</div>
									<div class="min-w-0 text-sm truncate">{item.cleanDescription}</div>
									<div class="min-w-0 text-sm text-muted truncate">{getPairLabel(item)}</div>
									<div
										class="text-sm text-right tabular-nums whitespace-nowrap"
										class:text-success={item.amountMinor > 0}
										class:text-error={item.amountMinor < 0}
									>
										{formatMoney(item.amountMinor, item.currency)}
									</div>
								</div>
							</button>
						{/each}
					</div>
				{/if}
			</div>
		</div>

		<aside class="border-t xl:border-t-0 xl:border-l border-border min-h-0 flex flex-col overflow-hidden bg-panel/40">
			<header class="border-b border-border p-2.5 flex flex-col gap-1.5">
				<h2 class="font-normal text-sm uppercase tracking-widest">Transaction detail</h2>
				<div class="text-sm leading-snug uppercase tracking-wider text-muted">
					Server-backed ledger detail for the selected posting
				</div>
			</header>

			<div class="flex-1 overflow-auto min-h-0 p-2.5">
				{#if detailLoading && !selectedTransaction}
					<div class="h-full flex items-center justify-center text-sm text-muted leading-relaxed">
						Loading transaction detail…
					</div>
				{:else if detailError}
					<div class="h-full flex items-center justify-center text-center text-sm text-muted leading-relaxed">
						{detailError}
					</div>
				{:else if selectedTransaction}
					<div class="flex flex-col gap-3">
						<div class="border border-border-subtle p-2.5 flex flex-col gap-1.5">
							<div class="flex items-start justify-between gap-3">
								<div class="min-w-0">
									<div class="text-lg leading-tight truncate">
										{selectedTransaction.cleanDescription ?? selectedTransaction.description}
									</div>
									<div class="text-2xs uppercase tracking-widest text-muted mt-1 truncate">
										{selectedTransaction.counterparty ?? selectedTransaction.description}
									</div>
								</div>
								<div
									class="text-lg tabular-nums whitespace-nowrap"
									class:text-success={selectedTransaction.amountMinor > 0}
									class:text-error={selectedTransaction.amountMinor < 0}
								>
									{formatMoney(selectedTransaction.amountMinor, selectedTransaction.currency)}
								</div>
							</div>
							<div class="text-2xs uppercase tracking-widest text-muted">
								{selectedTransaction.isTransfer ? "Transfer" : "Ledger transaction"}
							</div>
						</div>

						<div class="border border-border-subtle divide-y divide-border-subtle">
							{#each getDetailRows(selectedTransaction) as row}
								<div class="px-2.5 py-2 flex items-start justify-between gap-3 text-sm">
									<div class="text-2xs uppercase tracking-widest text-muted">{row.label}</div>
									<div class="text-right break-all">{row.value}</div>
								</div>
							{/each}
						</div>

						<div class="border border-border-subtle p-2.5 flex flex-col gap-2">
							<div class="text-2xs uppercase tracking-widest text-muted">Descriptions</div>
							<div class="grid gap-2 text-sm">
								<div>
									<div class="text-2xs uppercase tracking-widest text-muted">Display</div>
									<div class="mt-1 break-words">{selectedTransaction.description}</div>
								</div>
								{#if selectedTransaction.cleanDescription && selectedTransaction.cleanDescription !== selectedTransaction.description}
									<div>
										<div class="text-2xs uppercase tracking-widest text-muted">Clean</div>
										<div class="mt-1 break-words">{selectedTransaction.cleanDescription}</div>
									</div>
								{/if}
								{#if selectedTransaction.rawDescription && selectedTransaction.rawDescription !== selectedTransaction.description}
									<div>
										<div class="text-2xs uppercase tracking-widest text-muted">Raw</div>
										<div class="mt-1 break-words">{selectedTransaction.rawDescription}</div>
									</div>
								{/if}
							</div>
						</div>

						<div class="border border-border-subtle overflow-hidden">
							<div class="px-2.5 py-2 text-2xs uppercase tracking-widest text-muted border-b border-border-subtle">
								Pair postings
							</div>
							{#if selectedTransaction.pairPostings.length > 0}
								<div class="divide-y divide-border-subtle">
									{#each selectedTransaction.pairPostings as posting}
										<div class="px-2.5 py-2 flex flex-col gap-1.5 text-sm">
											<div class="flex items-start justify-between gap-3">
												<div class="break-all">{posting.accountId}</div>
												<div class="tabular-nums whitespace-nowrap">{formatMoney(posting.amountMinor, posting.currency)}</div>
											</div>
											<div class="text-2xs uppercase tracking-widest text-muted">
												{posting.memo ?? "No memo"}
											</div>
										</div>
									{/each}
								</div>
							{:else}
								<div class="px-2.5 py-3 text-sm text-muted">No counterparty postings were returned for this transaction.</div>
							{/if}
						</div>
					</div>
				{:else}
					<div class="h-full flex items-center justify-center text-center text-sm text-muted leading-relaxed">
						Select a transaction row to inspect its full ledger detail.
					</div>
				{/if}
			</div>
		</aside>
	</section>
</main>
