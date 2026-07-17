import {configDefaults, defineConfig} from "vitest/config";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import {fileURLToPath, URL} from "node:url";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
    plugins: [react(), tailwindcss()],
    resolve: {
        alias: {
            "@": fileURLToPath(new URL("./src", import.meta.url)),
        },
    },
    test: {
        exclude: [...configDefaults.exclude, "**/.omp/**", "**/.worktrees/**", "**/worktrees/**"],
        globals: true,
        environment: "node",
        coverage: {
            provider: "v8",
            reporter: ["text", "json-summary"],
            include: ["src/**/*.{ts,tsx}"],
            thresholds: {
                lines: 25.49,
                statements: 25.67,
                functions: 22.31,
                branches: 25.76,
                "src/hooks/autosave-queue.ts": {lines: 90},
                "src/lib/tauri-listener.ts": {lines: 90},
                "src/components/app/recognition-card-reducer.ts": {lines: 90},
            },
        },
    },
    clearScreen: false,
    server: {
        port: 1420,
        strictPort: true,
        host: host || false,
        hmr: host
            ? {
                protocol: "ws",
                host,
                port: 1421,
            }
            : undefined,
        watch: {
            ignored: ["**/src-tauri/**"],
        },
    },
});
