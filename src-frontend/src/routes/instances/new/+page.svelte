<script lang="ts">
	import { goto } from '$app/navigation';
	import { createInstance, type VmConfig } from '$lib/api/instances';

	// Keep regex quantifiers out of markup, where Svelte treats braces as expressions.
	const ipv4Pattern = String.raw`((25[0-5]|2[0-4][0-9]|1[0-9]{2}|[1-9]?[0-9])\.){3}(25[0-5]|2[0-4][0-9]|1[0-9]{2}|[1-9]?[0-9])/(3[0-2]|[12]?[0-9])`;
	const macPattern = String.raw`([0-9a-fA-F]{2}:){5}[0-9a-fA-F]{2}`;

	let busy = $state(false);
	let error = $state('');
	let notice = $state('');
	let createdId = $state('');

	async function create(event: SubmitEvent) {
		event.preventDefault();
		if (busy || createdId) return;
		const form = event.currentTarget as HTMLFormElement;
		const values = new FormData(form);
		const text = (key: string) => String(values.get(key) ?? '').trim();
		const number = (key: string) => Number(text(key));
		const config: VmConfig = {
			user: { name: text('username'), key: text('key') },
			networking: { ip: text('ip'), dhcp: values.has('dhcp'), mac: text('mac'), remote_port: number('remote_port'), service_port: number('service_port') },
			instance: { hostname: text('hostname'), description: text('description'), 'disk-size': number('disk'), memory: number('memory'), vcpus: number('vcpus') }
		};
		busy = true;
		error = '';
		notice = '';
		try {
			const id = await createInstance(config);
			if (id === null) {
				notice = 'Dry run completed. The server did not create or save an instance.';
			} else {
				createdId = id;
				notice = 'Instance created.';
				await goto(`/instances/${encodeURIComponent(id)}`);
			}
		} catch (cause) {
			error = createdId ? 'Instance created, but the page could not be opened. Use the link below.'
				: cause instanceof Error ? cause.message : 'Could not create the instance.';
		} finally { busy = false; }
	}
</script>

<svelte:head><title>Create instance · Andromeda</title></svelte:head>
<a class="back-link" href="/#instances">← All instances</a>
<section class="page-heading">
	<div><p class="eyebrow">NEW INSTANCE</p><h1>Create instance</h1><p class="muted">Configure your machine, networking, and SSH access.</p></div>
</section>
{#if error}<div class="error-notice standalone" role="alert">{error}</div>{/if}
{#if notice}<p class="success-notice" role="status">{notice}</p>{/if}
{#if createdId}<a class="button primary" href={`/instances/${encodeURIComponent(createdId)}`}>Open instance</a>{/if}
<form onsubmit={create} aria-busy={busy}>
	<fieldset disabled={busy || !!createdId} class="form-fields">
		<section class="panel form-panel">
			<div class="panel-header"><div><h2>Machine</h2><p class="muted small">Disk and memory sizes are in MiB.</p></div></div>
			<div class="form-grid">
				<label class="field full-width">Hostname<input name="hostname" required maxlength="63" pattern="[a-zA-Z0-9]([a-zA-Z0-9\-]*[a-zA-Z0-9])?" placeholder="my-instance" title="Letters, numbers, and hyphens; begin and end with a letter or number." /></label>
				<label class="field full-width">Description (optional)<textarea name="description" rows="3" maxlength="2000" placeholder="What is this instance for?"></textarea></label>
				<label class="field">Memory (MiB)<input name="memory" type="number" min="1" max="4294967295" step="1" value="2048" required /></label>
				<label class="field">Virtual CPUs<input name="vcpus" type="number" min="1" max="4294967295" step="1" value="2" required /></label>
				<label class="field">Disk size (MiB)<input name="disk" type="number" min="1" max="9007199254740991" step="1" value="8192" required /></label>
			</div>
		</section>
		<section class="panel form-panel">
			<div class="panel-header"><div><h2>Networking</h2><p class="muted small">Choose an unused address on your instance network.</p></div></div>
			<div class="form-grid">
				<label class="field">IPv4 address / prefix<input name="ip" required placeholder="10.0.0.200/24" pattern={ipv4Pattern} title="An IPv4 address with a prefix from 0 to 32, such as 10.0.0.200/24." /></label>
				<label class="field">MAC address<input name="mac" required placeholder="02:ad:cc:dd:ee:ff" pattern={macPattern} title="Six hexadecimal pairs separated by colons." /></label>
				<label class="field">Remote port<input name="remote_port" type="number" min="1" max="65535" value="22" required /></label>
				<label class="field">Service port<input name="service_port" type="number" min="1" max="65535" value="25565" required /></label>
				<label class="checkbox-field full-width"><input name="dhcp" type="checkbox" /> Enable DHCP alongside the configured address</label>
			</div>
		</section>
		<section class="panel form-panel">
			<div class="panel-header"><div><h2>SSH access</h2><p class="muted small">Create the guest account with your public key.</p></div></div>
			<div class="form-grid">
				<label class="field full-width">Username<input name="username" required maxlength="32" pattern="[a-z_][a-z0-9_\-]*" placeholder="andromeda" title="Lowercase letters, numbers, underscores, and hyphens; begin with a letter or underscore." /></label>
				<label class="field full-width">SSH public key<textarea name="key" rows="4" required placeholder="ssh-ed25519 AAAA…" spellcheck="false"></textarea></label>
			</div>
		</section>
		<div class="button-row"><button type="submit" class="button primary">{busy ? 'Creating instance…' : 'Create instance'}</button><a class="button secondary" href="/#instances">Cancel</a></div>
	</fieldset>
</form>
