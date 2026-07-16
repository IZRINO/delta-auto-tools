import {
    createContext,
    type ReactNode,
    useCallback,
    useContext,
    useEffect,
    useMemo,
    useState,
} from "react";
import {invokeLogged as invoke} from "@/lib/logging";

import {PROFILE_EVENTS} from "@/lib/tauri-events";
import {subscribeTauriEvent} from "@/lib/tauri-listener";
import {useNativeShell} from "@/hooks/use-native-shell";
import type {Profile, ProfileBootstrap} from "@/components/app/profile-types";
import {
    getActiveProfile,
    getProfileDisplayName,
} from "@/components/app/profile-utils";

/** Profile Context 对外暴露的接口。 */
type ProfileContextValue = {
    /** 当前 bootstrap（含全部 Profile 列表与激活 id）。 */
    bootstrap: ProfileBootstrap | null;
    /** 当前激活配置对象。 */
    activeProfile: Profile | null;
    /** 顶栏显示名；无 bootstrap 时回退为"配置1"。 */
    activeProfileName: string;
    /** 是否正在加载初始 bootstrap。 */
    loading: boolean;
    /** 错误信息。 */
    error: string | null;
    /**
     * 切换 Profile 后自增的 nonce。
     *
     * AppShell 用它作为工具页容器的 key：切换 Profile 后 nonce 变化 →
     * 当前工具页 unmount（清掉挂起的 autosave timer）→ remount（重新拉 bootstrap）。
     */
    reloadNonce: number;
    /** 当前正在切换的 Profile id；无切换时为 null。 */
    switchingProfileId: string | null;
    /** 新建一个全默认配置并立即切换过去。 */
    createDefaultProfile: () => Promise<void>;
    /** 切换到指定 Profile：写盘 + reload 各工具 + 重置计数器运行值。 */
    switchProfile: (id: string) => Promise<void>;
    /** 重命名 Profile。 */
    renameProfile: (id: string, name: string) => Promise<void>;
    /** 另存为：将当前运行时设置快照保存为新 Profile。 */
    saveCurrentProfile: (name: string) => Promise<void>;
    /** 删除非当前激活 Profile。 */
    deleteProfile: (id: string) => Promise<void>;
    /** 导出单个 Profile 为 JSON 字符串。 */
    exportProfile: (id: string) => Promise<string>;
    /** 从 JSON 字符串导入单个 Profile；不自动切换。 */
    importProfile: (json: string) => Promise<void>;
    /** 导出单个 Profile 到用户选择的路径。 */
    exportProfileToPath: (id: string, path: string) => Promise<void>;
    /** 从用户选择的路径导入单个 Profile；不自动切换。 */
    importProfileFromPath: (path: string) => Promise<void>;
};

const ProfileContext = createContext<ProfileContextValue | null>(null);

export function useProfile(): ProfileContextValue {
    const ctx = useContext(ProfileContext);
    if (!ctx) {
        throw new Error("useProfile 必须在 ProfileProvider 内使用");
    }
    return ctx;
}

type ProfileProviderProps = {
    children: ReactNode;
};

/**
 * Profile Provider。
 *
 * - 浏览器预览模式：不调后端，直接结束 loading。
 * - native shell：调 `profile_get_bootstrap` 拿真实配置；切换 Profile 后自增 reloadNonce
 *   通知 AppShell 重新挂载工具页（清挂起 autosave + 重拉新配置）。
 */
