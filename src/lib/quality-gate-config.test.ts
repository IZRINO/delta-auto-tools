import {describe, expect, it} from "vitest";

import ciWorkflow from "../../.github/workflows/ci.yml?raw";
import packageJson from "../../package.json";
import checkScript from "../../scripts/check.ps1?raw";
import viteConfig from "../../vite.config.ts?raw";

describe("Windows 质量门禁配置", () => {
    it("统一 check 命令按依赖顺序执行全部质量步骤", () => {
        expect(packageJson.scripts.check).toBe(
            "powershell -NoProfile -ExecutionPolicy Bypass -File scripts/check.ps1",
        );

        const commands = [
            "bun node_modules/typescript/bin/tsc --noEmit --pretty false",
            "node node_modules/vitest/vitest.mjs run --reporter=dot",
            "node node_modules/vitest/vitest.mjs run --coverage --reporter=dot",
            "cargo fmt --manifest-path src-tauri/Cargo.toml -- --check",
            "cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings",
            "cargo test --manifest-path src-tauri/Cargo.toml",
        ];
        const positions = commands.map((command) => checkScript.indexOf(command));

        expect(positions.every((position) => position >= 0)).toBe(true);
        expect(positions).toEqual([...positions].sort((left, right) => left - right));
    });

    it("coverage 覆盖全部前端源文件并守住当前基线", () => {
        expect(viteConfig).toContain('include: ["src/**/*.{ts,tsx}"]');
        expect(viteConfig).toContain("lines: 25.49");
        expect(viteConfig).toContain("statements: 25.67");
        expect(viteConfig).toContain("functions: 22.31");
        expect(viteConfig).toContain("branches: 25.76");
        expect(viteConfig).toContain('"src/hooks/autosave-queue.ts"');
        expect(viteConfig).toContain('"src/lib/tauri-listener.ts"');
        expect(viteConfig).toContain('"src/components/app/recognition-card-reducer.ts"');
    });

    it("Windows CI 复用本地 check 命令", () => {
        expect(ciWorkflow).toContain("windows-latest");
        expect(ciWorkflow).toContain("actions/setup-node@v4");
        expect(ciWorkflow).toContain("node-version: 24");
        expect(ciWorkflow).toContain("bun install --frozen-lockfile");
        expect(ciWorkflow).toContain("bun run check");
    });
});
