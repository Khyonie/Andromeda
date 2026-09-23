import { env } from '$env/dynamic/private';
import type { Session } from '$lib/api/auth';

export function backendUrl(path: string) {
	return new URL(path, env.ANDROMEDA_API_URL || 'http://127.0.0.1:9966');
}

export function sessionCookie(headers: Headers) {
	return (headers.get('cookie') ?? '').split(';').map((part) => part.trim())
		.filter((part) => /^(?:__Host-)?andromeda_session=/.test(part)).join('; ');
}

export async function getSession(headers: Headers): Promise<Session> {
	const response = await fetch(backendUrl('/api/session'), {
		headers: { Cookie: sessionCookie(headers) }, signal: AbortSignal.timeout(10_000)
	});
	if (!response.ok) throw new Error('Could not check your session. Please try again.');
	return await response.json() as Session;
}
