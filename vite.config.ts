import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import tailwindcss from "@tailwindcss/vite";

// The Tauri CLI waits on this fixed port (devUrl in tauri.conf.json).
export default defineConfig({
  plugins: [tailwindcss(), svelte()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
});
