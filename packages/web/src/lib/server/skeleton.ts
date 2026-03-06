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

export const placeholderConnection: ConnectionState = {
	loading: true,
	error: "fin-api client pending",
	detail: "The archived shell is restored. Route data will be rewired to fin-api in the next ticket.",
};

export function resolveGroup(url: URL): GroupId {
	const group = url.searchParams.get("group");
	return group && DEFAULT_GROUPS.some(([id]) => id === group) ? group : fallbackGroups[0] ?? "personal";
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
