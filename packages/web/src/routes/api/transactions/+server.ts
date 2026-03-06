import { json } from "@sveltejs/kit";

import { createFinApiClient } from "$lib/server/api";
import {
	fetchTransactionsDataset,
	type TransactionsSortColumn,
	type TransactionsSortDirection,
} from "$lib/server/transactions";

export async function GET({ url }: { url: URL }) {
	const group = url.searchParams.get("group")?.trim();
	if (!group) {
		return json({ error: "group is required" }, { status: 400, headers: { "cache-control": "no-store" } });
	}

	const sort = resolveSort(url.searchParams.get("sort"));
	const direction = resolveDirection(url.searchParams.get("dir"));
	const client = createFinApiClient();

	try {
		const list = await fetchTransactionsDataset(client, { group, sort, direction });
		return json({ list }, { headers: { "cache-control": "no-store" } });
	} catch (error) {
		return json(
			{ error: error instanceof Error ? error.message : "failed to load transactions" },
			{ status: 502, headers: { "cache-control": "no-store" } },
		);
	}
}

function resolveSort(value: string | null): TransactionsSortColumn {
	switch (value) {
		case "cleanDescription":
		case "pairAccountId":
		case "amountMinor":
		case "postedAt":
			return value;
		default:
			return "postedAt";
	}
}

function resolveDirection(value: string | null): TransactionsSortDirection {
	return value === "asc" ? "asc" : "desc";
}
