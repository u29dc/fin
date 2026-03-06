import { json } from '@sveltejs/kit';

import { loadShellState } from '$lib/server/api';

export async function GET() {
    const shell = await loadShellState();
    return json(
        {
            connection: shell.connection,
        },
        {
            headers: {
                'cache-control': 'no-store',
            },
        }
    );
}
