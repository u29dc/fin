import { loadShellState } from "$lib/server/api";

export async function load() {
	return loadShellState();
}
