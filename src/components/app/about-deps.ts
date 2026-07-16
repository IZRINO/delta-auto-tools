import type {Dependency} from "@/components/app/about-types";

/**
 * 前端 + Rust 后端主要依赖致谢列表。
 * 与 Rust 端 about::built_in_dependencies() 保持同步。
 */
export const DEPENDENCIES: Dependency[] = [
    // ── 前端 ──
    {name: "React 19", kind: "frontend", license: "MIT", url: "https://react.dev"},
    {name: "Vite 7", kind: "frontend", license: "MIT", url: "https://vite.dev"},
    {name: "@remixicon/react", kind: "frontend", license: "Apache-2.0", url: "https://remixicon.com"},
    {
        name: "@fontsource-variable/jetbrains-mono",
        kind: "frontend",
        license: "OFL-1.1",
        url: "https://fontsource.org/fonts/jetbrains-mono"
    },
    {name: "@tauri-apps/api", kind: "frontend", license: "MIT/Apache-2.0", url: "https://tauri.app"},
    {name: "@tauri-apps/plugin-opener", kind: "frontend", license: "MIT/Apache-2.0", url: "https://tauri.app"},
    {name: "@tauri-apps/plugin-process", kind: "frontend", license: "MIT/Apache-2.0", url: "https://tauri.app"},
    {name: "radix-ui", kind: "frontend", license: "MIT", url: "https://www.radix-ui.com"},
    {name: "tailwindcss 4", kind: "frontend", license: "MIT", url: "https://tailwindcss.com"},
    {name: "sonner", kind: "frontend", license: "MIT", url: "https://sonner.emilkowal.dev"},
    // ── Rust 运行时 ──
    {name: "tauri", kind: "runtime", license: "MIT/Apache-2.0", url: "https://tauri.app"},
    {name: "tauri-plugin-updater", kind: "runtime", license: "MIT/Apache-2.0", url: "https://tauri.app"},
    {name: "tauri-plugin-opener", kind: "runtime", license: "MIT/Apache-2.0", url: "https://tauri.app"},
    {name: "tauri-plugin-window-state", kind: "runtime", license: "MIT/Apache-2.0", url: "https://tauri.app"},
    {name: "tauri-plugin-process", kind: "runtime", license: "MIT/Apache-2.0", url: "https://tauri.app"},
    {name: "enigo", kind: "runtime", license: "MIT", url: "https://github.com/enigo-rs/enigo"},
    {name: "willhook", kind: "runtime", license: "MIT", url: "https://github.com/2hndr/willhook"},
    {name: "xcap", kind: "runtime", license: "MIT", url: "https://github.com/nicedoc/xcap"},
    {name: "image", kind: "runtime", license: "MIT/Apache-2.0", url: "https://github.com/image-rs/image"},
    {name: "rodio", kind: "runtime", license: "MIT/Apache-2.0", url: "https://github.com/RustAudio/rodio"},
    {name: "tokio", kind: "runtime", license: "MIT", url: "https://tokio.rs"},
    {name: "serde", kind: "runtime", license: "MIT/Apache-2.0", url: "https://serde.rs"},
    {name: "serde_json", kind: "runtime", license: "MIT/Apache-2.0", url: "https://github.com/serde-rs/json"},
    {name: "thiserror", kind: "runtime", license: "MIT/Apache-2.0", url: "https://github.com/dtolnay/thiserror"},
    {
        name: "crossbeam-channel",
        kind: "runtime",
        license: "MIT/Apache-2.0",
        url: "https://github.com/crossbeam-rs/crossbeam"
    },
    {name: "url", kind: "runtime", license: "MIT/Apache-2.0", url: "https://github.com/servo/rust-url"},
    {name: "windows-sys", kind: "runtime", license: "MIT/Apache-2.0", url: "https://github.com/microsoft/windows-rs"},
];
