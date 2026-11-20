import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

// base './' so the page works from any Pages path (user.github.io/PrivCord/ or a custom domain).
export default defineConfig({ base: './', plugins: [react()], build: { target: 'es2020', chunkSizeWarningLimit: 900 } });
