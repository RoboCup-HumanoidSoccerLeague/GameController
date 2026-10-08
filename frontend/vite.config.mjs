import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  // Tauri expects the dev server at the URL in tauri.conf.json. The address is given explicitly
  // because "localhost" may resolve to IPv4 or IPv6 depending on the system.
  server: { host: "127.0.0.1", port: 3000, strictPort: true },
  build: { outDir: "build", emptyOutDir: true },
});
