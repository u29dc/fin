<script lang="ts">
	import { onMount } from 'svelte';

	import Header from '$lib/Header.svelte';

	type GroupId = string;

	type Transaction = {
		id: string;
		chartAccountId: string;
		pairAccountId: string;
		postedAt: string;
		amountMinor: number;
		currency: string;
		rawDescription: string;
		cleanDescription: string;
		counterparty: string | null;
	};

	type SortColumn = 'postedAt' | 'cleanDescription' | 'pairAccountId' | 'amountMinor';
	type SortDirection = 'asc' | 'desc';

	const ROW_HEIGHT = 41; // Fixed row height in pixels
	const BUFFER_ROWS = 10; // Extra rows to render above/below viewport

	let { data } = $props();

	// Available groups from server config
	const availableGroups = $derived(data.availableGroups);
	const groupMetadata = $derived(data.groupMetadata ?? {});

	// All transactions pre-fetched for each group (use $derived to maintain reactivity)
	const allTransactions = $derived(data.transactions as Record<string, Transaction[]>);

	// Default to first available group
	let group: GroupId = $state('');
	let sortColumn: SortColumn = $state('postedAt');

	$effect(() => {
		if (!group) {
			group = data.availableGroups[0] ?? '';
		}
	});
	let sortDirection: SortDirection = $state('desc');

	// Virtual scrolling state
	let scrollContainer: HTMLElement | null = $state(null);
	let scrollTop = $state(0);
	let containerHeight = $state(600);

	// Get transactions for current group
	const transactions = $derived(allTransactions[group] ?? []);

	const sortedTransactions = $derived.by(() => {
		const sorted = [...transactions];
		sorted.sort((a, b) => {
			let aVal: string | number | null = a[sortColumn];
			let bVal: string | number | null = b[sortColumn];

			if (aVal === null) aVal = '';
			if (bVal === null) bVal = '';

			if (typeof aVal === 'number' && typeof bVal === 'number') {
				return sortDirection === 'asc' ? aVal - bVal : bVal - aVal;
			}

			const comparison = String(aVal).localeCompare(String(bVal));
			return sortDirection === 'asc' ? comparison : -comparison;
		});
		return sorted;
	});

	// Virtual scrolling calculations
	const totalHeight = $derived(sortedTransactions.length * ROW_HEIGHT);
	const startIndex = $derived(Math.max(0, Math.floor(scrollTop / ROW_HEIGHT) - BUFFER_ROWS));
	const endIndex = $derived(
		Math.min(sortedTransactions.length, Math.ceil((scrollTop + containerHeight) / ROW_HEIGHT) + BUFFER_ROWS)
	);
	const visibleTransactions = $derived(sortedTransactions.slice(startIndex, endIndex));
	const offsetTop = $derived(startIndex * ROW_HEIGHT);

	function handleScroll(e: Event) {
		const target = e.target as HTMLElement;
		scrollTop = target.scrollTop;
	}

	function handleSort(column: SortColumn) {
		if (sortColumn === column) {
			sortDirection = sortDirection === 'asc' ? 'desc' : 'asc';
		} else {
			sortColumn = column;
			sortDirection = 'asc';
		}
		// Reset scroll position on sort
		if (scrollContainer) {
			scrollContainer.scrollTop = 0;
			scrollTop = 0;
		}
	}

	function formatMoney(minor: number): string {
		const value = minor / 100;
		return value.toLocaleString('en-GB', { style: 'currency', currency: 'GBP' });
	}

	function formatDate(isoDate: string): string {
		const date = new Date(isoDate);
		return date.toLocaleDateString('en-GB', {
			day: 'numeric',
			month: 'short',
			year: 'numeric',
		});
	}

	function handleGroupChange(newGroup: GroupId) {
		group = newGroup;
		// Reset scroll position on group change
		if (scrollContainer) {
			scrollContainer.scrollTop = 0;
			scrollTop = 0;
		}
	}

	onMount(() => {
		// Update container height on resize
		const updateHeight = () => {
			if (scrollContainer) {
				containerHeight = scrollContainer.clientHeight;
			}
		};

		updateHeight();
		const resizeObserver = new ResizeObserver(updateHeight);
		if (scrollContainer) {
			resizeObserver.observe(scrollContainer);
		}

		return () => resizeObserver.disconnect();
	});
</script>

<svelte:head>
	<title>Finances | Transactions</title>
</svelte:head>

