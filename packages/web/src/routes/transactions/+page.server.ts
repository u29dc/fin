import {
	fallbackGroupMetadata,
	fallbackGroups,
	placeholderConnection,
	resolveGroup,
	resolveSort,
	resolveSortDirection,
} from "$lib/server/skeleton";

export function load({ url }: { url: URL }) {
	return {
		availableGroups: fallbackGroups,
		groupMetadata: fallbackGroupMetadata,
		initialGroup: resolveGroup(url),
		initialSort: resolveSort(url),
		initialDir: resolveSortDirection(url),
		connection: placeholderConnection,
	};
}
