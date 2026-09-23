<script lang="ts">
	let { label, percent }: { label: string; percent: number | null | undefined } = $props();
	const value = $derived(percent != null && Number.isFinite(percent) ? Math.min(100, Math.max(0, percent)) : null);
</script>

<div class="usage-bar">
	{#if value === null}
		<div class="usage-track unavailable" aria-hidden="true"></div>
		<span class="muted small">No reading</span>
	{:else}
		<meter min="0" max="100" value={value} aria-label={label}>{value.toFixed(1)}%</meter>
		<span class="usage-percent">{value.toFixed(1)}%</span>
	{/if}
</div>
