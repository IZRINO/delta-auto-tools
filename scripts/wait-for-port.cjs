const net = require("node:net");
const { spawn } = require("node:child_process");

const [portArg, ...commandParts] = process.argv.slice(2);
const port = Number.parseInt(portArg ?? "", 10);

if (!Number.isInteger(port) || port <= 0) {
  throw new Error("A valid port number is required.");
}

if (commandParts.length === 0) {
  throw new Error("A command to run after the port is ready is required.");
}

function waitForPort(portNumber, timeoutMs = 30000) {
  const startedAt = Date.now();

  return new Promise((resolve, reject) => {
    const tryConnect = () => {
      const socket = net.connect({ host: "127.0.0.1", port: portNumber });

      socket.once("connect", () => {
        socket.end();
        resolve();
      });

      socket.once("error", () => {
        socket.destroy();

        if (Date.now() - startedAt >= timeoutMs) {
          reject(new Error(`Timed out waiting for port ${portNumber}.`));
          return;
        }

        setTimeout(tryConnect, 500);
      });
    };

    tryConnect();
  });
}

async function main() {
  await waitForPort(port);

  const child = spawn(commandParts[0], commandParts.slice(1), {
    cwd: process.cwd(),
    stdio: "inherit",
    shell: true,
    windowsHide: true,
  });

  child.on("exit", (code) => {
    process.exit(code ?? 0);
  });
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
});
