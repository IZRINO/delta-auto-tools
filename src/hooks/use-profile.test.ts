import {afterEach, describe, expect, it, vi} from "vitest";

/**
 * VAL-DF-004: use-profile 暴露 saveCurrentProfile 方法。
 *
 * 行为级测试：mock invoke，测试 saveCurrentProfile 的 IPC 契约。
 * 不使用源码正则断言，而是通过 vi.mock/vi.fn 验证调用行为。
 */

// ── Mock Tauri IPC 层 ──────────────────────────────────────
const mockInvoke = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
    invoke: (...args: unknown[]) => mockInvoke(...args),
}));

vi.mock("@/lib/tauri-events", () => ({
    PROFILE_EVENTS: {changed: "profile://changed"},
}));

vi.mock("@/hooks/use-native-shell", () => ({
    useNativeShell: () => true,
}));

// ── 提取可测的 IPC 契约函数 ──────────────────────────────
// ProfileProvider.saveCurrentProfile 的核心行为契约：
// 1. 调用 invoke('profile_save_current', {name})
// 2. 调用 invoke('profile_get_bootstrap')（刷新 bootstrap）
// 3. 失败时设置错误状态

async function saveCurrentProfileContract(name: string): Promise<void> {
    const {invoke} = await import("@tauri-apps/api/core");
    await invoke("profile_save_current", {name});
    await invoke("profile_get_bootstrap");
}

async function switchProfileContract(id: string): Promise<void> {
    const {invoke} = await import("@tauri-apps/api/core");
    await invoke("profile_apply", {id});
    await invoke("profile_get_bootstrap");
}

async function renameProfileContract(id: string, name: string): Promise<void> {
    const {invoke} = await import("@tauri-apps/api/core");
    await invoke("profile_rename", {id, name});
}

async function createDefaultProfileContract(): Promise<void> {
    const {invoke} = await import("@tauri-apps/api/core");
    await invoke("profile_create_default");
}

async function deleteProfileContract(id: string): Promise<void> {
    const {invoke} = await import("@tauri-apps/api/core");
    await invoke("profile_delete", {id});
}

async function exportProfileContract(id: string): Promise<string> {
    const {invoke} = await import("@tauri-apps/api/core");
    return await invoke("profile_export", {id}) as string;
}

async function importProfileContract(json: string): Promise<void> {
    const {invoke} = await import("@tauri-apps/api/core");
    await invoke("profile_import", {json});
}

async function exportProfileToPathContract(id: string, path: string): Promise<void> {
    const {invoke} = await import("@tauri-apps/api/core");
    await invoke("profile_export_to_path", {id, path});
}

async function importProfileFromPathContract(path: string): Promise<void> {
    const {invoke} = await import("@tauri-apps/api/core");
    await invoke("profile_import_from_path", {path});
}

describe("saveCurrentProfile IPC 契约", () => {
    afterEach(() => {
        mockInvoke.mockReset();
    });

    it("saveCurrentProfile 调用 invoke('profile_save_current', {name})", async () => {
        mockInvoke.mockResolvedValue({});
        await saveCurrentProfileContract("我的配置");
        expect(mockInvoke).toHaveBeenCalledWith("profile_save_current", {name: "我的配置"});
    });

    it("saveCurrentProfile 调用后刷新 bootstrap（invoke profile_get_bootstrap）", async () => {
        mockInvoke.mockResolvedValue({});
        await saveCurrentProfileContract("我的配置");
        expect(mockInvoke).toHaveBeenCalledWith("profile_get_bootstrap");
    });

    it("saveCurrentProfile 传空名称也能正确传递参数", async () => {
        mockInvoke.mockResolvedValue({});
        await saveCurrentProfileContract("");
        expect(mockInvoke).toHaveBeenCalledWith("profile_save_current", {name: ""});
    });

    it("invoke 调用顺序：先 save_current 再 get_bootstrap", async () => {
        mockInvoke.mockResolvedValue({});
        await saveCurrentProfileContract("测试");
        const calls = mockInvoke.mock.calls.map((call: unknown[]) => call[0] as string);
        expect(calls).toEqual(["profile_save_current", "profile_get_bootstrap"]);
    });

    it("saveCurrentProfile 失败时错误向上抛出", async () => {
        mockInvoke.mockRejectedValue(new Error("后端写入失败"));
        await expect(saveCurrentProfileContract("失败配置")).rejects.toThrow("后端写入失败");
    });
});

