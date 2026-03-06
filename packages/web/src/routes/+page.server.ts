import { loadShellState } from "$lib/server/api";
import { resolveGroup } from "$lib/server/skeleton";

export async function load({ url }: { url: URL }) {
	const shell = await loadShellState();
	return {
		...shell,
		initialGroup: resolveGroup(url, shell.availableGroups),
	};
}
