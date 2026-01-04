<script lang="ts">
	import Briefcase from '@lucide/svelte/icons/briefcase';
	import Building from '@lucide/svelte/icons/building';
	import Heart from '@lucide/svelte/icons/heart';
	import Layers from '@lucide/svelte/icons/layers';
	import LayoutGrid from '@lucide/svelte/icons/layout-grid';
	import List from '@lucide/svelte/icons/list';
	import PiggyBank from '@lucide/svelte/icons/piggy-bank';
	import User from '@lucide/svelte/icons/user';
	import Wallet from '@lucide/svelte/icons/wallet';
	import ThemeToggle from '$lib/ThemeToggle.svelte';

	type GroupId = string;
	type PageId = 'dashboard' | 'transactions' | 'overview';
	type GroupMeta = { label: string; icon: string };

	type Props = {
		activePage: PageId;
		activeGroup: GroupId;
		onGroupChange: (group: GroupId) => void;
		availableGroups: GroupId[];
		groupMetadata: Record<string, GroupMeta>;
		allGroupsActive?: boolean;
		loading?: boolean;
		error?: string | null;
	};

	const { activePage, activeGroup, onGroupChange, availableGroups, groupMetadata, allGroupsActive = false, loading = false, error = null }: Props =
		$props();

	// Icon mapping from config icon names to Lucide components
	const ICON_MAP: Record<string, typeof Briefcase> = {
		briefcase: Briefcase,
		user: User,
		heart: Heart,
		building: Building,
		wallet: Wallet,
		'piggy-bank': PiggyBank,
	};

	function getGroupLabel(groupId: string): string {
		return groupMetadata[groupId]?.label ?? groupId.charAt(0).toUpperCase() + groupId.slice(1);
	}

	function getGroupIcon(groupId: string): typeof Briefcase {
		const iconName = groupMetadata[groupId]?.icon ?? 'wallet';
		return ICON_MAP[iconName] ?? Wallet;
	}
</script>

<header
	class="sticky top-0 z-10 py-3 px-2.5 bg-bg border-b border-border-subtle flex justify-between items-center gap-2.5"
>
	<!-- Left: Page Navigation -->
	<nav class="flex items-center gap-1" aria-label="Page navigation">
		<a
			href="/"
			class="p-2 border-t-2 transition-colors"
			class:border-text={activePage === 'dashboard'}
			class:text-text={activePage === 'dashboard'}
			class:border-transparent={activePage !== 'dashboard'}
			class:text-muted={activePage !== 'dashboard'}
			class:hover:text-text={activePage !== 'dashboard'}
			aria-current={activePage === 'dashboard' ? 'page' : undefined}
			title="Dashboard"
		>
			<LayoutGrid class="size-4" />
		</a>
		<a
			href="/overview"
			class="p-2 border-t-2 transition-colors"
			class:border-text={activePage === 'overview'}
			class:text-text={activePage === 'overview'}
			class:border-transparent={activePage !== 'overview'}
			class:text-muted={activePage !== 'overview'}
			class:hover:text-text={activePage !== 'overview'}
			aria-current={activePage === 'overview' ? 'page' : undefined}
			title="Overview"
		>
			<Layers class="size-4" />
		</a>
		<a
			href="/transactions"
			class="p-2 border-t-2 transition-colors"
			class:border-text={activePage === 'transactions'}
			class:text-text={activePage === 'transactions'}
			class:border-transparent={activePage !== 'transactions'}
			class:text-muted={activePage !== 'transactions'}
			class:hover:text-text={activePage !== 'transactions'}
			aria-current={activePage === 'transactions' ? 'page' : undefined}
			title="Transactions"
		>
			<List class="size-4" />
		</a>
	</nav>

	<!-- Center: Group Tabs -->
	<nav class="absolute left-1/2 -translate-x-1/2 flex items-center gap-1" aria-label="Account group">
		{#each availableGroups as groupId (groupId)}
			{@const isActive = allGroupsActive || activeGroup === groupId}
			{@const Icon = getGroupIcon(groupId)}
			{@const label = getGroupLabel(groupId)}
			<button
				type="button"
				class="p-2 border-t-2 cursor-pointer transition-colors"
				class:border-text={isActive}
				class:text-text={isActive}
				class:border-transparent={!isActive}
				class:text-muted={!isActive}
				class:hover:text-text={!isActive}
				aria-pressed={activeGroup === groupId}
				onclick={() => onGroupChange(groupId)}
				title={label}
			>
				<Icon class="size-4 md:hidden" />
				<span class="hidden md:inline text-xs uppercase tracking-widest leading-none">{label}</span>
			</button>
		{/each}
	</nav>

	<!-- Right: Status + Theme -->
	<div class="flex items-center gap-3">
		<div class="text-xs uppercase tracking-wider flex items-center gap-1.5" role="status" aria-live="polite">
			{#if error}
				<span class="hidden md:inline text-error">ERROR: {error}</span>
				<span class="size-1.5 rounded-full shrink-0 bg-error"></span>
			{:else if loading}
				<span class="hidden md:inline text-pending">CONNECTING</span>
				<span class="size-1.5 rounded-full shrink-0 bg-pending"></span>
			{:else}
				<span class="hidden md:inline text-success">API CONNECTED</span>
				<span class="size-1.5 rounded-full shrink-0 bg-success"></span>
			{/if}
		</div>
		<ThemeToggle />
	</div>
</header>
