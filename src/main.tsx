import React from "react";
import ReactDOM from "react-dom/client";
import {ThemeProvider} from "next-themes";
import App from "./App";

import {Toaster} from "@/components/ui/sonner";
import {TooltipProvider} from "@/components/ui/tooltip";
import {initLogging, log} from "@/lib/logging";

// 初始化日志系统（从 Rust 获取 session_id）
initLogging().then(() => {
    // 生产环境 console 劫持：同时写入统一日志文件
    if (import.meta.env.PROD) {
        const origConsole = {
            log: console.log,
            warn: console.warn,
            error: console.error,
        };

        console.log = (...args: unknown[]) => {
            origConsole.log(...args);
            log.info("console", "auto", args.map(String).join(" "));
        };

        console.warn = (...args: unknown[]) => {
            origConsole.warn(...args);
            log.warn("console", "auto", args.map(String).join(" "));
        };

        console.error = (...args: unknown[]) => {
            origConsole.error(...args);
            log.error("console", "auto", args.map(String).join(" "));
        };
    }
});

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
        <ThemeProvider attribute="class" defaultTheme="light" disableTransitionOnChange enableSystem={false}>
            <TooltipProvider>
                <App/>
                <Toaster/>
            </TooltipProvider>
        </ThemeProvider>
    </React.StrictMode>,
);
