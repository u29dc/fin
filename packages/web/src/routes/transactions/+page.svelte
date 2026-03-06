<script lang="ts">
	import { goto } from "$app/navigation";
	import { page } from "$app/state";

	import Header from "$lib/Header.svelte";
	import type {
		TransactionDetail,
		TransactionListItem,
		TransactionsPageData,
		TransactionsSortColumn,
		TransactionsSortDirection,
	} from "$lib/server/transactions";

	type GroupId = string;

	const SORT_COLUMNS: Array<{ id: TransactionsSortColumn; label: string }> = [
		{ id: "postedAt", label: "Date" },
		{ id: "cleanDescription", label: "Title" },
		{ id: "pairAccountId", label: "Counterparty" },
		{ id: "amountMinor", label: "Amount" },
	];

	let { data }: { data: TransactionsPageData } = $props();

	const availableGroups = $derived(data.availableGroups as GroupId[]);
	const groupMetadata = $derived(data.groupMetadata);
	const connection = $derived(data.connection);
	const group = $derived(data.initialGroup);
	const sortColumn = $derived(data.initialSort);
	const sortDirection = $derived(data.initialDir);
	const searchQuery = $derived(data.searchQuery);
	const list = $derived(data.list);
	const selectedPostingId = $derived(data.selectedPostingId);
	const selectedTransaction = $derived(data.selectedTransaction as TransactionDetail | null);

	let searchInput = $state("");

	$effect(() => {
		searchInput = searchQuery;
	});

	$effect(() => {
		const currentTrail = page.url.searchParams.getAll("cursor");
		const currentSelected = page.url.searchParams.get("selected");
		const shouldNormalizeTrail = !cursorTrailMatches(currentTrail, list.cursorTrail);
		const shouldNormalizeSelection = currentSelected !== selectedPostingId;
		if (shouldNormalizeTrail || shouldNormalizeSelection) {
			navigate((url) => {
				if (shouldNormalizeTrail) {
					setCursorTrail(url, list.cursorTrail);
				}
				if (selectedPostingId) {
					url.searchParams.set("selected", selectedPostingId);
				} else {
					url.searchParams.delete("selected");
				}
			}, true);
		}
	});

	function cursorTrailMatches(left: string[], right: string[]): boolean {
		if (left.length !== right.length) {
			return false;
		}
		return left.every((value, index) => value === right[index]);
	}

	function navigate(update: (url: URL) => void, replaceState = false) {
		const url = new URL(page.url);
		update(url);
		goto(url.toString(), { replaceState, noScroll: true, keepFocus: true });
	}

	function setCursorTrail(url: URL, cursorTrail: string[]) {
		url.searchParams.delete("cursor");
		for (const cursor of cursorTrail) {
			url.searchParams.append("cursor", cursor);
		}
	}

	function clearPaging(url: URL) {
		setCursorTrail(url, []);
	}

	function handleGroupChange(nextGroup: GroupId) {
		navigate((url) => {
			url.searchParams.set("group", nextGroup);
			clearPaging(url);
			url.searchParams.delete("selected");
		});
	}

	function handleSort(nextSort: TransactionsSortColumn) {
		const nextDirection: TransactionsSortDirection =
			sortColumn === nextSort
				? sortDirection === "asc"
					? "desc"
					: "asc"
				: nextSort === "cleanDescription" || nextSort === "pairAccountId"
					? "asc"
					: "desc";

		navigate((url) => {
			url.searchParams.set("sort", nextSort);
			url.searchParams.set("dir", nextDirection);
			clearPaging(url);
			url.searchParams.delete("selected");
		});
	}

	function handleSearchSubmit(event: SubmitEvent) {
		event.preventDefault();
		navigate((url) => {
			if (searchInput.trim().length > 0) {
				url.searchParams.set("search", searchInput.trim());
			} else {
				url.searchParams.delete("search");
			}
			clearPaging(url);
			url.searchParams.delete("selected");
		});
	}

	function handleClearSearch() {
		searchInput = "";
		navigate((url) => {
			url.searchParams.delete("search");
			clearPaging(url);
			url.searchParams.delete("selected");
		});
	}

	function handleSelect(postingId: string) {
		navigate(
			(url) => {
				url.searchParams.set("selected", postingId);
			},
			true,
		);
	}

	function handleNextPage() {
		if (!list.nextCursorToken) {
			return;
		}
		navigate((url) => {
			setCursorTrail(url, [...list.cursorTrail, list.nextCursorToken!]);
			url.searchParams.delete("selected");
		});
	}

	function handlePreviousPage() {
		if (list.cursorTrail.length === 0) {
			return;
		}
		navigate((url) => {
			setCursorTrail(url, list.cursorTrail.slice(0, -1));
			url.searchParams.delete("selected");
		});
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
		if (sortColumn !== column) {
			return "";
		}
		return sortDirection === "asc" ? "↑" : "↓";
	}

	function getPairLabel(item: TransactionListItem): string {
		if (item.counterparty) {
			return item.counterparty;
		}
		if (item.pairAccountIds.length > 0) {
			return item.pairAccountIds.join(" · ");
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
		activeGroup={group}
		onGroupChange={handleGroupChange}
		{availableGroups}
		{groupMetadata}
		loading={connection.loading}
		error={connection.error}
		detail={connection.detail}
	/>

	<section class="border border-border bg-panel flex-1 min-h-0 grid xl:grid-cols-[minmax(0,1.8fr)_minmax(320px,1fr)] fade-in overflow-hidden">
		<div class="min-h-0 flex flex-col">
			<header class="border-b border-border p-2.5 flex flex-col gap-2">
				<div class="flex flex-col gap-1 lg:flex-row lg:items-end lg:justify-between">
					<div>
						<h2 class="font-normal text-sm uppercase tracking-widest">Transactions</h2>
						<div class="text-sm mt-0.5 leading-snug uppercase tracking-wider text-muted">
							Showing {list.rangeStart}-{list.rangeEnd} of {list.totalCount} for {groupMetadata[group]?.label ?? group}
						</div>
					</div>
					<div class="text-2xs uppercase tracking-widest text-muted">
						Page {list.pageNumber} · {list.count} loaded · {list.pageSize} per page
					</div>
				</div>

				<div class="flex flex-col gap-2 lg:flex-row lg:items-center lg:justify-between">
					<form class="flex flex-col gap-2 sm:flex-row sm:items-center" onsubmit={handleSearchSubmit}>
						<label class="sr-only" for="transaction-search">Search transactions</label>
						<input
							id="transaction-search"
							type="search"
							bind:value={searchInput}
							placeholder="Search description, counterparty, or account"
							class="min-h-[44px] w-full sm:w-[24rem] bg-transparent border border-border-subtle px-3 text-sm outline-none focus:border-text placeholder:text-muted"
						/>
						<div class="flex items-center gap-2">
							<button
								type="submit"
								class="min-h-[44px] px-3 border border-border text-2xs uppercase tracking-widest hover:border-text"
							>
								Apply
							</button>
							{#if searchQuery}
								<button
									type="button"
									class="min-h-[44px] px-3 border border-border-subtle text-2xs uppercase tracking-widest text-muted hover:text-text"
									onclick={handleClearSearch}
								>
									Clear
								</button>
							{/if}
						</div>
					</form>

					<div class="flex items-center gap-2 text-2xs uppercase tracking-widest text-muted">
						<button
							type="button"
							class="min-h-[44px] px-3 border border-border-subtle hover:border-text disabled:opacity-40 disabled:cursor-not-allowed"
							onclick={handlePreviousPage}
							disabled={list.cursorTrail.length === 0}
						>
							Previous
						</button>
						<button
							type="button"
							class="min-h-[44px] px-3 border border-border-subtle hover:border-text disabled:opacity-40 disabled:cursor-not-allowed"
							onclick={handleNextPage}
							disabled={!list.hasMore || !list.nextCursorToken}
						>
							Next
						</button>
					</div>
				</div>
			</header>

			<div class="border-b border-border bg-panel flex text-sm">
				{#each SORT_COLUMNS as column}
					<button
						type="button"
						class="min-h-[44px] flex-1 min-w-0 text-left p-2 text-2xs uppercase tracking-widest text-muted font-normal cursor-pointer hover:text-text transition-colors bg-transparent border-0 appearance-none"
						onclick={() => handleSort(column.id)}
					>
						{column.label}
						{#if getSortArrow(column.id)}
							<span class="ml-1">{getSortArrow(column.id)}</span>
						{/if}
					</button>
				{/each}
			</div>

			<div class="flex-1 overflow-auto min-h-0">
				{#if list.items.length === 0}
					<div class="h-full flex flex-col items-center justify-center gap-2 p-6 text-center">
						<div class="text-sm uppercase tracking-widest">No transactions found</div>
						<div class="text-sm text-muted leading-relaxed max-w-xl">
							Adjust the group, clear the search filter, or reset the page cursor to return to the latest transactions.
						</div>
						<div class="text-2xs uppercase tracking-widest text-muted">{connection.detail}</div>
					</div>
				{:else}
					<div class="divide-y divide-border-subtle">
						{#each list.items as item (item.postingId)}
							<button
								type="button"
								class="w-full text-left px-2.5 py-2 grid gap-2 transition-colors hover:bg-panel-subtle"
								class:bg-panel-subtle={selectedPostingId === item.postingId}
								aria-pressed={selectedPostingId === item.postingId}
								onclick={() => handleSelect(item.postingId)}
							>
								<div class="flex items-start justify-between gap-3 md:hidden">
									<div class="min-w-0">
										<div class="text-sm leading-snug truncate">{item.cleanDescription}</div>
										<div class="text-2xs uppercase tracking-widest text-muted mt-1">
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

								<div class="hidden md:grid md:grid-cols-[120px_minmax(0,1.6fr)_minmax(0,1fr)_140px] md:items-center md:gap-3">
									<div class="text-sm text-muted tabular-nums whitespace-nowrap">{formatDate(item.postedDate)}</div>
									<div class="min-w-0">
										<div class="text-sm leading-snug truncate">{item.cleanDescription}</div>
										<div class="text-2xs uppercase tracking-widest text-muted truncate mt-1">
											{item.counterparty ?? item.rawDescription}
										</div>
									</div>
									<div class="text-sm text-muted truncate">{getPairLabel(item)}</div>
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
					Server-backed detail for the selected posting
				</div>
			</header>

			<div class="flex-1 overflow-auto min-h-0 p-2.5">
				{#if selectedTransaction}
					<div class="flex flex-col gap-3">
						<div class="border border-border-subtle p-2.5 flex flex-col gap-1.5">
							<div class="flex items-start justify-between gap-3">
								<div>
									<div class="text-lg leading-tight">{selectedTransaction.cleanDescription ?? selectedTransaction.description}</div>
									<div class="text-2xs uppercase tracking-widest text-muted mt-1">
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

	<footer class="px-0.5 text-2xs uppercase tracking-widest text-muted">
		{connection.detail}
	</footer>
</main>
