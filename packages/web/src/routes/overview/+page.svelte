<script lang="ts">
	import Header from "$lib/Header.svelte";

	type GroupMeta = {
		label: string;
		icon: string;
	};
	type ConnectionState = {
		loading: boolean;
		error: string | null;
		detail: string;
	};
	type PlaceholderPoint = {
		title: string;
		description: string;
	};

	let { data } = $props();

	const availableGroups = $derived(data.availableGroups as string[]);
	const groupMetadata = $derived(data.groupMetadata as Record<string, GroupMeta>);
	const connection = $derived(data.connection as ConnectionState);

	const overviewPanels: PlaceholderPoint[] = [
		{
			title: "All-account balance history",
			description:
				"Reconnect the archive overview timeline to /v1/dashboard/balances without recreating SQL access in the web package.",
		},
		{
			title: "Runway projection",
			description:
				"Use the Rust projection report directly so warning and threshold semantics stay server-defined.",
		},
	];
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

	{#each overviewPanels as panel}
		<article class="border border-border bg-panel p-2.5 flex flex-col gap-2 fade-in">
			<header class="flex flex-col gap-1 md:flex-row md:items-end md:justify-between">
				<div>
					<h2 class="font-normal text-sm uppercase tracking-widest">{panel.title}</h2>
					<div class="text-sm mt-0.5 leading-snug uppercase tracking-wider text-muted">
						Archive overview surface queued for fin-api wiring
					</div>
				</div>
			</header>
			<p class="text-sm leading-relaxed text-muted">{panel.description}</p>
			<p class="text-2xs uppercase tracking-widest text-muted">{connection.detail}</p>
		</article>
	{/each}
</main>