export function ProfileProvider({children}: ProfileProviderProps) {
    const isNativeShell = useNativeShell();
    const [bootstrap, setBootstrap] = useState<ProfileBootstrap | null>(null);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);
    const [reloadNonce, setReloadNonce] = useState(0);
    const [switchingProfileId, setSwitchingProfileId] = useState<string | null>(null);

    // 初始化：拉取 bootstrap
    useEffect(() => {
        if (!isNativeShell) {
            setLoading(false);
            return;
        }

        let disposed = false;
        void invoke<ProfileBootstrap>("profile_get_bootstrap")
            .then((boot) => {
                if (disposed) return;
                setBootstrap(boot);
            })
            .catch((err: unknown) => {
                if (!disposed) setError(String(err));
            })
            .finally(() => {
                if (!disposed) setLoading(false);
            });

        return () => {
            disposed = true;
        };
    }, [isNativeShell]);

    // 监听 profile://changed 事件：其他窗口或后端推送的变更刷新列表
    useEffect(() => {
        if (!isNativeShell) return;

        let disposed = false;

        const unlisten = subscribeTauriEvent<ProfileBootstrap>(PROFILE_EVENTS.changed, (event) => {
            if (disposed) return;
            setBootstrap(event.payload);
        });

        return () => {
            disposed = true;
            unlisten();
        };
    }, [isNativeShell]);

    /** 切换后刷新本地 bootstrap + 自增 reloadNonce 触发工具页重挂载。 */
    const refreshAfterSwitch = useCallback(async () => {
        const boot = await invoke<ProfileBootstrap>("profile_get_bootstrap");
        setBootstrap(boot);
        setReloadNonce((n) => n + 1);
    }, []);

    const switchProfile = useCallback(
        async (id: string) => {
            if (switchingProfileId) return;
            setSwitchingProfileId(id);
            try {
                await invoke("profile_apply", {id});
                await refreshAfterSwitch();
            } catch (err: unknown) {
                setError(String(err));
                throw err;
            } finally {
                setSwitchingProfileId(null);
            }
        },
        [refreshAfterSwitch, switchingProfileId],
    );

    const renameProfile = useCallback(async (id: string, name: string) => {
        try {
            const boot = await invoke<ProfileBootstrap>("profile_rename", {id, name});
            setBootstrap(boot);
        } catch (err: unknown) {
            setError(String(err));
            throw err;
        }
    }, []);

    const saveCurrentProfile = useCallback(
        async (name: string) => {
            try {
                await invoke<Profile>("profile_save_current", {name});
                await refreshAfterSwitch();
            } catch (err: unknown) {
                setError(String(err));
                throw err;
            }
        },
        [refreshAfterSwitch],
    );

    const createDefaultProfile = useCallback(async () => {
        try {
            const boot = await invoke<ProfileBootstrap>("profile_create_default");
            setBootstrap(boot);
            setReloadNonce((n) => n + 1);
        } catch (err: unknown) {
            setError(String(err));
            throw err;
        }
    }, []);

    const deleteProfile = useCallback(async (id: string) => {
        try {
            const boot = await invoke<ProfileBootstrap>("profile_delete", {id});
            setBootstrap(boot);
        } catch (err: unknown) {
            setError(String(err));
            throw err;
        }
    }, []);

    const exportProfile = useCallback(async (id: string) => {
        try {
            return await invoke<string>("profile_export", {id});
        } catch (err: unknown) {
            setError(String(err));
            throw err;
        }
    }, []);

    const importProfile = useCallback(async (json: string) => {
        try {
            const boot = await invoke<ProfileBootstrap>("profile_import", {json});
            setBootstrap(boot);
        } catch (err: unknown) {
            setError(String(err));
            throw err;
        }
    }, []);

    const exportProfileToPath = useCallback(async (id: string, path: string) => {
        try {
            await invoke("profile_export_to_path", {id, path});
        } catch (err: unknown) {
            setError(String(err));
            throw err;
        }
    }, []);

    const importProfileFromPath = useCallback(async (path: string) => {
        try {
            const boot = await invoke<ProfileBootstrap>("profile_import_from_path", {path});
            setBootstrap(boot);
        } catch (err: unknown) {
            setError(String(err));
            throw err;
        }
    }, []);

    const activeProfile = useMemo(() => getActiveProfile(bootstrap), [bootstrap]);
    const activeProfileName = useMemo(() => getProfileDisplayName(bootstrap), [bootstrap]);

    const value = useMemo<ProfileContextValue>(
        () => ({
            bootstrap,
            activeProfile,
            activeProfileName,
            loading,
            error,
            reloadNonce,
            switchingProfileId,
            createDefaultProfile,
            switchProfile,
            renameProfile,
            saveCurrentProfile,
            deleteProfile,
            exportProfile,
            importProfile,
            exportProfileToPath,
            importProfileFromPath,
        }),
        [
            bootstrap,
            activeProfile,
            activeProfileName,
            loading,
            error,
            reloadNonce,
            switchingProfileId,
            createDefaultProfile,
            switchProfile,
            renameProfile,
            saveCurrentProfile,
            deleteProfile,
            exportProfile,
            importProfile,
            exportProfileToPath,
            importProfileFromPath,
        ],
    );

    return <ProfileContext.Provider value={value}>{children}</ProfileContext.Provider>;
}
