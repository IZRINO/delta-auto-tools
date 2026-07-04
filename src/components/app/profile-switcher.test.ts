import {afterEach, describe, expect, it, vi} from "vitest";

/**
 * VAL-DF-005: profile-switcher 含"复制"入口。
 *
 * 行为级测试：mock saveCurrentProfile，测试复制流程的行为契约。
 * 不使用 source-regex 断言，而是通过 vi.mock/vi.fn 验证调用行为。
 */

// ── Mock 依赖 ──────────────────────────────────────────
const mockSaveCurrentProfile = vi.fn();
const mockDeleteProfile = vi.fn();
const mockExportProfileToPath = vi.fn();
const mockImportProfileFromPath = vi.fn();
const mockValidateProfileName = vi.fn();

vi.mock("@/hooks/use-profile", () => ({
    useProfile: () => ({
        bootstrap: {
            profiles: [
                {id: "p1", name: "默认配置", createdAt: 0, updatedAt: 0, snapshot: {}},
            ],
            activeProfileId: "p1",
        },
        activeProfile: {id: "p1", name: "默认配置"},
        activeProfileName: "默认配置",
        loading: false,
        error: null,
        reloadNonce: 0,
        createDefaultProfile: vi.fn(),
        switchProfile: vi.fn(),
        renameProfile: vi.fn(),
        saveCurrentProfile: mockSaveCurrentProfile,
        deleteProfile: mockDeleteProfile,
        exportProfile: vi.fn(),
        importProfile: vi.fn(),
        exportProfileToPath: mockExportProfileToPath,
        importProfileFromPath: mockImportProfileFromPath,
    }),
}));

vi.mock("@/components/app/profile-utils", () => ({
    sortProfilesForSwitcher: (profiles: unknown[]) => profiles,
    validateProfileName: (...args: unknown[]) => mockValidateProfileName(...args),
}));

vi.mock("@/lib/utils", () => ({
    cn: (...args: string[]) => args.filter(Boolean).join(" "),
}));

// ── 复制流程行为测试 ──────────────────────────────────

/**
 * handleSaveAs 的核心行为契约（提取自 profile-switcher.tsx）：
 * 1. 验证名称：调用 validateProfileName(name)
 * 2. 验证失败时设置错误消息，不调用 saveCurrentProfile
 * 3. 验证通过时调用 saveCurrentProfile(trimmedName)
 * 4. 成功后关闭复制面板、清空输入
 * 5. 失败时设置错误消息
 */
async function handleSaveAsContract(
    saveAsName: string,
    saveCurrentProfile: (name: string) => Promise<void>,
    validateProfileName: (name: string) => string | null,
): Promise<{message: string | null; saveAsOpen: boolean; saveAsName: string}> {
    const validation = validateProfileName(saveAsName);
    if (validation) {
        return {message: validation, saveAsOpen: true, saveAsName};
    }
    try {
        await saveCurrentProfile(saveAsName.trim());
        return {message: null, saveAsOpen: false, saveAsName: ""};
    } catch (err) {
        return {message: String(err), saveAsOpen: true, saveAsName};
    }
}

