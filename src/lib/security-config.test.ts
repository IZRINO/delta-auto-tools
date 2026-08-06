import {describe, expect, it} from "vitest";

import defaultCapabilityJson from "../../src-tauri/capabilities/default.json?raw";
import overlaysCapabilityJson from "../../src-tauri/capabilities/overlays.json?raw";
import specialOpsProfitCapabilityJson from "../../src-tauri/capabilities/special-ops-profit.json?raw";
import strategyCapabilityJson from "../../src-tauri/capabilities/strategy.json?raw";
import tauriConfigJson from "../../src-tauri/tauri.conf.json?raw";

type Capability = {
    identifier: string;
    local?: boolean;
    windows?: string[];
    webviews?: string[];
    remote?: {urls: string[]};
    permissions: Array<string | {identifier: string; allow?: unknown[]}>;
};

function readJson<T>(raw: string): T {
    return JSON.parse(raw) as T;
}

function permissionIdentifiers(capability: Capability): string[] {
    return capability.permissions.map((permission) =>
        typeof permission === "string" ? permission : permission.identifier,
    );
}

describe("Tauri security config", () => {
    it("main capability 只授权 main 且不使用 default permission set", () => {
        const capability = readJson<Capability>(defaultCapabilityJson);
        const permissions = permissionIdentifiers(capability);

        expect(capability.windows).toEqual(["main"]);
        expect(permissions.some((permission) => permission.endsWith(":default"))).toBe(false);
        expect(permissions).not.toContain("core:default");
        expect(permissions).not.toContain("updater:default");
        expect(permissions).not.toContain("core:window:allow-close");
        expect(permissions).toContain("core:window:allow-destroy");
    });

    it("未完整迁移全部 app commands 前禁止启用局部 app ACL", () => {
        const capability = readJson<Capability>(defaultCapabilityJson);
        const permissions = permissionIdentifiers(capability);

        expect(permissions.every((permission) => permission.includes(":"))).toBe(true);
    });

    it("overlay capability 独立存在且不授予高权限 plugin", () => {
        const capability = readJson<Capability>(overlaysCapabilityJson);
        const permissions = permissionIdentifiers(capability);

        expect(capability.windows?.some((window) => window.includes("overlay"))).toBe(true);
        expect(permissions.some((permission) => /^(dialog|opener|process|updater):/.test(permission))).toBe(false);
    });

    it("Strategy remote WebView capability 不授予 IPC permission", () => {
        const capability = readJson<Capability>(strategyCapabilityJson);

        expect(capability.local).toBe(false);
        expect(capability.webviews).toEqual(["strategy-content"]);
        expect(capability.permissions).toEqual([]);
    });

    it("Moligod 隐藏 WebView 只允许精确 origin 且无 IPC permission", () => {
        const capability = readJson<Capability>(specialOpsProfitCapabilityJson);

        expect(capability.local).toBe(false);
        expect(capability.remote?.urls).toEqual(["https://moligod.com/*"]);
        expect(capability.webviews).toEqual(["special-ops-profit-*"]);
        expect(capability.permissions).toEqual([]);
        expect(capability.identifier).not.toBe("strategy");
    });

    it("生产 CSP 禁止 unsafe-eval 与远程通配源", () => {
        const config = readJson<{app: {security: {csp: string | null}}}>(tauriConfigJson);
        const csp = config.app.security.csp;

        expect(csp).not.toBeNull();
        expect(csp).not.toContain("'unsafe-eval'");
        expect(csp).not.toMatch(/https?:\/\/\*/);
        expect(csp).toContain("connect-src ipc: http://ipc.localhost");
        expect(csp).toContain("img-src 'self' asset: http://asset.localhost data: blob:");
    });
});
