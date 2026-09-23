<script lang="ts">
	import { onMount } from 'svelte';
	import { listInstances, type InstanceSummary } from '$lib/api/instances';
	import StatusBadge from '$lib/components/StatusBadge.svelte';
	import InstanceTable from '$lib/components/InstanceTable.svelte';

	let instances = $state<InstanceSummary[] | null>(null);
	let loading = $state(false);
	let error = $state('');
	let lastUpdated = $state('');
	let filter = $state('');
	let controller: AbortController | undefined;
	const visibleInstances = $derived((instances ?? []).filter((instance) =>
		[instance.hostname, instance.description, String(instance.remote_port), String(instance.service_port)]
			.some((value) => value.toLowerCase().includes(filter.trim().toLowerCase()))));

	onMount(() => {
		const poll = () => { if (!document.hidden) void refresh(); };
		poll();
		const timer = setInterval(poll, 10_000);
		document.addEventListener('visibilitychange', poll);
		return () => { clearInterval(timer); controller?.abort(); document.removeEventListener('visibilitychange', poll); };
	});

	async function refresh() {
		if (loading) return;
		const request = new AbortController();
		controller = request;
		loading = true;
		try {
			const next = await listInstances(request.signal);
			if (request.signal.aborted) return;
			instances = next;
			error = '';
			lastUpdated = new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
		} catch (cause) {
			if (request.signal.aborted) return;
			instances = instances?.map((instance) => ({ ...instance, state: 'unavailable' })) ?? null;
			error = cause instanceof TypeError || (cause instanceof DOMException && cause.name === 'TimeoutError')
				? 'Could not reach the management server. Check that it is running, then try again.'
				: cause instanceof Error ? cause.message : 'Could not load instances. Please try again.';
		} finally {
			if (!request.signal.aborted) loading = false;
		}
	}
</script>

<svelte:head>
	<title>Overview · Andromeda</title>
	<meta name="description" content="Your Andromeda workspace. View and organize your virtual machine instances." />
</svelte:head>

<section class="page-heading">
	<div><p class="eyebrow">YOUR WORKSPACE</p><h1>Overview</h1><p class="muted">A little perspective on your infrastructure.</p></div>
	<div class="button-row"><a class="button primary" href="/instances/new">Create instance</a>
	<button class="button secondary" onclick={refresh} disabled={loading}>
		<svg viewBox="0 0 24 24" fill="none" aria-hidden="true"><path d="M20 7v5h-5M4 17v-5h5M6.1 7a7 7 0 0 1 11.6-1L20 9M4 15l2.3 3A7 7 0 0 0 17.9 17" /></svg>
		{loading ? 'Loading…' : instances === null ? 'Load instances' : 'Refresh instances'}
	</button></div>
</section>

<div class="summary-grid">
	<section class="summary-card"><p class="card-label">Registered instances</p><p class="metric">{instances === null ? '—' : instances.length}</p><p class="muted small">{instances === null ? 'Load your workspace to get started' : 'Instances in your workspace'}</p></section>
	<section class="summary-card"><p class="card-label">Management server</p><div class="connection-status"><StatusBadge label={error ? 'Unavailable' : instances !== null ? 'Connected' : loading ? 'Connecting' : 'Not checked'} tone={error ? 'warning' : instances !== null ? 'success' : 'neutral'} /></div><p class="muted small">{lastUpdated ? `Last refreshed at ${lastUpdated}` : 'Connection checked when you load instances'}</p></section>
</div>

<section class="panel" id="instances" aria-labelledby="instances-heading" aria-busy={loading}>
	<div class="panel-header"><div><h2 id="instances-heading">Instance Workspace</h2><p class="muted small">Accurate within 10 seconds.</p></div><span class="panel-kicker">WORKSPACE INVENTORY</span></div>
	{#if error}<div class="error-notice" role="alert">{error}{#if instances !== null} Showing the last loaded list; current statuses are unavailable.{/if}</div>{/if}
	{#if instances === null || instances.length === 0}
		<div class="empty-state" aria-live="polite">
			<div class="empty-icon" aria-hidden="true"><svg viewBox="0 0 48 48" fill="none"><rect x="10" y="9" width="28" height="12" rx="3" /><rect x="10" y="27" width="28" height="12" rx="3" /><path d="M16 15h.01M16 33h.01M28 15h4M28 33h4" /></svg></div>
			<h3>{loading ? 'Loading your workspace…' : instances === null ? 'Ready when you are.' : 'A fresh start.'}</h3>
			<p>{instances === null ? 'Load your instances to see what’s in your workspace.' : 'There are no registered instances in this workspace yet.'}</p>
		</div>
	{:else}
		<div class="list-toolbar"><label for="instance-search">Filter instances</label><input id="instance-search" type="search" bind:value={filter} placeholder="Search names, descriptions, or ports…" /></div>
		{#if visibleInstances.length === 0}
			<p class="no-results">No instances match “{filter}”.</p>
		{:else}
			<InstanceTable instances={visibleInstances} />
		{/if}
	{/if}
</section>
