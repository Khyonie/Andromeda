<script lang="ts">
	import { getInstanceStatus, type InstanceStatus } from '$lib/api/instances';
	import UsageBar from './UsageBar.svelte';

	let { id, refreshToken = 0, status = $bindable(null), loading = $bindable(true) }: {
		id: string; refreshToken?: number; status?: InstanceStatus | null; loading?: boolean;
	} = $props();
	let statusError = $state('');
	let updated = $state('');
	const root = $derived(status?.filesystems?.find((fs) => fs.mountpoint === '/'));


	function bytes(value: number | null | undefined) {
		if (value == null) return 'Unavailable';
		if (value < 1024 ** 3) return `${(value / 1024 ** 2).toLocaleString(undefined, { maximumFractionDigits: 1 })} MiB`;
		return `${(value / 1024 ** 3).toLocaleString(undefined, { maximumFractionDigits: 2 })} GiB`;
	}

	function percentage(used: number | null | undefined, total: number | null | undefined) {
		return used != null && total != null && total > 0 ? used / total * 100 : null;
	}

	$effect(() => {
		const instanceId = id;
		void refreshToken;
		const controller = new AbortController();
		let timer: ReturnType<typeof setTimeout>;
		status = null;
		statusError = '';
		updated = '';
		loading = true;
		async function poll() {
			try {
				if (!document.hidden) {
					const next = await getInstanceStatus(instanceId, controller.signal);
					if (controller.signal.aborted) return;
					status = next;
					statusError = '';
					updated = new Date().toLocaleTimeString();
				}
			} catch (cause) {
				if (controller.signal.aborted) return;
				status = null; // Stale readings must not look live or enable state-dependent actions.
				statusError = cause instanceof Error ? cause.message : 'Could not read instance status.';
			} finally {
				if (!controller.signal.aborted) {
					loading = false;
					timer = setTimeout(poll, 5000);
				}
			}
		}
		void poll();
		return () => { controller.abort(); clearTimeout(timer); };
	});
</script>

<section aria-labelledby="status-heading" class="panel live-status">
	<div class="panel-header"><div><h2 id="status-heading">Live status</h2><p class="muted small">{loading ? 'Reading status…' : statusError ? 'Status unavailable · retrying automatically' : updated ? `Updated ${updated} · refreshes every 5 seconds` : 'Refreshes while this tab is visible'}</p></div></div>
	{#if statusError}<p class="error-notice" role="status">{statusError}</p>{/if}
	<div class="metric-rows">
		<section class="metric-row" aria-labelledby="cpu-label">
			<div class="metric-label"><h3 id="cpu-label">CPU usage</h3><p class="muted small">{status?.cpu_percent == null ? 'Waiting for two CPU samples' : `Across ${status.vcpus} virtual CPUs`}</p></div>
			<div class="metric-reading"><p class="usage-value">{status?.cpu_percent == null ? 'Unavailable' : `${status.cpu_percent.toFixed(1)}%`}</p><UsageBar label="CPU usage across all virtual CPUs" percent={status?.cpu_percent} /></div>
		</section>
		<section class="metric-row" aria-labelledby="ram-label">
			<div class="metric-label"><h3 id="ram-label">RAM usage</h3><p class="muted small">{status?.memory_total_bytes != null ? 'Usable guest memory' : status?.active ? 'Waiting for guest memory statistics' : 'Available while the guest is active'}</p></div>
			<div class="metric-reading"><p class="usage-value">{bytes(status?.memory_used_bytes)}{#if status?.memory_total_bytes != null}<span> / {bytes(status.memory_total_bytes)}</span>{/if}</p><UsageBar label="Guest RAM usage" percent={percentage(status?.memory_used_bytes, status?.memory_total_bytes)} />{#if status}<p class="muted small">Configured allocation: {bytes(status.memory_allocated_bytes)}</p>{/if}</div>
		</section>
		<section class="metric-row" aria-labelledby="disk-label">
			<div class="metric-label"><h3 id="disk-label">Disk usage</h3><p class="muted small">Root filesystem · /</p></div>
			<div class="metric-reading">
				<p class="usage-value">{bytes(root?.used_bytes)}{#if root}<span> / {bytes(root.total_bytes)}</span>{/if}</p>
				<UsageBar label="Root filesystem usage" percent={percentage(root?.used_bytes, root?.total_bytes)} />
				{#if !root}<p class="muted small">Requires a running guest with QEMU guest agent</p>{/if}
				{#each status?.filesystems?.filter((fs) => fs.mountpoint !== '/') ?? [] as fs}
					<div class="filesystem-reading"><p class="muted small">{fs.mountpoint}</p><p class="filesystem-value">{bytes(fs.used_bytes)} / {bytes(fs.total_bytes)}</p><UsageBar label={`Filesystem usage on ${fs.mountpoint}`} percent={percentage(fs.used_bytes, fs.total_bytes)} /></div>
				{/each}
				{#if status}<dl class="disk-allocation"><div><dt>Virtual disk capacity</dt><dd>{bytes(status.disk_capacity_bytes)}</dd></div><div><dt>Host allocation (instance layer)</dt><dd>{bytes(status.disk_allocated_bytes)}</dd></div></dl>{/if}
			</div>
		</section>
	</div>
</section>
