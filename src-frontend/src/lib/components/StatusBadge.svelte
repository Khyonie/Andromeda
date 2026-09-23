<script lang="ts">
	let { label, tone = 'neutral' }: {
		label: string;
		tone?: 'neutral' | 'success' | 'warning';
	} = $props();
</script>

<span class="status-badge" class:success={tone === 'success'} class:warning={tone === 'warning'}>
	<span class="status-dot" aria-hidden="true"></span>
	{label}
</span>
