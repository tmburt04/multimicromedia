import type { BridgeLimits, BridgeInput, BridgeResult, FFmpegInstance } from './ffmpeg_types.js';
export type { BridgeLimits, BridgeInput, BridgeResult, FFmpegInstance } from './ffmpeg_types.js';
export function initialize(ffmpeg: FFmpegInstance, options?: BridgeLimits): Promise<void>;
export function isAvailable(): boolean;
export function validateArgs(args: string[]): string | null;
export function execute(args: string[], data: Uint8Array): Promise<BridgeResult>;
export function executeMultiInput(inputs: BridgeInput[], args: string[]): Promise<BridgeResult>;
export function getVersion(): Promise<string>;
