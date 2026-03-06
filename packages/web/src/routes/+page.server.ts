import { loadDashboardPageData } from "$lib/server/dashboard";

export async function load({ url }: { url: URL }) {
	return loadDashboardPageData({ url });
}
