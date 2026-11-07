import { writeFileSync } from "node:fs";

const targetId = "0CA62F40F9918C9E25B63C76702A9AC3";
const wsUrl = `ws://127.0.0.1:9222/devtools/page/${targetId}`;

console.log("Connecting to Chrome target:", wsUrl);
const ws = new WebSocket(wsUrl);
let id = 1;
const pending = new Map();
const networkFailures = [];
const consoleMessages = [];

function send(method, params = {}) {
  const reqId = id++;
  return new Promise((resolve) => {
    pending.set(reqId, resolve);
    ws.send(JSON.stringify({ id: reqId, method, params }));
  });
}

ws.onopen = async () => {
  console.log("Connected to Chrome via CDP!");
  await send("Page.enable");
  await send("Runtime.enable");
  await send("Console.enable");
  await send("Network.enable");

  // Reload page bypassing cache
  console.log("Reloading https://3dland.tech/ (ignoreCache: true)...");
  await send("Page.reload", { ignoreCache: true });

  // Wait 4 seconds for full boot sequence, audio, scripts and views
  await new Promise((r) => setTimeout(r, 4000));

  // Inspect DOM state
  const domState = await send("Runtime.evaluate", {
    expression: `(() => {
      const app = document.querySelector('.app');
      const boot = document.getElementById('boot-sequence');
      const bootVisible = boot ? getComputedStyle(boot).display !== 'none' && getComputedStyle(boot).opacity !== '0' : false;
      const channels = Array.from(document.querySelectorAll('.channels .channel-icon')).map((el, i) => {
        const titleEl = el.querySelector('.title, span, p');
        const imgEl = el.querySelector('img');
        const bg = getComputedStyle(el).backgroundImage;
        return {
          index: i,
          title: titleEl ? titleEl.innerText : el.innerText,
          hasImg: Boolean(imgEl),
          imgSrc: imgEl ? imgEl.src : null,
          backgroundImage: bg,
          classes: el.className
        };
      });

      return {
        title: document.title,
        bootVisible,
        bootDisplay: boot ? getComputedStyle(boot).display : null,
        bootOpacity: boot ? getComputedStyle(boot).opacity : null,
        appHtmlLength: app ? app.innerHTML.length : 0,
        appChildrenCount: app ? app.children.length : 0,
        channelsCount: channels.length,
        channels,
        activeView: document.querySelector('.main-menu') ? 'main-menu' : (document.querySelector('.settings-main') ? 'settings' : 'other'),
        windowErrors: window.__wii_errors || []
      };
    })()`,
    returnByValue: true
  });

  console.log("\n=== DOM STATE ===");
  console.log(JSON.stringify(domState.result?.result?.value, null, 2));

  console.log("\n=== CONSOLE MESSAGES ===");
  console.log(JSON.stringify(consoleMessages, null, 2));

  console.log("\n=== NETWORK FAILURES (4xx, 5xx, or failed) ===");
  console.log(JSON.stringify(networkFailures, null, 2));

  // Capture screenshot
  const screenshot = await send("Page.captureScreenshot", { format: "png" });
  if (screenshot.result?.data) {
    const buf = Buffer.from(screenshot.result.data, "base64");
    writeFileSync("C:/src/wii/wii-os/userland/build/chrome-live-debug.png", buf);
    console.log("\nScreenshot saved to C:/src/wii/wii-os/userland/build/chrome-live-debug.png");
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
    consoleMessages.push(data.params.message);
  }
  if (data.method === "Runtime.consoleAPICalled") {
    consoleMessages.push({
      type: data.params.type,
      args: data.params.args?.map(a => a.value || a.description)
    });
  }
  if (data.method === "Runtime.exceptionThrown") {
    consoleMessages.push({
      type: "exception",
      text: data.params.exceptionDetails?.text,
      exception: data.params.exceptionDetails?.exception?.description,
      stack: data.params.exceptionDetails?.stackTrace
    });
  }

  if (data.method === "Network.responseReceived") {
    const { status, url, statusText } = data.params.response;
    if (status >= 400) {
      networkFailures.push({ url, status, statusText });
    }
  }
  if (data.method === "Network.loadingFailed") {
    networkFailures.push({
      url: data.params.requestId,
      errorText: data.params.errorText,
      canceled: data.params.canceled
    });
  }
};
