import {invoke} from "@tauri-apps/api/core";
import {getErrorMessage} from "@/lib/error-utils";

const STARTUP_RETRY_MS = 2000;
const STARTUP_RETRY_DELAY_MS = 100;

function sleep(ms: number): Promise<void> {
    return new Promise((resolve) => globalThis.setTimeout(resolve, ms));
}

function isStateNotManagedError(error: unknown): boolean {
    return getErrorMessage(error).includes("state not managed");
}

export async function invokeWithStartupRetry<T>(command: string): Promise<T> {
    const startedAt = Date.now();
    for (;;) {
        try {
            return await invoke<T>(command);
        } catch (error) {
            if (!isStateNotManagedError(error) || Date.now() - startedAt >= STARTUP_RETRY_MS) {
                throw error;
            }
            await sleep(STARTUP_RETRY_DELAY_MS);
        }
    }
}
