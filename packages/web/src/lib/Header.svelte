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

	function handleGroupTabKeydown(event: KeyboardEvent, currentIndex: number) {
		let newIndex = currentIndex;

		switch (event.key) {
			case 'ArrowLeft':
				newIndex = currentIndex > 0 ? currentIndex - 1 : availableGroups.length - 1;
				break;
			case 'ArrowRight':
				newIndex = currentIndex < availableGroups.length - 1 ? currentIndex + 1 : 0;
				break;
			case 'Home':
				newIndex = 0;
				break;
			case 'End':
				newIndex = availableGroups.length - 1;
				break;
			default:
				return;
		}

		event.preventDefault();
		const newGroupId = availableGroups[newIndex];
		if (newGroupId) {
			onGroupChange(newGroupId);
			// Focus the newly selected tab
			const tablist = (event.target as HTMLElement).closest('[role="tablist"]');
			const tabs = tablist?.querySelectorAll('[role="tab"]');
			(tabs?.[newIndex] as HTMLElement)?.focus();
		}
	}
</script>

<header
	class="sticky top-0 z-10 py-1.5 px-2.5 bg-bg border-b border-border-subtle flex justify-between items-center gap-2.5"
>
	<!-- Left: Page Navigation -->
	<nav class="flex items-center gap-0.5" aria-label="Page navigation">
		<a
			href="/"
			class="min-h-[44px] min-w-[44px] flex items-center justify-center border-t-2"
			class:border-text={activePage === 'dashboard'}
			class:text-text={activePage === 'dashboard'}
			class:border-transparent={activePage !== 'dashboard'}
			class:text-muted={activePage !== 'dashboard'}
			class:hover:text-text={activePage !== 'dashboard'}
			aria-current={activePage === 'dashboard' ? 'page' : undefined}
			aria-label="Dashboard"
		>
			<LayoutGrid class="size-4" aria-hidden="true" />
		</a>
		<a
			href="/overview"
			class="min-h-[44px] min-w-[44px] flex items-center justify-center border-t-2"
			class:border-text={activePage === 'overview'}
			class:text-text={activePage === 'overview'}
			class:border-transparent={activePage !== 'overview'}
			class:text-muted={activePage !== 'overview'}
			class:hover:text-text={activePage !== 'overview'}
			aria-current={activePage === 'overview' ? 'page' : undefined}
			aria-label="Overview"
		>
			<Layers class="size-4" aria-hidden="true" />
		</a>
		<a
			href="/transactions"
			class="min-h-[44px] min-w-[44px] flex items-center justify-center border-t-2"
			class:border-text={activePage === 'transactions'}
			class:text-text={activePage === 'transactions'}
			class:border-transparent={activePage !== 'transactions'}
			class:text-muted={activePage !== 'transactions'}
			class:hover:text-text={activePage !== 'transactions'}
			aria-current={activePage === 'transactions' ? 'page' : undefined}
			aria-label="Transactions"
		>
			<List class="size-4" aria-hidden="true" />
		</a>
	</nav>

	<!-- Center: Group Tabs -->
	<nav class="absolute left-1/2 -translate-x-1/2 flex items-center gap-0.5" aria-label="Account group">
		<div role="tablist" tabindex="0" aria-label="Account groups" class="flex items-center gap-0.5">
			{#each availableGroups as groupId, index (groupId)}
				{@const gid = groupId!}
				{@const isActive = allGroupsActive || activeGroup === gid}
				{@const Icon = getGroupIcon(gid)}
				{@const label = getGroupLabel(gid)}
				<button
					type="button"
					role="tab"
					class="min-h-[44px] px-3 flex items-center justify-center border-t-2 cursor-pointer"
					class:border-text={isActive}
					class:text-text={isActive}
					class:border-transparent={!isActive}
					class:text-muted={!isActive}
					class:hover:text-text={!isActive}
					aria-selected={isActive}
					tabindex={isActive ? 0 : -1}
					aria-label={label}
					onclick={() => onGroupChange(gid)}
					onkeydown={(e) => handleGroupTabKeydown(e, index)}
				>
					<Icon class="size-4 md:hidden" aria-hidden="true" />
					<span class="hidden md:inline text-xs uppercase tracking-widest leading-none">{label}</span>
				</button>
			{/each}
		</div>
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
