export type PositionPoint = {x: number; y: number};

type PositionMoveQueueOptions = {
    invoke: (point: PositionPoint) => Promise<void>;
    requestFrame: (callback: () => void) => number;
    cancelFrame: (id: number) => void;
    onError?: (error: unknown) => void;
};

export class PositionMoveQueue {
    private pending: PositionPoint | null = null;
    private frameId: number | null = null;
    private inFlight = false;
    private flushRequested = false;
    private disposed = false;
    private dispatchError: unknown = null;
    private idleWaiters: Array<{resolve: () => void; reject: (error: unknown) => void}> = [];

    constructor(private readonly options: PositionMoveQueueOptions) {}

    move(point: PositionPoint): void {
        if (this.disposed) return;
        this.pending = point;
        if (!this.inFlight && this.frameId === null) {
            this.frameId = this.options.requestFrame(() => {
                this.frameId = null;
                this.dispatchLatest();
            });
        }
    }

    flush(): Promise<void> {
        if (this.disposed) return Promise.resolve();
        this.flushRequested = true;
        if (this.frameId !== null) {
            this.options.cancelFrame(this.frameId);
            this.frameId = null;
        }
        this.dispatchLatest();
        if (!this.inFlight && this.pending === null) {
            this.flushRequested = false;
            return this.takeDispatchError();
        }
        return new Promise((resolve, reject) => this.idleWaiters.push({resolve, reject}));
    }

    dispose(): void {
        this.disposed = true;
        this.pending = null;
        if (this.frameId !== null) {
            this.options.cancelFrame(this.frameId);
            this.frameId = null;
        }
        if (!this.inFlight) this.resolveIdle();
    }

    private dispatchLatest(): void {
        if (this.inFlight || this.pending === null || this.disposed) return;
        const point = this.pending;
        this.pending = null;
        this.inFlight = true;

        let task: Promise<void>;
        try {
            task = this.options.invoke(point);
        } catch (error) {
            task = Promise.reject(error);
        }

        void task.then(() => {
            this.dispatchError = null;
        }, (error) => {
            this.dispatchError = error;
            this.options.onError?.(error);
        }).finally(() => {
            this.inFlight = false;
            if (this.pending !== null && !this.disposed) {
                if (this.flushRequested) {
                    this.dispatchLatest();
                } else {
                    this.move(this.pending);
                }
                return;
            }
            this.flushRequested = false;
            this.resolveIdle();
        });
    }

    private resolveIdle(): void {
        const waiters = this.idleWaiters.splice(0);
        if (waiters.length === 0) return;
        const error = this.dispatchError;
        this.dispatchError = null;
        for (const waiter of waiters) {
            if (error === null) waiter.resolve();
            else waiter.reject(error);
        }
    }

    private takeDispatchError(): Promise<void> {
        const error = this.dispatchError;
        this.dispatchError = null;
        return error === null ? Promise.resolve() : Promise.reject(error);
    }
}
