const path = require("node:path");

const projectRoot = __dirname;
const waitForPortScript = path.join(projectRoot, "scripts", "wait-for-port.cjs");

module.exports = {
    apps: [
        {
            name: "delta-auto-tools-1420",
            cwd: projectRoot,
            script: "bun",
            args: "run dev",
            interpreter: "none",
            windowsHide: true,
            env: {
                NODE_ENV: "development",
            },
        },
        {
            name: "delta-auto-tools-tauri",
            cwd: projectRoot,
            script: "node",
            args: `"${waitForPortScript}" 1420 bun run tauri dev --no-dev-server`,
            interpreter: "none",
            windowsHide: true,
            env: {
                NODE_ENV: "development",
            },
        },
    ],
};
