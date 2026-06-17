import { defineConfig } from 'astro/config';

export default defineConfig({
  site: 'https://forja-lang.github.io',
  base: '/',
  outDir: './dist',
  srcDir: './src',
  publicDir: './public',
});
