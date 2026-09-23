import { getInstance } from '$lib/server/instances';
import type { PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ params, request, parent }) => {
	await parent();
	try {
		return { instance: await getInstance(params.id, request.headers), loadError: '' };
	} catch (cause) {
		return { instance: null, loadError: cause instanceof Error ? cause.message : 'Could not load this instance.' };
	}
};