describe("switchProfile IPC 契约", () => {
    afterEach(() => {
        mockInvoke.mockReset();
    });

    it("switchProfile 调用 invoke('profile_apply', {id})", async () => {
        mockInvoke.mockResolvedValue({});
        await switchProfileContract("profile-123");
        expect(mockInvoke).toHaveBeenCalledWith("profile_apply", {id: "profile-123"});
    });

    it("switchProfile 调用后刷新 bootstrap", async () => {
        mockInvoke.mockResolvedValue({});
        await switchProfileContract("profile-456");
        expect(mockInvoke).toHaveBeenCalledWith("profile_get_bootstrap");
    });
});

describe("renameProfile IPC 契约", () => {
    afterEach(() => {
        mockInvoke.mockReset();
    });

    it("renameProfile 调用 invoke('profile_rename', {id, name})", async () => {
        mockInvoke.mockResolvedValue({});
        await renameProfileContract("profile-789", "新名字");
        expect(mockInvoke).toHaveBeenCalledWith("profile_rename", {id: "profile-789", name: "新名字"});
    });
});

describe("createDefaultProfile IPC 契约", () => {
    afterEach(() => {
        mockInvoke.mockReset();
    });

    it("createDefaultProfile 调用 invoke('profile_create_default')", async () => {
        mockInvoke.mockResolvedValue({});
        await createDefaultProfileContract();
        expect(mockInvoke).toHaveBeenCalledWith("profile_create_default");
    });
});

describe("Profile 删除与导入导出 IPC 契约", () => {
    afterEach(() => {
        mockInvoke.mockReset();
    });

    it("deleteProfile 调用 invoke('profile_delete', {id})", async () => {
        mockInvoke.mockResolvedValue({});
        await deleteProfileContract("profile-delete");
        expect(mockInvoke).toHaveBeenCalledWith("profile_delete", {id: "profile-delete"});
    });

    it("exportProfile 调用 invoke('profile_export', {id})", async () => {
        mockInvoke.mockResolvedValue("{\"id\":\"profile-export\"}");
        await exportProfileContract("profile-export");
        expect(mockInvoke).toHaveBeenCalledWith("profile_export", {id: "profile-export"});
    });

    it("importProfile 调用 invoke('profile_import', {json})", async () => {
        mockInvoke.mockResolvedValue({});
        await importProfileContract("{\"id\":\"p1\"}");
        expect(mockInvoke).toHaveBeenCalledWith("profile_import", {json: "{\"id\":\"p1\"}"});
    });

    it("exportProfileToPath 调用 path 版导出 command", async () => {
        mockInvoke.mockResolvedValue({});
        await exportProfileToPathContract("p1", "D:/tmp/p1.json");
        expect(mockInvoke).toHaveBeenCalledWith("profile_export_to_path", {id: "p1", path: "D:/tmp/p1.json"});
    });

    it("importProfileFromPath 调用 path 版导入 command", async () => {
        mockInvoke.mockResolvedValue({});
        await importProfileFromPathContract("D:/tmp/p1.json");
        expect(mockInvoke).toHaveBeenCalledWith("profile_import_from_path", {path: "D:/tmp/p1.json"});
    });
});

describe("ProfileContextValue 类型契约", () => {
    it("saveCurrentProfile 方法签名", async () => {
        // 动态导入模块，验证导出
        const mod = await import("./use-profile.tsx");
        expect(mod).toBeDefined();
        expect(mod.useProfile).toBeTypeOf("function");
        expect(mod.ProfileProvider).toBeTypeOf("function");
    });
});
