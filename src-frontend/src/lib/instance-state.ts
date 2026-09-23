const labels: Record<string, string> = {
	running: 'Running', blocked: 'Running (waiting)', paused: 'Paused', 'shutting-down': 'Shutting down',
	'shut-off': 'Stopped', crashed: 'Crashed', suspended: 'Suspended', missing: 'Domain missing',
	unknown: 'Unknown', unavailable: 'Unavailable'
};

export function powerStateLabel(state: string): string {
	return labels[state] ?? 'Unknown';
}

export function powerStateTone(state: string): 'success' | 'neutral' | 'warning' {
	if (state === 'running' || state === 'blocked') return 'success';
	if (['crashed', 'missing', 'unknown', 'unavailable'].includes(state)) return 'warning';
	return 'neutral';
}
