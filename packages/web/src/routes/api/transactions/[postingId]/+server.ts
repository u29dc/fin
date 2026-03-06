import { json } from "@sveltejs/kit";

import { createFinApiClient } from "$lib/server/api";
import { fetchTransactionDetail } from "$lib/server/transactions";

export async function GET({ params }: { params: { postingId: string } }) {
	const postingId = params.postingId?.trim();
	if (!postingId) {
		return json({ error: "postingId is required" }, { status: 400, headers: { "cache-control": "no-store" } });
	}

	const client = createFinApiClient();

	try {
		const detail = await fetchTransactionDetail(client, postingId);
		return json({ detail }, { headers: { "cache-control": "no-store" } });
	} catch (error) {
		return json(
			{ error: error instanceof Error ? error.message : "failed to load transaction detail" },
			{ status: 502, headers: { "cache-control": "no-store" } },
		);
	}
}
