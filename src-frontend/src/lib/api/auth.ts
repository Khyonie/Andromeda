import { page } from '$app/state';

export interface Account {
	id: string;
	discord_id: string;
	username: string;
	display_name: string;
	avatar: string | null;
}
export interface Session {
	user: Account | null;
	csrf_token: string | null;
	login_available: boolean;
	is_admin: boolean;
}

export async function authenticatedFetch(url: string, options: RequestInit = {}) {
	const headers = new Headers(options.headers);
	const method = options.method ?? 'GET';
	if (method !== 'GET' && method !== 'HEAD') {
		const csrf = page.data.session.csrf_token;
		if (!csrf) { window.location.assign('/login'); throw new Error('Please sign in again.'); }
		headers.set('X-CSRF-Token', csrf);
	}
	const response = await fetch(url, { ...options, headers });
	if (response.status === 401) {
		window.location.assign('/login');
		throw new Error('Your session expired. Please sign in again.');
	}
	return response;
}

export async function logout() {
	const response = await authenticatedFetch('/api/auth/logout', { method: 'POST' });
	if (!response.ok) {
		const message = response.status === 403 ? (await response.text()).trim() : '';
		throw new Error(message || 'Could not sign out. Please try again.');
	}
	window.location.assign('/login');
}
