<script lang="ts">
	import { goto } from "$app/navigation";
	import { page } from "$app/state";

	import Header from "$lib/Header.svelte";

	type GroupId = string;
	type SortColumn = "postedAt" | "cleanDescription" | "pairAccountId" | "amountMinor";
	type SortDirection = "asc" | "desc";
	type GroupMeta = {
		label: string;
		icon: string;
	};
	type ConnectionState = {
		loading: boolean;
		error: string | null;
		detail: string;
	};

	let { data } = $props();

	const availableGroups = $derived(data.availableGroups as GroupId[]);
	const groupMetadata = $derived(data.groupMetadata as Record<string, GroupMeta>);
	const connection = $derived(data.connection as ConnectionState);

	let group = $state<GroupId>("personal");
	let sortColumn = $state<SortColumn>("postedAt");
	let sortDirection = $state<SortDirection>("desc");
	let initialized = $state(false);

	const placeholderRows = [
		["Scope", "Group-aware, paginated transaction inspection"],
		["Sort", "Server-driven order and stable cursor semantics"],
		["Detail", "Selected-row drill-down backed by /v1/view/transactions/{postingId}"],
	] as const;

	$effect.pre(() => {
		if (initialized) {
			return;
		}
		initialized = true;
		group = data.initialGroup ?? availableGroups[0] ?? "personal";
		sortColumn = data.initialSort ?? "postedAt";
		sortDirection = data.initialDir ?? "desc";
	});

	function updateUrl() {
		const url = new URL(page.url);
		url.searchParams.set("group", group);
		url.searchParams.set("sort", sortColumn);
		url.searchParams.set("dir", sortDirection);
		goto(url.toString(), { replaceState: true, noScroll: true });
	}

	function handleGroupChange(nextGroup: GroupId) {
		group = nextGroup;
		updateUrl();
	}

	function handleSort(nextSort: SortColumn) {
		if (sortColumn === nextSort) {
			sortDirection = sortDirection === "asc" ? "desc" : "asc";
		} else {
			sortColumn = nextSort;
			sortDirection = "desc";
		}
		updateUrl();
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
	/>

	<section class="border border-border bg-panel flex-1 flex flex-col min-h-0 fade-in">
		<h2 class="px-2 py-1 text-2xs uppercase tracking-widest text-muted border-b border-border">
			Transactions surface restored; paginated fin-api wiring next
		</h2>
		<div class="border-b border-border bg-panel flex-shrink-0">
			<div class="flex text-sm">
				{#each [
					["postedAt", "Date"],
					["cleanDescription", "Title"],
					["pairAccountId", "Pair"],
					["amountMinor", "Amount"],
				] as [column, label]}
					<button
						type="button"
						class="min-h-[44px] flex-1 min-w-0 text-left p-2 text-2xs uppercase tracking-widest text-muted font-normal cursor-pointer hover:text-text transition-colors bg-transparent border-0 appearance-none"
						onclick={() => handleSort(column as SortColumn)}
					>
						{label}
						{#if sortColumn === column}
							<span class="ml-1">{sortDirection === "asc" ? "\u2191" : "\u2193"}</span>
						{/if}
					</button>
				{/each}
			</div>
		</div>
		<div class="flex-1 overflow-auto min-h-0 p-2.5">
			<p class="text-sm leading-relaxed text-muted">{connection.detail}</p>
			<div class="mt-3 grid gap-2">
				{#each placeholderRows as [label, value]}
					<div class="border border-border-subtle p-2 flex flex-col gap-1 md:flex-row md:items-center md:justify-between">
						<div class="text-2xs uppercase tracking-widest text-muted">{label}</div>
						<div class="text-sm leading-relaxed">{value}</div>
					</div>
				{/each}
			</div>
		</div>
	</section>
</main>
