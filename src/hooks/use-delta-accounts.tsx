import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { DeltaAccountRecord } from "@/components/app/delta-types";

type DeltaAccountsContextValue = {
  accounts: DeltaAccountRecord[];
  selectedAccountId: number | null;
  selectedAccount: DeltaAccountRecord | null;
  selectAccount: (id: number | null) => void;
  refreshAccounts: () => Promise<void>;
  isNativeShell: boolean;
};

const DeltaAccountsContext = createContext<DeltaAccountsContextValue | null>(null);

export function useDeltaAccounts(): DeltaAccountsContextValue {
  const ctx = useContext(DeltaAccountsContext);
  if (!ctx) throw new Error("useDeltaAccounts must be used within DeltaAccountsProvider");
  return ctx;
}

export function DeltaAccountsProvider({ children }: { children: ReactNode }) {
  const [accounts, setAccounts] = useState<DeltaAccountRecord[]>([]);
  const [selectedAccountId, setSelectedAccountId] = useState<number | null>(null);
  const [isNativeShell, setIsNativeShell] = useState(false);
  const mountedRef = useRef(true);

  useEffect(() => {
    mountedRef.current = true;
    setIsNativeShell(typeof window !== "undefined" && "__TAURI_INTERNALS__" in window);
    return () => { mountedRef.current = false; };
  }, []);

  const refreshAccounts = useCallback(async () => {
    try {
      const res = await invoke<{ code: number; msg: string; data: DeltaAccountRecord[] }>("delta_list_accounts");
      if (mountedRef.current && res.code === 0) {
        setAccounts(res.data);
      }
    } catch {
      // 非原生环境或后端错误
    }
  }, []);

  useEffect(() => {
    if (isNativeShell) {
      refreshAccounts();
    }
  }, [isNativeShell, refreshAccounts]);

  const selectedAccount = useMemo(() => {
    if (selectedAccountId === null) return null;
    return accounts.find((a) => a.id === selectedAccountId) ?? null;
  }, [accounts, selectedAccountId]);

  const selectAccount = useCallback((id: number | null) => {
    setSelectedAccountId(id);
  }, []);

  const value = useMemo(() => ({
    accounts,
    selectedAccountId,
    selectedAccount,
    selectAccount,
    refreshAccounts,
    isNativeShell,
  }), [accounts, selectedAccountId, selectedAccount, selectAccount, refreshAccounts, isNativeShell]);

  return (
    <DeltaAccountsContext.Provider value={value}>
      {children}
    </DeltaAccountsContext.Provider>
  );
}
