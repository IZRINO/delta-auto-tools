import {listen, type EventCallback, type UnlistenFn} from "@tauri-apps/api/event";

export function subscribeTauriEvent<T>(
    event: string,
    handler: EventCallback<T>,
    onError?: (error: unknown) => void,
): () => void {
    let disposed = false;
    let unlisten: UnlistenFn | undefined;
    let unlistened = false;

    const disposeListener = () => {
        if (!unlisten || unlistened) return;
        unlistened = true;
        unlisten();
    };

    void listen<T>(event, (tauriEvent) => {
        if (!disposed) handler(tauriEvent);
    }).then(
        (nextUnlisten) => {
            unlisten = nextUnlisten;
            if (disposed) disposeListener();
        },
        (error: unknown) => {
            onError?.(error);
        },
    );

    return () => {
        if (disposed) return;
        disposed = true;
        disposeListener();
    };
}
