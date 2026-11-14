import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

// O Tauri espera o servidor de desenvolvimento sempre na mesma porta (ver src-tauri/tauri.conf.json).
// 1430 e não a 1420 padrão, que já é do Assets Studio Maker: assim os dois rodam ao mesmo tempo.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  server: {
    port: 1430,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**", "**/crates/**", "**/target/**"] },
  },
});
