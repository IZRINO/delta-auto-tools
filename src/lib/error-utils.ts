export function getErrorMessage(error: unknown): string {
    if (error instanceof Error) return error.message;
    return String(error ?? "发生未知错误");
}
