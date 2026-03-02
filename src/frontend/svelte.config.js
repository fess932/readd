import adapter from '@sveltejs/adapter-static';

/** @type {import('@sveltejs/kit').Config} */
const config = {
	kit: {
		adapter: adapter({
			pages: '../../dist',   // собранные HTML/JS/CSS → корень проекта/dist
			assets: '../../dist',
			fallback: 'index.html', // SPA-режим: все маршруты → index.html
		}),
	}
};

export default config;
