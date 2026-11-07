// Chrome DevTools Protocol Inspector for Wii OS (https://3dland.tech/)
import { writeFileSync } from "node:fs";

const wsUrl = "ws://127.0.0.1:9222/devtools/page/6E90D4F519B1B9CECF5405E8B0DDE1E8";
console.log("Connecting to Chrome CDP:", wsUrl);

const ws = new WebSocket(wsUrl);
let id = 1;
const pending = new Map();

function send(method, params = {}) {
  const reqId = id++;
  return new Promise((resolve) => {
    pending.set(reqId, resolve);
    ws.send(JSON.stringify({ id: reqId, method, params }));
  });
}

ws.onopen = async () => {
  console.log("Connected to Chrome DevTools Protocol!");
  await send("Page.enable");
  await send("Runtime.enable");
  await send("Console.enable");
  await send("Network.enable");

  // Wait 3 seconds for DOM, sounds, and assets to settle
  await new Promise((r) => setTimeout(r, 3000));

  // Evaluate DOM
  const evalResult = await send("Runtime.evaluate", {
    expression: `(() => {
      return {
        title: document.title,
        url: window.location.href,
        channelsCount: document.querySelectorAll('.channels .channel-icon').length,
        visibleButtons: Array.from(document.querySelectorAll('[role="button"], button, .btn')).map(b => b.innerText || b.getAttribute('aria-label') || b.className),
        audioElements: Array.from(document.querySelectorAll('audio')).map(a => ({ id: a.id, src: a.src, readyState: a.readyState })),
        bodyClasses: document.body.className
      };
    })()`,
    returnByValue: true,
  });

  console.log("DOM Inspection:", JSON.stringify(evalResult.result?.result?.value, null, 2));

  // Take screenshot
  const screenshot = await send("Page.captureScreenshot", { format: "png" });
  if (screenshot.result?.data) {
    const buffer = Buffer.from(screenshot.result.data, "base64");
    const outPath = "C:/src/wii/wii-os/userland/build/chrome-wii-menu-screenshot.png";
    writeFileSync(outPath, buffer);
    console.log(`Screenshot saved to ${outPath} (${buffer.length} bytes)`);
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
    console.log("[Chrome Console]", data.params.message.text);
  }
};

ws.onerror = (err) => {
  console.error("CDP Error:", err);
  process.exit(1);
};