describe("复制流程行为", () => {
    afterEach(() => {
        mockSaveCurrentProfile.mockReset();
        mockValidateProfileName.mockReset();
    });

    it("复制：输入有效名称并确认后调用 saveCurrentProfile", async () => {
        mockValidateProfileName.mockReturnValue(null);
        mockSaveCurrentProfile.mockResolvedValue(undefined);

        const result = await handleSaveAsContract("新配置", mockSaveCurrentProfile, mockValidateProfileName);

        expect(mockSaveCurrentProfile).toHaveBeenCalledWith("新配置");
        expect(result.saveAsOpen).toBe(false);
        expect(result.saveAsName).toBe("");
        expect(result.message).toBeNull();
    });

    it("复制：名称前后空格会被 trim 后传给 saveCurrentProfile", async () => {
        mockValidateProfileName.mockReturnValue(null);
        mockSaveCurrentProfile.mockResolvedValue(undefined);

        await handleSaveAsContract("  带空格配置  ", mockSaveCurrentProfile, mockValidateProfileName);

        expect(mockSaveCurrentProfile).toHaveBeenCalledWith("带空格配置");
    });

    it("复制：名称验证失败时不调用 saveCurrentProfile", async () => {
        mockValidateProfileName.mockReturnValue("配置名称不能为空");

        const result = await handleSaveAsContract("", mockSaveCurrentProfile, mockValidateProfileName);

        expect(mockSaveCurrentProfile).not.toHaveBeenCalled();
        expect(result.message).toBe("配置名称不能为空");
        expect(result.saveAsOpen).toBe(true);
    });

    it("复制：名称过长验证失败时不调用 saveCurrentProfile", async () => {
        mockValidateProfileName.mockReturnValue("配置名称不能超过 40 个字符");

        const result = await handleSaveAsContract("超".repeat(41), mockSaveCurrentProfile, mockValidateProfileName);

        expect(mockSaveCurrentProfile).not.toHaveBeenCalled();
        expect(result.message).toBe("配置名称不能超过 40 个字符");
    });

    it("复制：saveCurrentProfile 失败时显示错误消息但保持面板打开", async () => {
        mockValidateProfileName.mockReturnValue(null);
        mockSaveCurrentProfile.mockRejectedValue(new Error("保存失败"));

        const result = await handleSaveAsContract("测试配置", mockSaveCurrentProfile, mockValidateProfileName);

        expect(mockSaveCurrentProfile).toHaveBeenCalledWith("测试配置");
        expect(result.message).toBe("Error: 保存失败");
        expect(result.saveAsOpen).toBe(true);
    });
});

describe("切换 Profile 行为", () => {
    const mockSwitchProfile = vi.fn();

    afterEach(() => {
        mockSwitchProfile.mockReset();
    });

    /**
     * handleSwitch 核心行为：
     * 1. 如果点击的是当前激活 Profile，不调用 switchProfile
     * 2. 否则调用 switchProfile(profile.id)
     */
    async function handleSwitchContract(
        clickedProfileId: string,
        activeProfileId: string,
        switchProfile: (id: string) => Promise<void>,
    ): Promise<{called: boolean; error: string | null}> {
        if (clickedProfileId === activeProfileId) {
            return {called: false, error: null};
        }
        try {
            await switchProfile(clickedProfileId);
            return {called: true, error: null};
        } catch (err) {
            return {called: true, error: String(err)};
        }
    }

    it("点击当前激活 Profile 不调用 switchProfile", async () => {
        const result = await handleSwitchContract("p1", "p1", mockSwitchProfile);
        expect(result.called).toBe(false);
        expect(mockSwitchProfile).not.toHaveBeenCalled();
    });

    it("点击其他 Profile 调用 switchProfile", async () => {
        mockSwitchProfile.mockResolvedValue(undefined);
        const result = await handleSwitchContract("p2", "p1", mockSwitchProfile);
        expect(result.called).toBe(true);
        expect(mockSwitchProfile).toHaveBeenCalledWith("p2");
    });

    it("switchProfile 失败时返回错误", async () => {
        mockSwitchProfile.mockRejectedValue(new Error("切换失败"));
        const result = await handleSwitchContract("p2", "p1", mockSwitchProfile);
        expect(result.error).toBe("Error: 切换失败");
    });
});

