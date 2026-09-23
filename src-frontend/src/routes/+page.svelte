<script lang="ts">
	import { onMount } from 'svelte';
	import { listInstanceIds } from '$lib/api/instances';
	import StatusBadge from '$lib/components/StatusBadge.svelte';

	let ids = $state<string[] | null>(null);
	let loading = $state(false);
	let error = $state('');
	let lastUpdated = $state('');
	let filter = $state('');
	const visibleIds = $derived((ids ?? []).filter((id) => id.toLowerCase().includes(filter.toLowerCase())));

	onMount(() => { void refresh(); });

	async function refresh() {
		loading = true;
		error = '';
		try {
			ids = await listInstanceIds();
			lastUpdated = new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
		} catch (cause) {
			error = cause instanceof TypeError || (cause instanceof DOMException && cause.name === 'TimeoutError')
				? 'Could not reach the management server. Check that it is running, then try again.'
				: cause instanceof Error ? cause.message : 'Could not load instances. Please try again.';
		} finally {
			loading = false;
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
		{loading ? 'Loading…' : ids === null ? 'Load instances' : 'Refresh instances'}
	</button></div>
</section>

<div class="summary-grid">
	<section class="summary-card"><p class="card-label">Registered instances</p><p class="metric">{ids === null ? '—' : ids.length}</p><p class="muted small">{ids === null ? 'Load your workspace to get started' : 'Instances in your workspace'}</p></section>
	<section class="summary-card"><p class="card-label">Management server</p><div class="connection-status"><StatusBadge label={loading ? 'Connecting' : error ? 'Unavailable' : ids === null ? 'Not checked' : 'Connected'} tone={error ? 'warning' : ids !== null && !loading ? 'success' : 'neutral'} /></div><p class="muted small">{lastUpdated ? `Last refreshed at ${lastUpdated}` : 'Connection checked when you load instances'}</p></section>
</div>

<section class="panel" id="instances" aria-labelledby="instances-heading" aria-busy={loading}>
	<div class="panel-header"><div><h2 id="instances-heading">Instances</h2><p class="muted small">Your machines, all in one place.</p></div><span class="panel-kicker">WORKSPACE INVENTORY</span></div>
	{#if error}<div class="error-notice" role="alert">{error}{#if ids !== null} Showing the last loaded list.{/if}</div>{/if}
	{#if ids === null || ids.length === 0}
		<div class="empty-state" aria-live="polite">
			<div class="empty-icon" aria-hidden="true"><svg viewBox="0 0 48 48" fill="none"><rect x="10" y="9" width="28" height="12" rx="3" /><rect x="10" y="27" width="28" height="12" rx="3" /><path d="M16 15h.01M16 33h.01M28 15h4M28 33h4" /></svg></div>
			<h3>{loading ? 'Loading your workspace…' : ids === null ? 'Ready when you are.' : 'A fresh start.'}</h3>
			<p>{ids === null ? 'Load your instances to see what’s in your workspace.' : 'There are no registered instances in this workspace yet.'}</p>
		</div>
	{:else}
		<div class="list-toolbar"><label for="instance-search">Filter by ID</label><input id="instance-search" type="search" bind:value={filter} placeholder="Search instance IDs…" /></div>
		{#if visibleIds.length === 0}
			<p class="no-results">No instance IDs match “{filter}”.</p>
		{:else}
			<ul class="instance-list" aria-label="Instance IDs">
				{#each visibleIds as id (id)}<li><span class="instance-marker" aria-hidden="true"></span><a class="instance-link" href={`/instances/${encodeURIComponent(id)}`}><code>{id}</code></a><span class="record-label">Registered</span></li>{/each}
			</ul>
		{/if}
	{/if}
</section>
