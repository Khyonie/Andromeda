<script lang="ts">
	import { onMount } from 'svelte';
	import { getHostStatus, type HostStatus } from '$lib/api/admin';
	import UsageBar from './UsageBar.svelte';

	let status = $state<HostStatus | null>(null);
	let loading = $state(true);
	let error = $state('');

	function bytes(value: number | undefined) {
		if (value === undefined) return 'Unavailable';
		const unit = value >= 1024 ** 4 ? 'TiB' : 'GiB';
		return `${(value / 1024 ** (unit === 'TiB' ? 4 : 3)).toLocaleString(undefined, { maximumFractionDigits: 1 })} ${unit}`;
	}
	function percent(used: number | undefined, total: number | undefined) {
		return used !== undefined && total !== undefined && total > 0 ? used / total * 100 : null;
	}

	onMount(() => {
		const controller = new AbortController();
		let polling = false;
		async function poll() {
			if (polling || document.hidden) return;
			polling = true;
			try {
				const next = await getHostStatus(controller.signal);
				if (controller.signal.aborted) return;
				status = next;
				error = '';
			} catch (cause) {
				if (controller.signal.aborted) return;
				status = null;
				error = cause instanceof Error ? cause.message : 'Could not read host status.';
			} finally {
				polling = false;
				if (!controller.signal.aborted) loading = false;
			}
		}
		void poll();
		const timer = setInterval(poll, 5000);
		document.addEventListener('visibilitychange', poll);
		return () => { controller.abort(); clearInterval(timer); document.removeEventListener('visibilitychange', poll); };
	});
</script>

<section class="panel host-status" aria-labelledby="host-status-heading">
	<div class="panel-header">
		<div><h2 id="host-status-heading">Host status</h2><p class="muted small">{loading ? 'Reading host resources…' : error ? 'Host status unavailable · retrying automatically' : status ? `Updated ${new Date(status.sampled_at).toLocaleTimeString()} · refreshes every 5 seconds` : 'Refreshes while this tab is visible'}</p></div>
	</div>
	{#if error}<p class="error-notice" role="status">{error}</p>{/if}
	{#each status?.warnings ?? [] as warning}<p class="error-notice" role="status">{warning}</p>{/each}
	<div class="host-metrics-grid">
		<section class="host-metric" aria-labelledby="host-cpu-heading">
			<h3 id="host-cpu-heading">CPU usage</h3>
			<p class="usage-value">{status?.cpu?.used_percent == null ? 'Unavailable' : `${status.cpu.used_percent.toFixed(1)}%`}</p>
			<UsageBar label="Host CPU usage" percent={status?.cpu?.used_percent} />
			<p class="muted small">{status?.cpu ? `${status.cpu.logical_cpus} logical CPUs` : 'Host CPU count unavailable'}{#if status?.cpu && status.cpu.used_percent === null} · waiting for two samples{/if}</p>
			{#if status}<p class="host-allocation">Configured for VMs: <strong>{status.configured.vcpus} vCPUs</strong></p>{/if}
		</section>
		<section class="host-metric" aria-labelledby="host-memory-heading">
			<h3 id="host-memory-heading">RAM usage</h3>
			<p class="usage-value">{bytes(status?.memory?.used_bytes)}{#if status?.memory}<span> / {bytes(status.memory.total_bytes)}</span>{/if}</p>
			<UsageBar label="Host RAM usage" percent={percent(status?.memory?.used_bytes, status?.memory?.total_bytes)} />
			<p class="muted small">Available: {bytes(status?.memory?.available_bytes)}</p>
			{#if status}<p class="host-allocation">Configured for VMs: <strong>{bytes(status.configured.memory_bytes)}</strong></p>{/if}
		</section>
		<section class="host-metric" aria-labelledby="host-storage-heading">
			<h3 id="host-storage-heading">Disk usage</h3>
			{#each status?.storage ?? [] as disk (disk.paths[0])}
				<div class="host-disk">
					<p class="usage-value">{bytes(disk.used_bytes)}<span> / {bytes(disk.total_bytes)}</span></p>
					<UsageBar label={`Disk usage for ${disk.paths[0]}`} percent={percent(disk.used_bytes, disk.total_bytes)} />
					<p class="muted small">Available: {bytes(disk.available_bytes)}</p>
					<details class="host-disk-paths"><summary>Storage paths ({disk.paths.length})</summary><ul>{#each disk.paths as path}<li><code>{path}</code></li>{/each}</ul></details>
				</div>
			{:else}
				<p class="usage-value">Unavailable</p>
				<UsageBar label="Host disk usage" percent={null} />
			{/each}
		</section>
	</div>
	<div class="host-status-note"><p class="muted small">{#if status}Configured totals cover {status.configured.instances} registered instance{status.configured.instances === 1 ? '' : 's'}, including stopped VMs. {/if} Available resources are a snapshot, not a reservation. Disk usage reflects actual filesystem space; VM disks may grow as guests write data.</p></div>
</section>
