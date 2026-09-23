<script lang="ts">
	import { page } from '$app/state';
	import { goto, invalidateAll } from '$app/navigation';
	import InstanceMetrics from '$lib/components/InstanceMetrics.svelte';
	import StatusBadge from '$lib/components/StatusBadge.svelte';
	import { powerStateLabel, powerStateTone } from '$lib/instance-state';
	import { performInstanceAction, type InstanceAction, type InstanceStatus } from '$lib/api/instances';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();
	let pending = $state<InstanceAction | 'refresh' | null>(null);
	let error = $state('');
	let notice = $state('');
	let deleted = $state(false);
	let status = $state<InstanceStatus | null>(null);
	let statusRefresh = $state(0);
	let statusLoading = $state(true);
	const labels: Record<InstanceAction, string> = {
		start: 'Start', shutdown: 'Shutdown', 'force-shutdown': 'Force-shutdown', delete: 'Delete instance'
	};

	async function refresh() {
		if (pending) return;
		pending = 'refresh';
		try { await invalidateAll(); statusRefresh += 1; }
		catch { error = 'Could not refresh this page. Please try again.'; }
		finally { pending = null; }
	}

	async function act(action: InstanceAction) {
		if (pending || !data.instance || deleted) return;
		const instance = data.instance;
		if (action === 'force-shutdown' && !window.confirm(`Force-shutdown ${instance.hostname}? Power will be cut immediately and unsaved guest data may be lost.`)) return;
		if (action === 'delete' && !window.confirm(`Delete ${instance.hostname}? This permanently removes its disk, seed ISO, domain definition, and database record. Download any data you want to keep first. The instance must be shut down.`)) return;
		pending = action;
		error = '';
		notice = '';
		try {
			await performInstanceAction(instance.id, action);
			if (action === 'delete') {
				deleted = true;
				notice = 'Instance deleted.';
				await goto('/#instances');
			} else {
				statusRefresh += 1;
				notice = action === 'start' ? 'Start request accepted.'
					: action === 'shutdown' ? 'Shutdown requested. The guest may take a moment to stop.'
					: 'Force-shutdown request completed.';
			}
		} catch (cause) {
			error = deleted ? 'Instance deleted. Use All instances to return to the list.'
				: cause instanceof Error ? cause.message : 'The operation failed.';
		} finally { pending = null; }
	}
</script>

<svelte:head><title>{data.instance?.hostname ?? 'Instance'} · Andromeda</title></svelte:head>
<a class="back-link" href="/#instances">← All instances</a>
<section class="page-heading">
	<div><p class="eyebrow">INSTANCE DETAILS</p><h1>{data.instance?.hostname ?? 'Instance unavailable'}</h1></div>
	<button class="button secondary" onclick={refresh} disabled={pending !== null || deleted}>{pending === 'refresh' ? 'Refreshing…' : 'Refresh details'}</button>
</section>
{#if data.loadError}<div class="error-notice standalone" role="alert">{data.loadError}</div>{/if}
{#if error}<div class="error-notice standalone" role="alert">{error}</div>{/if}
{#if notice}<p class="success-notice" role="status">{notice}</p>{/if}
{#if data.instance && !deleted}
	<div class="instance-detail-layout">
		<div class="instance-main-column">
			<InstanceMetrics id={data.instance.id} refreshToken={statusRefresh} bind:status bind:loading={statusLoading} />
		</div>
		<aside class="instance-sidebar" aria-label="Instance information and controls">
			<section class="panel" aria-labelledby="controls-heading" aria-busy={pending !== null}>
				<div class="panel-header"><h2 id="controls-heading">Instance controls</h2></div>
				<div class="controls-body">
					<div class="button-row">
						{#each ['start', 'shutdown', 'force-shutdown'] as action}
							{@const kind = action as InstanceAction}
							{@const available = data.instance.permissions.control && pending === null && status !== null && status.state !== 'missing' && (kind === 'start' ? !status.active : status.active)}
							<button class="button power-control" class:primary={available} class:secondary={!available} disabled={!available} aria-busy={pending === kind} aria-describedby={kind === 'force-shutdown' ? 'force-shutdown-warning' : undefined} onclick={() => act(kind)}>{pending === kind ? 'Working…' : labels[kind]}</button>
						{/each}
						<p id="force-shutdown-warning" class="muted small">Only use force-shutdown if the instance becomes unresponsive. It may result in data loss.</p>
					</div>
				</div>
			</section>
			<section class="panel" aria-labelledby="configuration-heading">
				<div class="panel-header"><h2 id="configuration-heading">Instance information</h2></div>
				<dl class="details-grid">
					<div><dt>Your role</dt><dd>{data.instance.role}</dd></div>
				<div><dt>Power state</dt><dd><StatusBadge label={status ? powerStateLabel(status.state) : statusLoading ? 'Loading' : 'Unavailable'} tone={status ? powerStateTone(status.state) : 'neutral'} /></dd></div>
					<div class="full-width"><dt>Instance ID</dt><dd><code>{data.instance.id}</code></dd></div>
					<div><dt>Hostname</dt><dd>{data.instance.hostname}</dd></div>
					<div><dt>Description</dt><dd class="instance-description">{data.instance.description || '—'}</dd></div>
					<div><dt>Memory</dt><dd>{data.instance.memory_mib.toLocaleString()} MiB</dd></div>
					<div><dt>Virtual CPUs</dt><dd>{data.instance.vcpus}</dd></div>
					<div><dt>MAC address</dt><dd><code>{data.instance.mac_address}</code></dd></div>
					<div><dt>IPv4 address / prefix</dt><dd>{data.instance.ipv4_address || 'Not configured'}</dd></div>
					<div><dt>Remote port</dt><dd>{data.instance.remote_port}</dd></div>
					<div><dt>Service port</dt><dd>{data.instance.service_port}</dd></div>
				</dl>
			</section>
			<section class="panel" aria-labelledby="download-heading">
				<div class="panel-header"><h2 id="download-heading">Your instance data</h2></div>
				<div class="controls-body">
					<div class="button-row">
						{#if !data.instance.permissions.download || pending !== null || !status || status.active || status.state === 'missing'}
							<button class="button secondary" disabled>Download disk</button>
						{:else}
							<form method="POST" action={`/instances/${encodeURIComponent(data.instance.id)}/disk`} target="_blank" rel="noopener noreferrer">
							<input type="hidden" name="csrf_token" value={page.data.session.csrf_token ?? ''} />
							<button class="button secondary" type="submit">Download disk</button>
						</form>
						{/if}
					</div>
					<div class="instance-delete">
						<button class="button danger" disabled={!data.instance.permissions.delete || pending !== null || status?.active === true} onclick={() => act('delete')}>{pending === 'delete' ? 'Deleting…' : labels.delete}</button>
					</div>
				</div>
			</section>
		</aside>
	</div>
{/if}
