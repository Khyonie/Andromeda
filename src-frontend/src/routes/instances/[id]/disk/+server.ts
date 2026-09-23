import { backendUrl, sessionCookie } from '$lib/server/auth';
import type { RequestHandler } from './$types';

/** Stream through the server so the browser can save large disks without a Blob in memory. */
export const POST: RequestHandler = async ({ params, request, url }) => {
	if (request.headers.get('origin') !== url.origin) return new Response('Invalid request origin', { status: 403 });
	const form = await request.formData();
	const csrf = form.get('csrf_token');
	if (typeof csrf !== 'string' || !csrf) return new Response('Please sign in again', { status: 403 });
	try {
		const response = await fetch(backendUrl('/api/instance/disk'), {
			method: 'POST',
			headers: { 'Content-Type': 'application/json', Cookie: sessionCookie(request.headers), 'X-CSRF-Token': csrf, Origin: url.origin },
			body: JSON.stringify({ id: params.id }),
			signal: request.signal
		});
		const headers = new Headers({ 'Cache-Control': 'no-store', 'X-Content-Type-Options': 'nosniff' });
		if (response.ok) {
			headers.set('Content-Type', 'application/octet-stream');
			headers.set('Content-Disposition', 'attachment; filename="instance.qcow2"');
			const length = response.headers.get('Content-Length');
			if (length) headers.set('Content-Length', length);
		} else {
			headers.set('Content-Type', 'text/plain; charset=utf-8');
		}
		return new Response(response.body, { status: response.status, headers });
	} catch {
		return new Response('Could not download the disk. Check that the management server is running, then try again.', {
			status: 502, headers: { 'Content-Type': 'text/plain; charset=utf-8', 'Cache-Control': 'no-store' }
		});
	}
};
