import { defineConfig } from '@playwright/test';
export default defineConfig({
  testDir: '.',
  testMatch: 'site.spec.mjs',
  workers: 1,
  use: { baseURL: 'http://127.0.0.1:3035/how-far/' },
  webServer: {
    command: 'python3 -m http.server 3035 --bind 0.0.0.0 --directory ../target/site-preview',
    url: 'http://127.0.0.1:3035/how-far/',
    reuseExistingServer: false,
  },
});
