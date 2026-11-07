import { writeFileSync } from "node:fs";

const targetId = "0CA62F40F9918C9E25B63C76702A9AC3";
const wsUrl = `ws://127.0.0.1:9222/devtools/page/${targetId}`;

console.log("Navigating Chrome to https://3dland.tech/games/budokai-tenkaichi-3/ via CDP...");
const ws = new WebSocket(wsUrl);
let id = 1;
const pending = new Map();
const consoleLogs = [];
const networkErrors = [];

function send(method, params = {}) {
  const reqId = id++;
  return new Promise((resolve) => {
    pending.set(reqId, resolve);
    ws.send(JSON.stringify({ id: reqId, method, params }));
  });
}

ws.onopen = async () => {
  await send("Page.enable");
  await send("Runtime.enable");
  await send("Console.enable");
  await send("Network.enable");

  await send("Page.navigate", { url: "https://3dland.tech/games/budokai-tenkaichi-3/" });

  // Wait 6 seconds for boot sequence and disc header stream
  await new Promise(r => setTimeout(r, 6000));

  const state = await send("Runtime.evaluate", {
    expression: `(() => {
      const splash = document.getElementById("boot-splash");
      const statusEl = document.getElementById("boot-status");
      const canvasList = Array.from(document.querySelectorAll("canvas"));
      return {
        title: document.title,
        bootStatusText: statusEl ? statusEl.innerText : null,
        splashHidden: splash ? splash.classList.contains("hidden") : null,
        splashOpacity: splash ? getComputedStyle(splash).opacity : null,
        canvasCount: canvasList.length,
        canvases: canvasList.map(c => ({ width: c.width, height: c.height, style: c.style.cssText }))
      };
    })()`,
    returnByValue: true
  });

  console.log("\n=== GAME PAGE STATE ===");
  console.log(JSON.stringify(state.result?.result?.value, null, 2));

  console.log("\n=== CONSOLE LOGS ===");
  console.log(JSON.stringify(consoleLogs, null, 2));

  console.log("\n=== NETWORK ERRORS ===");
  console.log(JSON.stringify(networkErrors, null, 2));

  const shot = await send("Page.captureScreenshot", { format: "png" });
  if (shot.result?.data) {
    const buf = Buffer.from(shot.result.data, "base64");
    writeFileSync("C:/src/wii/wii-os/userland/build/chrome-game-live.png", buf);
    console.log("Screenshot saved to C:/src/wii/wii-os/userland/build/chrome-game-live.png");
  }

  ws.close();
  process.exit(0);
};

ws.onmessage = (event) => {
  const data = JSON.parse(event.data);
  if (data.id && pending.has(data.id)) {
    const resolve = pending.get(data.id);
    pending.delete(data.id);
    resolve(data);
  }
  if (data.method === "Console.messageAdded") {
    consoleLogs.push(data.params.message);
  }
  if (data.method === "Runtime.consoleAPICalled") {
    consoleLogs.push({ type: data.params.type, args: data.params.args?.map(a => a.value || a.description) });
  }
  if (data.method === "Runtime.exceptionThrown") {
    consoleLogs.push({ type: "exception", text: data.params.exceptionDetails?.text, desc: data.params.exceptionDetails?.exception?.description });
  }
  if (data.method === "Network.responseReceived" && data.params.response.status >= 400) {
    networkErrors.push({ url: data.params.response.url, status: data.params.response.status });
  }
};
