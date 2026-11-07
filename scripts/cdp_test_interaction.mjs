// Chrome DevTools Protocol interaction test
const wsUrl = "ws://127.0.0.1:9222/devtools/page/6E90D4F519B1B9CECF5405E8B0DDE1E8";
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
  console.log("Connected to Chrome for interaction testing...");
  await send("Runtime.enable");

  // Click on the first channel (Chaîne Disque)
  const clickResult = await send("Runtime.evaluate", {
    expression: `(() => {
      const discChannel = document.querySelector('.channels .channel-icon:first-child .hover');
      if (discChannel) {
        discChannel.click();
        return { clicked: true, splashOpen: document.body.classList.contains('channel-splash') };
      }
      return { clicked: false };
    })()`,
    returnByValue: true
  });

  console.log("Click result:", JSON.stringify(clickResult.result?.result?.value, null, 2));

  // Wait 1 second
  await new Promise(r => setTimeout(r, 1000));

  // Check state
  const stateResult = await send("Runtime.evaluate", {
    expression: `(() => {
      return {
        bodyClasses: document.body.className,
        splashBarVisible: getComputedStyle(document.querySelector('.splash-bar')).display !== 'none',
        startBtnText: document.querySelector('.splash-bar .btn:not(.menu-btn)')?.innerText
      };
    })()`,
    returnByValue: true
  });

  console.log("State after click:", JSON.stringify(stateResult.result?.result?.value, null, 2));

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
};
