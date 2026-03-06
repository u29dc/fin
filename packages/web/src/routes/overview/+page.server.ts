import { fallbackGroupMetadata, fallbackGroups, placeholderConnection } from "$lib/server/skeleton";

export function load() {
	return {
		availableGroups: fallbackGroups,
		groupMetadata: fallbackGroupMetadata,
		connection: placeholderConnection,
	};
}
