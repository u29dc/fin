import { fallbackGroupMetadata, fallbackGroups, placeholderConnection, resolveGroup } from "$lib/server/skeleton";

export function load({ url }: { url: URL }) {
	return {
		availableGroups: fallbackGroups,
		groupMetadata: fallbackGroupMetadata,
		initialGroup: resolveGroup(url),
		connection: placeholderConnection,
	};
}
