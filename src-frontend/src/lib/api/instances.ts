import { authenticatedFetch } from './auth';

export interface Permissions { view: boolean; control: boolean; download: boolean; delete: boolean; manage_access: boolean }
export interface Instance {
	id: string;
	owner_id: string;
	role: 'owner' | 'operator' | 'viewer';
	permissions: Permissions;
	hostname: string;
	memory_mib: number;
	vcpus: number;
	mac_address: string;
	ipv4_address: string;
	remote_port: number;
	service_port: number;
}

export interface VmConfig {
	user: { name: string; key: string };
	networking: { ip: string; dhcp: boolean; mac: string; remote_port: number; service_port: number };
	instance: { hostname: string; 'disk-size': number; memory: number; vcpus: number };
}

export type InstanceAction = 'start' | 'shutdown' | 'force-shutdown' | 'delete';

export function parseInstance(value: unknown): Instance {
	if (typeof value !== 'object' || value === null) throw new Error('Unexpected instance response.');
	const record = value as Record<string, unknown>;
	const strings = ['id', 'owner_id', 'hostname', 'mac_address', 'ipv4_address'];
	const numbers = ['memory_mib', 'vcpus', 'remote_port', 'service_port'];
	if (!strings.every((key) => typeof record[key] === 'string') ||
		!numbers.every((key) => typeof record[key] === 'number' && Number.isFinite(record[key]))) {
		throw new Error('The management server returned unexpected instance data.');
	}
	const permissions = record.permissions as Record<string, unknown> | null;
	if (!['owner', 'operator', 'viewer'].includes(String(record.role)) || !permissions ||
		!['view', 'control', 'download', 'delete', 'manage_access'].every((key) => typeof permissions[key] === 'boolean')) {
		throw new Error('The management server returned unexpected instance permissions.');
	}
	return value as Instance;
}

async function request(path: string, method = 'GET', body?: unknown): Promise<Response> {
	let response: Response;
	try {
		response = await authenticatedFetch(path, {
			method,
			headers: { Accept: 'application/json', ...(body === undefined ? {} : { 'Content-Type': 'application/json' }) },
			body: body === undefined ? undefined : JSON.stringify(body),
			// Creation may include disk and ISO preparation. Never retry a mutation automatically.
			signal: AbortSignal.timeout(method === 'GET' ? 10_000 : 120_000)
		});
	} catch {
		throw new Error(method === 'GET'
			? 'Could not reach the management server. Check that it is running, then try again.'
			: 'No response from the management server. The operation may still have completed; check your instances before trying again.');
	}
	if (!response.ok) {
		const message = await response.text();
		throw new Error(message || `The management server returned HTTP ${response.status}.`);
	}
	return response;
}

export async function listInstanceIds(): Promise<string[]> {
	const ids: unknown = await (await request('/api/instance')).json();
	if (!Array.isArray(ids) || !ids.every((id): id is string => typeof id === 'string')) {
		throw new Error('The management server returned an unexpected instance list.');
	}
	return ids;
}

export async function createInstance(config: VmConfig): Promise<string | null> {
	const response = await request('/api/instance', 'PUT', config);
	if (response.status === 202) return null; // Backend dry run: nothing was persisted.
	const result: unknown = await response.json();
	if (typeof result !== 'object' || result === null || !('id' in result) || typeof result.id !== 'string' || !result.id) {
		throw new Error('Creation returned no instance ID. Check the instance list before trying again.');
	}
	return result.id;
}

export async function performInstanceAction(id: string, action: InstanceAction): Promise<void> {
	const endpoints = {
		start: ['/api/instance/start', 'POST'],
		shutdown: ['/api/instance/stop', 'POST'],
		'force-shutdown': ['/api/instance/stop', 'DELETE'],
		delete: ['/api/instance', 'DELETE']
	} as const;
	const [path, method] = endpoints[action];
	await request(path, method, { id });
}

export interface FilesystemUsage {
	mountpoint: string;
	used_bytes: number;
	total_bytes: number;
}

export interface InstanceStatus {
	state: string;
	active: boolean;
	cpu_percent: number | null;
	vcpus: number;
	memory_used_bytes: number | null;
	memory_total_bytes: number | null;
	memory_allocated_bytes: number;
	disk_capacity_bytes: number | null;
	disk_allocated_bytes: number | null;
	filesystems: FilesystemUsage[] | null;
}

export async function getInstanceStatus(id: string, signal: AbortSignal): Promise<InstanceStatus> {
	const response = await authenticatedFetch('/api/instance/status', {
		method: 'POST',
		headers: { 'Content-Type': 'application/json', Accept: 'application/json' },
		body: JSON.stringify({ id }),
		signal: AbortSignal.any([signal, AbortSignal.timeout(10_000)])
	});
	if (!response.ok) throw new Error(await response.text() || `Status request failed (HTTP ${response.status}).`);
	const value = await response.json();
	const optionalNumbers = ['cpu_percent', 'memory_used_bytes', 'memory_total_bytes', 'disk_capacity_bytes', 'disk_allocated_bytes'];
	if (!value || typeof value.state !== 'string' || typeof value.active !== 'boolean' ||
		!['vcpus', 'memory_allocated_bytes'].every((key) => typeof value[key] === 'number' && Number.isFinite(value[key])) ||
		!optionalNumbers.every((key) => value[key] === null || typeof value[key] === 'number' && Number.isFinite(value[key])) ||
		!(value.filesystems === null || Array.isArray(value.filesystems) && value.filesystems.every((fs: FilesystemUsage) =>
			fs && typeof fs.mountpoint === 'string' && typeof fs.used_bytes === 'number' && Number.isFinite(fs.used_bytes) && typeof fs.total_bytes === 'number' && Number.isFinite(fs.total_bytes)))) {
		throw new Error('The management server returned unexpected status data.');
	}
	return value as InstanceStatus;
}
