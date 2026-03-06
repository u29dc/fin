<script lang="ts">
	import { goto } from "$app/navigation";
	import { page } from "$app/state";

	import Header from "$lib/Header.svelte";

	type GroupId = string;
	type GroupMeta = {
		label: string;
		icon: string;
	};
	type ConnectionState = {
		loading: boolean;
		error: string | null;
		detail: string;
	};
	type PlaceholderPanel = {
		kicker: string;
		title: string;
		description: string;
	};

	let { data } = $props();

	const availableGroups = $derived(data.availableGroups as GroupId[]);
	const groupMetadata = $derived(data.groupMetadata as Record<string, GroupMeta>);
	const connection = $derived(data.connection as ConnectionState);

	const dashboardPanels: PlaceholderPanel[] = [
		{
			kicker: "Cashflow",
			title: "KPI strip and monthly comparison",
			description:
				"Restore archive-grade cashflow context against fin-api with typed monthly series, anomaly markers, and reserve-aware summaries.",
		},
		{
			kicker: "Allocation",
			title: "Reserve and asset composition",
			description:
				"Bring back the segmented allocation view so available cash, tax reserve, expense reserve, savings, and investments are visible at a glance.",
		},
		{
			kicker: "Flow",
			title: "Distribution and hierarchy",
			description:
				"Rebuild the flow-of-funds and expense hierarchy panels from Rust read models instead of client-side business logic.",
		},
		{
			kicker: "History",
			title: "Account balance context",
			description:
				"Reconnect the dense archive layout to historical balance and contribution series without direct database access from SvelteKit.",
		},
	];

	let group = $state<GroupId>("personal");
	let initialized = $state(false);

	$effect.pre(() => {
		if (initialized) {
			return;
		}
		initialized = true;
		group = data.initialGroup ?? availableGroups[0] ?? "personal";
	});

	function handleGroupChange(nextGroup: GroupId) {
		group = nextGroup;
		const url = new URL(page.url);
		url.searchParams.set("group", nextGroup);
		goto(url.toString(), { replaceState: true, noScroll: true });
	}
</script>

<svelte:head>
	<title>Finances | Dashboard</title>
</svelte:head>

<main class="h-svh overflow-auto box-border px-2.5 pb-2.5 flex flex-col gap-2">
	<h1 class="sr-only">Financial Dashboard</h1>
	<Header
		activePage="dashboard"
		activeGroup={group}
		onGroupChange={handleGroupChange}
		{availableGroups}
		{groupMetadata}
		loading={connection.loading}
		error={connection.error}
	/>

	<section class="border border-border bg-panel p-2.5 fade-in">
		<header class="flex flex-col gap-1 md:flex-row md:items-end md:justify-between">
			<div>
				<p class="text-2xs uppercase tracking-widest text-muted">Restored Shell</p>
				<h2 class="text-sm uppercase tracking-widest">Dashboard Workspace Back On Main</h2>
			</div>
			<p class="max-w-2xl text-sm leading-relaxed text-muted">
				{connection.detail}
			</p>
		</header>
	</section>

	<section class="grid gap-2 md:grid-cols-2 xl:grid-cols-4">
		{#each dashboardPanels as panel}
			<article class="border border-border bg-panel p-2.5 flex flex-col gap-2 min-h-48 fade-in">
				<p class="text-2xs uppercase tracking-widest text-muted">{panel.kicker}</p>
				<h2 class="text-sm uppercase tracking-widest">{panel.title}</h2>
				<p class="text-sm leading-relaxed text-muted">{panel.description}</p>
			</article>
		{/each}
	</section>
</main>
