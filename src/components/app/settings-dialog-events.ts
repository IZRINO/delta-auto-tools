export const SETTINGS_DIALOG_OPEN_EVENT = "delta-auto-tools:settings-dialog-open";
export const SETTINGS_DIALOG_CLOSE_EVENT = "delta-auto-tools:settings-dialog-close";

const SETTINGS_DIALOG_DATA_KEY = "settingsDialogOpen";
const activeCoverSources = new Set<string>();

export function isSettingsDialogOpen(): boolean {
    return document.documentElement.dataset[SETTINGS_DIALOG_DATA_KEY] === "true";
}

export function publishUiCoverState(source: string, open: boolean): void {
    const wasOpen = isSettingsDialogOpen();
    if (open) {
        activeCoverSources.add(source);
        document.documentElement.dataset[SETTINGS_DIALOG_DATA_KEY] = "true";
    } else {
        activeCoverSources.delete(source);
        if (activeCoverSources.size === 0) {
            delete document.documentElement.dataset[SETTINGS_DIALOG_DATA_KEY];
        }
    }
    const nextOpen = isSettingsDialogOpen();
    if (wasOpen !== nextOpen) {
        window.dispatchEvent(new Event(nextOpen ? SETTINGS_DIALOG_OPEN_EVENT : SETTINGS_DIALOG_CLOSE_EVENT));
    }
}

export function publishSettingsDialogState(open: boolean): void {
    publishUiCoverState("settings-dialog", open);
}
