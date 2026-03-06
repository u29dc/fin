export type GroupId = string;

export type GroupMeta = {
	label: string;
	icon: string;
};

export type ConnectionState = {
	loading: boolean;
	error: string | null;
	detail: string;
};

const DEFAULT_GROUPS = [
	["personal", { label: "Personal", icon: "user" }],
	["joint", { label: "Joint", icon: "heart" }],
	["business", { label: "Business", icon: "briefcase" }],
] as const satisfies ReadonlyArray<readonly [GroupId, GroupMeta]>;

export const fallbackGroups = DEFAULT_GROUPS.map(([id]) => id);

export const fallbackGroupMetadata = Object.fromEntries(DEFAULT_GROUPS) as Record<GroupId, GroupMeta>;

export function resolveGroup(url: URL, availableGroups: readonly string[] = fallbackGroups): GroupId {
	const group = url.searchParams.get("group");
	return group && availableGroups.includes(group) ? group : availableGroups[0] ?? "personal";
}

export function resolveSort(url: URL): "postedAt" | "cleanDescription" | "pairAccountId" | "amountMinor" {
	const sort = url.searchParams.get("sort");
	switch (sort) {
		case "cleanDescription":
		case "pairAccountId":
		case "amountMinor":
		case "postedAt":
			return sort;
		default:
			return "postedAt";
	}
}

export function resolveSortDirection(url: URL): "asc" | "desc" {
	return url.searchParams.get("dir") === "asc" ? "asc" : "desc";
}
