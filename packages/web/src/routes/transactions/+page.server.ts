import { loadTransactionsPageData } from "$lib/server/transactions";

export function load({ url }: { url: URL }) {
	return loadTransactionsPageData({ url });
}
