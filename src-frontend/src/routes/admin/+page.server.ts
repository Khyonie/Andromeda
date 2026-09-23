import { error } from '@sveltejs/kit';
import { backendUrl, sessionCookie } from '$lib/server/auth';
import { parseGuestDefaults } from '$lib/api/admin';
import type { PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ parent, request }) => {
	const { session } = await parent();
	if (!session.is_admin) error(403, 'System administrator access is required.');
	const response = await fetch(backendUrl('/api/admin/settings'), {
		headers: { Cookie: sessionCookie(request.headers) }, signal: AbortSignal.timeout(10_000)
	}).catch(() => error(503, 'Could not reach the management server.'));
	if (!response.ok) error(response.status, 'Could not load guest defaults.');
	return { settings: parseGuestDefaults(await response.json()) };
};
