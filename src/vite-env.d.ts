/// <reference types="vite/client" />

declare module "*?raw" {
    const content: string;
    export default content;
}

// ponytail: culori@4 未打包 TS 类型，按本项目实际使用的子集声明
declare module "culori" {
    export interface Color {
        mode: string;
        l?: number;
        c?: number;
        h?: number;
        r?: number;
        g?: number;
        b?: number;
        alpha?: number;
    }
    export function parse(input: string): Color | undefined;
    export function oklch(color: Color): Color | undefined;
    export function rgb(color: Color): Color | undefined;
    export function formatHex(color: Color): string | undefined;
}
