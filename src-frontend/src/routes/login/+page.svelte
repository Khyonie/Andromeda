<script lang="ts">
	import { page } from '$app/state';
	import type { PageData } from './$types';
	let { data }: { data: PageData } = $props();
</script>

<svelte:head><title>Sign in · Andromeda</title></svelte:head>
<section class="panel login-panel">
	<div class="controls-body">
		<p class="eyebrow">ANDROMEDA</p>
		<h1>Sign in to manage your instances.</h1>
		<p class="muted">Your first sign-in creates your account.</p>
		{#if page.url.searchParams.has('error')}<p class="error-notice standalone" role="alert">Discord sign-in was cancelled, expired, or could not be completed. Please try again.</p>{/if}
		{#if data.authError}<p class="error-notice standalone" role="alert">{data.authError}</p>{/if}
		{#if data.session.login_available}
			<a class="button primary" href="/api/auth/discord" data-sveltekit-reload>Continue with Discord</a>
		{:else}
			<p class="muted">
				Discord sign-in is currently unavailable. If you are setting up a new machine, follow the
				<a class="setup-link" href="https://docs.khyonieheart.coffee/andromeda/host-setup.md">setup guide</a>.
			</p>
		{/if}
		{#if data.session.user}<a class="back-link" href="/">Return to workspace</a>{/if}
	</div>
</section>

<style>
	.setup-link {
		color: var(--accent);
		text-decoration: underline;
		text-underline-offset: 3px;
	}
</style>
