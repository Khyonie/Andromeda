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
