import { authenticatedFetch } from './auth';

export interface GuestDefaults {
	gateway_ip: string;
	sysadmin_ssh_key: string;
	revision: number;
}

export function parseGuestDefaults(value: unknown): GuestDefaults {
	if (!value || typeof value !== 'object' || !('gateway_ip' in value) || typeof value.gateway_ip !== 'string' ||
		!('sysadmin_ssh_key' in value) || typeof value.sysadmin_ssh_key !== 'string' ||
		!('revision' in value) || typeof value.revision !== 'number' || !Number.isSafeInteger(value.revision) || value.revision < 0) {
		throw new Error('The management server returned unexpected settings.');
	}
	return value as GuestDefaults;
}

export async function saveGuestDefaults(settings: GuestDefaults): Promise<GuestDefaults> {
	const response = await authenticatedFetch('/api/admin/settings', {
		method: 'PUT',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(settings),
		signal: AbortSignal.timeout(15_000)
	});
	if (!response.ok) throw new Error(await response.text() || 'Could not save guest defaults.');
	return parseGuestDefaults(await response.json());
}

export interface HostStatus {
	cpu: { logical_cpus: number; used_percent: number | null } | null;
	memory: { total_bytes: number; used_bytes: number; available_bytes: number } | null;
	storage: { paths: string[]; total_bytes: number; used_bytes: number; available_bytes: number }[];
	configured: { instances: number; vcpus: number; memory_bytes: number };
	warnings: string[];
	sampled_at: string;
}

export async function getHostStatus(signal: AbortSignal): Promise<HostStatus> {
	const response = await authenticatedFetch('/api/admin/host/status', {
		signal: AbortSignal.any([signal, AbortSignal.timeout(10_000)])
	});
	if (!response.ok) throw new Error(await response.text() || 'Could not read host status.');
	const value = await response.json();
	const integer = (n: unknown) => typeof n === 'number' && Number.isSafeInteger(n) && n >= 0;
	const capacity = (item: Record<string, unknown>) => item &&
		['total_bytes', 'used_bytes', 'available_bytes'].every((key) => integer(item[key])) &&
		Number(item.used_bytes) <= Number(item.total_bytes) && Number(item.available_bytes) <= Number(item.total_bytes);
	if (!value ||
		!(value.cpu === null || value.cpu && integer(value.cpu.logical_cpus) && value.cpu.logical_cpus > 0 &&
			(value.cpu.used_percent === null || typeof value.cpu.used_percent === 'number' && Number.isFinite(value.cpu.used_percent) && value.cpu.used_percent >= 0 && value.cpu.used_percent <= 100)) ||
		!(value.memory === null || capacity(value.memory)) ||
		!Array.isArray(value.storage) || !value.storage.every((disk: HostStatus['storage'][number]) => capacity(disk) &&
			Array.isArray(disk.paths) && disk.paths.length > 0 && disk.paths.every((path) => typeof path === 'string')) ||
		!value.configured || !['instances', 'vcpus', 'memory_bytes'].every((key) => integer(value.configured[key])) ||
		!Array.isArray(value.warnings) || !value.warnings.every((message: unknown) => typeof message === 'string') ||
		typeof value.sampled_at !== 'string' || !Number.isFinite(Date.parse(value.sampled_at))) {
		throw new Error('The management server returned unexpected host status data.');
	}
	return value as HostStatus;
}
