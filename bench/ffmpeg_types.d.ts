export interface BridgeLimits {
    timeoutMs?: number;
    maxQueuedJobs?: number;
    maxQueuedBytes?: number;
    maxOutputBytes?: number;
}
export type BridgeResult = { data: Uint8Array; error?: never } | { error: string; data?: never };
export interface BridgeInput { name: string; data: Uint8Array }
export interface FFmpegInstance {
    loaded?: boolean;
    exec(args: string[], timeout?: number): Promise<number>;
    on(event: 'log', listener: (event: { message: string }) => void): void;
    off(event: 'log', listener: (event: { message: string }) => void): void;
    writeFile(path: string, data: Uint8Array): Promise<unknown>;
    readFile(path: string): Promise<Uint8Array | string>;
    deleteFile(path: string): Promise<unknown>;
    createDir(path: string): Promise<unknown>;
    deleteDir(path: string): Promise<unknown>;
}
