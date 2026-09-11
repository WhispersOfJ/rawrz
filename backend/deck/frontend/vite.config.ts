import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  server: {
    proxy: {
      "/api": { target: "http://localhost:7780", changeOrigin: true },
      "/api/v1/ws": { target: "ws://localhost:7780", ws: true },
    },
  },
  build: { outDir: "dist", sourcemap: true },
});
