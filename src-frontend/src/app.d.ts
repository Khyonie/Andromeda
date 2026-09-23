// See https://svelte.dev/docs/kit/types#app.d.ts
// for information about these interfaces
declare global {
	namespace App {
		// interface Error {}
		// interface Locals {}
		interface PageData { session: import('$lib/api/auth').Session }
		// interface PageState {}
		// interface Platform {}
	}
}

export {};
