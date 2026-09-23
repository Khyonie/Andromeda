import adapter from '@sveltejs/adapter-auto';
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig, loadEnv } from 'vite';

export default defineConfig(({ mode }) => {
	const env = loadEnv(mode, '.', 'ANDROMEDA_');
	const proxy = {
		'/api': {
			target: env.ANDROMEDA_API_URL || 'http://127.0.0.1:9966',
			changeOrigin: true
		}
	};

	return {
		server: { proxy },
		preview: { proxy },
		plugins: [
			sveltekit({
				compilerOptions: {
					// Use Svelte 5 runes for application components, preserving library defaults.
					runes: ({ filename }) =>
						filename.split(/[/\\]/).includes('node_modules') ? undefined : true
				},
				// Select a deployment-specific adapter when choosing how to host the frontend.
				adapter: adapter()
			})
		]
	};
});
