<script lang="ts">
	import { invalidateAll } from '$app/navigation';
	import { saveGuestDefaults } from '$lib/api/admin';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();
	let busy = $state(false);
	let error = $state('');
	let notice = $state('');
	let saved = $state<typeof data.settings | null>(null);
	const settings = $derived(saved ?? data.settings);
	const ipv4Pattern = String.raw`((25[0-5]|2[0-4][0-9]|1[0-9]{2}|[1-9]?[0-9])\.){3}(25[0-5]|2[0-4][0-9]|1[0-9]{2}|[1-9]?[0-9])`;

	async function save(event: SubmitEvent) {
		event.preventDefault();
		if (busy) return;
		const values = new FormData(event.currentTarget as HTMLFormElement);
		busy = true;
		error = '';
		notice = '';
		try {
			saved = await saveGuestDefaults({
				gateway_ip: String(values.get('gateway_ip') ?? '').trim(),
				sysadmin_ssh_key: String(values.get('sysadmin_ssh_key') ?? '').trim(),
				revision: settings.revision
			});
			notice = 'Saved. New instances will use these defaults.';
		} catch (cause) {
			error = cause instanceof Error ? cause.message : 'Could not save guest defaults.';
		} finally { busy = false; }
	}

	async function reload() {
		busy = true;
		error = '';
		notice = '';
		try { await invalidateAll(); saved = null; }
		catch { error = 'Could not reload settings. Please try again.'; }
		finally { busy = false; }
	}
</script>

<svelte:head><title>Administration · Andromeda</title></svelte:head>

<div class="page-heading">
	<div>
		<p class="eyebrow">ADMINISTRATION</p>
		<h1>Guest defaults</h1>
		<p class="muted">Changes apply to new instances immediately. Existing instances keep their current configuration.</p>
	</div>
</div>

{#if error}<p class="error-notice standalone" role="alert">{error}</p>{/if}
{#if notice}<p class="success-notice" role="status">{notice}</p>{/if}

{#key settings}
<form onsubmit={save}>
	<fieldset class="form-fields" disabled={busy}>
		<section class="panel form-panel" aria-labelledby="network-heading">
			<div class="panel-header"><h2 id="network-heading">Network</h2></div>
			<div class="form-grid">
				<label class="field">Default gateway
					<input name="gateway_ip" value={settings.gateway_ip} required pattern={ipv4Pattern} placeholder="10.0.0.1" aria-describedby="gateway-help" />
					<span id="gateway-help" class="muted small">The IPv4 router address reachable from your guests.</span>
				</label>
			</div>
		</section>
		<section class="panel form-panel" aria-labelledby="access-heading">
			<div class="panel-header"><h2 id="access-heading">Sysadmin access</h2></div>
			<div class="form-grid">
				<label class="field full-width">SSH public key
					<textarea name="sysadmin_ssh_key" value={settings.sysadmin_ssh_key} rows="4" maxlength="16384" spellcheck="false" placeholder="ssh-ed25519 AAAA…" aria-describedby="key-help"></textarea>
					<span id="key-help" class="muted small">Grants SSH access as <code>sysadmin</code> with passwordless sudo on new instances. Paste one public key, or leave empty to disable this account.</span>
				</label>
			</div>
		</section>
		<div class="button-row">
			<button class="button primary" type="submit">{busy ? 'Saving…' : 'Save defaults'}</button>
			<button class="button secondary" type="button" onclick={reload}>Reload saved settings</button>
		</div>
	</fieldset>
</form>
{/key}