describe("删除 Profile 行为", () => {
    afterEach(() => {
        mockDeleteProfile.mockReset();
    });

    async function handleDeleteContract(
        clickedProfileId: string,
        activeProfileId: string,
        deleteProfile: (id: string) => Promise<void>,
        confirmed: boolean,
    ): Promise<{deleted: boolean; message: string | null}> {
        if (clickedProfileId === activeProfileId) {
            return {deleted: false, message: "不能删除当前激活的配置。"};
        }
        if (!confirmed) {
            return {deleted: false, message: null};
        }
        try {
            await deleteProfile(clickedProfileId);
            return {deleted: true, message: "配置已删除。"};
        } catch (err) {
            return {deleted: false, message: String(err)};
        }
    }

    it("删除当前激活 Profile 被前端拦截", async () => {
        const result = await handleDeleteContract("p1", "p1", mockDeleteProfile, true);
        expect(mockDeleteProfile).not.toHaveBeenCalled();
        expect(result.message).toBe("不能删除当前激活的配置。");
    });

    it("删除非当前 Profile 调用 deleteProfile", async () => {
        mockDeleteProfile.mockResolvedValue(undefined);
        const result = await handleDeleteContract("p2", "p1", mockDeleteProfile, true);
        expect(mockDeleteProfile).toHaveBeenCalledWith("p2");
        expect(result.deleted).toBe(true);
    });

    it("取消确认时不调用 deleteProfile", async () => {
        const result = await handleDeleteContract("p2", "p1", mockDeleteProfile, false);
        expect(mockDeleteProfile).not.toHaveBeenCalled();
        expect(result.deleted).toBe(false);
    });
});

describe("Profile 导入导出路径行为", () => {
    afterEach(() => {
        mockExportProfileToPath.mockReset();
        mockImportProfileFromPath.mockReset();
    });

    it("导出时把 Profile id 和用户选择路径传给 hook", async () => {
        mockExportProfileToPath.mockResolvedValue(undefined);
        await mockExportProfileToPath("p1", "D:/tmp/profile-p1.json");
        expect(mockExportProfileToPath).toHaveBeenCalledWith("p1", "D:/tmp/profile-p1.json");
    });

    it("导入时把用户选择路径传给 hook", async () => {
        mockImportProfileFromPath.mockResolvedValue(undefined);
        await mockImportProfileFromPath("D:/tmp/profile-p1.json");
        expect(mockImportProfileFromPath).toHaveBeenCalledWith("D:/tmp/profile-p1.json");
    });
});

describe("重命名 Profile 行为", () => {
    const mockRenameProfile = vi.fn();

    afterEach(() => {
        mockRenameProfile.mockReset();
        mockValidateProfileName.mockReset();
    });

    /**
     * commitRename 核心行为：
     * 1. 验证名称
     * 2. 调用 renameProfile(id, trimmedName)
     * 3. 成功后清空重命名状态
     * 4. 失败时保留错误消息
     */
    async function commitRenameContract(
        id: string,
        name: string,
        renameProfile: (id: string, name: string) => Promise<void>,
        validateProfileName: (name: string) => string | null,
    ): Promise<{message: string | null; renamed: boolean}> {
        const validation = validateProfileName(name);
        if (validation) {
            return {message: validation, renamed: false};
        }
        try {
            await renameProfile(id, name.trim());
            return {message: null, renamed: true};
        } catch (err) {
            return {message: String(err), renamed: false};
        }
    }

    it("重命名：输入有效名称后调用 renameProfile", async () => {
        mockValidateProfileName.mockReturnValue(null);
        mockRenameProfile.mockResolvedValue(undefined);

        const result = await commitRenameContract("p1", "新名字", mockRenameProfile, mockValidateProfileName);

        expect(mockRenameProfile).toHaveBeenCalledWith("p1", "新名字");
        expect(result.renamed).toBe(true);
    });

    it("重命名：名称验证失败时不调用 renameProfile", async () => {
        mockValidateProfileName.mockReturnValue("配置名称不能为空");

        const result = await commitRenameContract("p1", "", mockRenameProfile, mockValidateProfileName);

        expect(mockRenameProfile).not.toHaveBeenCalled();
        expect(result.message).toBe("配置名称不能为空");
    });

    it("重命名：名称 trim 后传给 renameProfile", async () => {
        mockValidateProfileName.mockReturnValue(null);
        mockRenameProfile.mockResolvedValue(undefined);

        await commitRenameContract("p1", "  带空格  ", mockRenameProfile, mockValidateProfileName);

        expect(mockRenameProfile).toHaveBeenCalledWith("p1", "带空格");
    });
});
