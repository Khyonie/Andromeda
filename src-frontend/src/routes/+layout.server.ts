import { redirect } from '@sveltejs/kit';
import { getSession } from '$lib/server/auth';
import type { LayoutServerLoad } from './$types';

export const load: LayoutServerLoad = async ({ request, url, setHeaders }) => {
	setHeaders({ 'cache-control': 'no-store' });
	let session;
	let authError = '';
	try { session = await getSession(request.headers); }
	catch { session = { user: null, csrf_token: null, login_available: false, is_admin: false }; authError = 'Could not reach the management server. Please try again shortly.'; }
	if (!session.user && url.pathname !== '/login') redirect(303, '/login');
	if (session.user && url.pathname === '/login' && !url.searchParams.has('error')) redirect(303, '/');
	return { session, authError };
};
