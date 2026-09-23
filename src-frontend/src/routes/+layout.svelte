<script lang="ts">
	import '../app.css';
	import { page } from '$app/state';
	import favicon from '$lib/assets/favicon.svg';
	import { logout } from '$lib/api/auth';
	import type { LayoutData } from './$types';
	import type { Snippet } from 'svelte';

	let { children, data }: { children: Snippet; data: LayoutData } = $props();
	let signingOut = $state(false);
	let logoutError = $state('');
	async function signOut() {
		signingOut = true;
		logoutError = '';
		try { await logout(); }
		catch (cause) { logoutError = cause instanceof Error ? cause.message : 'Could not sign out.'; signingOut = false; }
	}
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
	<meta name="theme-color" content="#111017" />
</svelte:head>

<a class="skip-link" href="#main">Skip to content</a>
{#if data.session.user}
<div class="app-shell">
	<aside class="sidebar">
		<a class="brand" href="/" aria-label="Andromeda home">
			<img src={favicon} alt="" width="40" height="40" />
			<span>Andromeda<span class="brand-caption">INSTANCE MANAGER</span></span>
		</a>

		<nav aria-label="Main navigation">
			<p class="nav-heading">WORKSPACE</p>
			<a class="nav-link" class:active={page.url.pathname === "/"} href="/" aria-current={page.url.pathname === "/" ? "page" : undefined}>
				<svg viewBox="0 0 24 24" fill="none" aria-hidden="true"><rect x="3" y="3" width="7" height="7" rx="1.5" /><rect x="14" y="3" width="7" height="7" rx="1.5" /><rect x="3" y="14" width="7" height="7" rx="1.5" /><rect x="14" y="14" width="7" height="7" rx="1.5" /></svg>
				Overview
			</a>
			<a class="nav-link" class:active={page.url.pathname.startsWith("/instances")} href="/#instances">
				<svg viewBox="0 0 24 24" fill="none" aria-hidden="true"><rect x="3" y="3" width="18" height="7" rx="2" /><rect x="3" y="14" width="18" height="7" rx="2" /><path d="M7 6.5h.01M7 17.5h.01M15 6.5h3M15 17.5h3" /></svg>
				Instances
			</a>
			{#if data.session.is_admin}
				<a class="nav-link" class:active={page.url.pathname === '/admin'} href="/admin" aria-current={page.url.pathname === '/admin' ? 'page' : undefined}>
					<svg viewBox="0 0 24 24" fill="none" aria-hidden="true"><path d="M12 3 4 6v6c0 5 8 9 8 9s8-4 8-9V6l-8-3Z" /><path d="m8 12 3 3 5-6" /></svg>
					Administration
				</a>
			{/if}
		</nav>

		<div class="sidebar-footer"><span class="workspace-mark">A</span><div>{data.session.user.display_name}<small>@{data.session.user.username}</small></div></div>
	</aside>

	<div class="workspace">
		<header class="topbar"><span>Workspace <span class="breadcrumb-separator">/</span> <strong>{page.url.pathname === '/admin' ? 'Administration' : page.url.pathname === "/instances/new" ? "Create instance" : page.url.pathname.startsWith("/instances/") ? "Instance details" : "Overview"}</strong></span><button class="button secondary" onclick={signOut} disabled={signingOut}>{signingOut ? 'Signing out…' : 'Sign out'}</button></header>
		<main id="main" tabindex="-1">{#if logoutError}<p class="error-notice standalone" role="alert">{logoutError}</p>{/if}{@render children()}</main>
		<footer class="page-footer"><span>Andromeda</span><span>A home for your virtual machines.</span></footer>
	</div>
</div>

{:else}
	<main id="main" tabindex="-1" class="login-shell">{@render children()}</main>
{/if}
