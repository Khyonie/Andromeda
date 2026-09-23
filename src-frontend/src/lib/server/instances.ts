import { request as httpRequest } from 'node:http';
import { request as httpsRequest } from 'node:https';
import { backendUrl, sessionCookie } from './auth';
import { parseInstance, type Instance } from '$lib/api/instances';

/** Node's HTTP client preserves the backend's GET-with-JSON contract; fetch cannot. */
export async function getInstance(id: string, headers: Headers): Promise<Instance> {
	const url = backendUrl('/api/instance/id');
	const body = JSON.stringify({ id });
	const request = url.protocol === 'https:' ? httpsRequest : httpRequest;
	const value = await new Promise<unknown>((resolve, reject) => {
		const req = request(url, {
			method: 'GET',
			headers: { Cookie: sessionCookie(headers), Accept: 'application/json', 'Content-Type': 'application/json', 'Content-Length': Buffer.byteLength(body) },
			signal: AbortSignal.timeout(10_000)
		}, (res) => {
			const chunks: Buffer[] = [];
			let size = 0;
			res.on('data', (chunk: Buffer) => {
				size += chunk.length;
				if (size > 1_048_576) {
					req.destroy(new Error('Instance response was too large.'));
					return;
				}
				chunks.push(chunk);
			});
			res.on('error', reject);
			res.on('end', () => {
				const text = Buffer.concat(chunks).toString('utf8');
				if (res.statusCode !== 200) {
					reject(new Error(res.statusCode === 404 ? 'This instance was not found. It may have been deleted.' : text || `Management server returned HTTP ${res.statusCode}.`));
					return;
				}
				try { resolve(JSON.parse(text)); }
				catch { reject(new Error('The management server returned invalid instance data.')); }
			});
		});
		req.on('error', () => reject(new Error('Could not reach the management server. Check that it is running, then try again.')));
		req.end(body);
	});
	return parseInstance(value);
}
