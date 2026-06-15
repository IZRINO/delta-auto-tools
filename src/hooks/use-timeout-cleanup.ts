import {useEffect, useRef} from "react";

export function useTimeoutCleanup() {
    const timeoutRef = useRef<number | null>(null);

    useEffect(() => {
        return () => {
            if (timeoutRef.current !== null) {
                window.clearTimeout(timeoutRef.current);
            }
        };
    }, []);

    return timeoutRef;
}