<main class="h-svh overflow-hidden box-border px-2.5 pb-2.5 flex flex-col gap-2">
	<Header activePage="transactions" activeGroup={group} onGroupChange={handleGroupChange} availableGroups={availableGroups} {groupMetadata} loading={false} error={null} />

	<section class="border border-border bg-panel flex-1 flex flex-col min-h-0 fade-in">
		<!-- Fixed header -->
		<div class="border-b border-border bg-panel flex-shrink-0">
			<div class="flex text-sm">
				<div
					class="w-28 flex-shrink-0 text-left p-2 text-2xs uppercase tracking-widest text-muted font-normal cursor-pointer hover:text-text transition-colors select-none"
					onclick={() => handleSort('postedAt')}
					onkeydown={(e) => e.key === 'Enter' && handleSort('postedAt')}
					role="button"
					tabindex="0"
				>
					Date
					{#if sortColumn === 'postedAt'}
						<span class="ml-1">{sortDirection === 'asc' ? '\u2191' : '\u2193'}</span>
					{/if}
				</div>
				<div
					class="flex-1 min-w-0 text-left p-2 text-2xs uppercase tracking-widest text-muted font-normal cursor-pointer hover:text-text transition-colors select-none"
					onclick={() => handleSort('cleanDescription')}
					onkeydown={(e) => e.key === 'Enter' && handleSort('cleanDescription')}
					role="button"
					tabindex="0"
				>
					Title
					{#if sortColumn === 'cleanDescription'}
						<span class="ml-1">{sortDirection === 'asc' ? '\u2191' : '\u2193'}</span>
					{/if}
				</div>
				<div
					class="flex-1 min-w-0 text-left p-2 text-2xs uppercase tracking-widest text-muted font-normal cursor-pointer hover:text-text transition-colors select-none"
					onclick={() => handleSort('pairAccountId')}
					onkeydown={(e) => e.key === 'Enter' && handleSort('pairAccountId')}
					role="button"
					tabindex="0"
				>
					Pair
					{#if sortColumn === 'pairAccountId'}
						<span class="ml-1">{sortDirection === 'asc' ? '\u2191' : '\u2193'}</span>
					{/if}
				</div>
				<div
					class="w-28 flex-shrink-0 text-right p-2 text-2xs uppercase tracking-widest text-muted font-normal cursor-pointer hover:text-text transition-colors select-none"
					onclick={() => handleSort('amountMinor')}
					onkeydown={(e) => e.key === 'Enter' && handleSort('amountMinor')}
					role="button"
					tabindex="0"
				>
					Amount
					{#if sortColumn === 'amountMinor'}
						<span class="ml-1">{sortDirection === 'asc' ? '\u2191' : '\u2193'}</span>
					{/if}
				</div>
			</div>
		</div>

		<!-- Virtual scrolling body -->
		<div
			bind:this={scrollContainer}
			class="flex-1 overflow-auto min-h-0"
			onscroll={handleScroll}
		>
			{#if transactions.length === 0}
				<div class="p-8 text-center text-muted">No transactions found.</div>
			{:else}
				<!-- Virtual scroll container with total height -->
				<div style="height: {totalHeight}px; position: relative;">
					<!-- Visible rows positioned absolutely -->
					<div style="position: absolute; top: {offsetTop}px; left: 0; right: 0;">
						{#each visibleTransactions as txn (`${txn.id}-${txn.chartAccountId}`)}
							<div
								class="flex text-sm border-b border-border-subtle hover:bg-active-bg transition-colors"
								style="height: {ROW_HEIGHT}px;"
							>
								<div class="w-28 flex-shrink-0 p-2 text-muted tabular-nums whitespace-nowrap flex items-center">
									{formatDate(txn.postedAt)}
								</div>
								<div class="flex-1 min-w-0 p-2 text-text truncate flex items-center">
									{txn.cleanDescription}
								</div>
								<div class="flex-1 min-w-0 p-2 text-muted truncate flex items-center" title={txn.pairAccountId}>
									{txn.pairAccountId}
								</div>
								<div
									class="w-28 flex-shrink-0 p-2 text-right tabular-nums whitespace-nowrap flex items-center justify-end"
									class:text-success={txn.amountMinor > 0}
									class:text-error={txn.amountMinor < 0}
								>
									{formatMoney(txn.amountMinor)}
								</div>
							</div>
						{/each}
					</div>
				</div>
			{/if}
		</div>
	</section>
</main>
