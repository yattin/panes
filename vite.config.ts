import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  define: {
    __NON_NATIVE_HARNESSES__: JSON.stringify(
      process.env.VITE_NON_NATIVE_HARNESSES === "1"
    ),
  },
  build: {
    minify: false,
  },
  server: {
    host: "127.0.0.1",
    port: 1420,
    strictPort: true,
    hmr: {
      port: 1421
    }
  },
  clearScreen: false
});
