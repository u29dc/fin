import { loadShellState } from "$lib/server/api";
import { resolveGroup, resolveSort, resolveSortDirection } from "$lib/server/skeleton";

export async function load({ url }: { url: URL }) {
	const shell = await loadShellState();
	return {
		...shell,
		initialGroup: resolveGroup(url, shell.availableGroups),
		initialSort: resolveSort(url),
		initialDir: resolveSortDirection(url),
	};
}
