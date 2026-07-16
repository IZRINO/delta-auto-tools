type SaveTask<T> = {
    value: T;
    save: (value: T) => Promise<void>;
    waiters: Array<{
        resolve: () => void;
        reject: (error: unknown) => void;
    }>;
};

export class LatestSaveQueue<T> {
    private pending: SaveTask<T> | null = null;
    private draining = false;

    enqueue(value: T, save: (value: T) => Promise<void>): Promise<void> {
        const promise = new Promise<void>((resolve, reject) => {
            const waiters = this.pending?.waiters ?? [];
            this.pending = {value, save, waiters: [...waiters, {resolve, reject}]};
        });
        if (!this.draining) {
            this.draining = true;
            void this.drain();
        }
        return promise;
    }

    private async drain(): Promise<void> {
        while (this.pending) {
            const task = this.pending;
            this.pending = null;
            try {
                await task.save(task.value);
                task.waiters.forEach(({resolve}) => resolve());
            } catch (error) {
                task.waiters.forEach(({reject}) => reject(error));
            }
        }
        this.draining = false;
    }
}
