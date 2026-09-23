<script lang="ts">
	import type { InstanceSummary } from '$lib/api/instances';
	import { powerStateLabel, powerStateTone } from '$lib/instance-state';
	import StatusBadge from './StatusBadge.svelte';
	let { instances }: { instances: InstanceSummary[] } = $props();
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex (Focus lets keyboard users scroll the table horizontally.) -->
<div class="instance-table-scroll" role="region" aria-label="Instance list" tabindex="0">
	<table class="instance-table" aria-label="Instances">
		<thead><tr><th scope="col">Name</th><th scope="col">Status</th><th scope="col">Remote Port</th><th scope="col">Service Port</th><th scope="col">Description</th></tr></thead>
		<tbody>
			{#each instances as instance (instance.id)}
				<tr>
					<th scope="row"><a class="instance-link" href={`/instances/${encodeURIComponent(instance.id)}`}>{instance.hostname}</a></th>
					<td><StatusBadge label={powerStateLabel(instance.state)} tone={powerStateTone(instance.state)} /></td>
					<td class="instance-port">{instance.remote_port}</td>
					<td class="instance-port">{instance.service_port}</td>
					<td class="instance-description">{instance.description || '—'}</td>
				</tr>
			{/each}
		</tbody>
	</table>
</div>
