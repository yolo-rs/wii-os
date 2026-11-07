import { WebGpuChannelLayer } from "./webgpu-channel-layer.js";

const sessionPromise = (async () => {
  const response = await fetch("../menu.catalog.json", { cache: "no-store" });
  if (!response.ok) throw new Error("Wii OS menu catalog is not staged");
  const catalog = await response.text();
  const {
    default: initWiiMenuCore,
    MenuSession,
    install_menu_interactions,
  } = await import("./menu-core/wii_menu_web.js");
  await initWiiMenuCore();
  install_menu_interactions();
  window.wiiOsRustInteractionsReady = true;
  return new MenuSession(catalog);
})();
// The core is optional and may reject before the menu view is ready.
// Observe that rejection immediately; activateCurrentView handles it below.
sessionPromise.catch(() => {});

let session;
let activeChannelId;
let channelCatalog = [];
let webGpuLayer;
let gamepadFrame = 0;
let activeGamepadIndex = -1;
let gamepadButtons = new Set();
const gamepadRepeatAt = new Map();

function resolveChannelIcon(path) {
  if (
    typeof path !== "string" ||
    !path.startsWith("assets/") ||
    path.includes("..") ||
    path.includes("\\") ||
    /^[a-z]+:/i.test(path)
  )
    return null;
  return new URL(path, document.baseURI).href;
}

function publishState(snapshotText) {
  const snapshot = JSON.parse(snapshotText);
  document.dispatchEvent(
    new CustomEvent("wii-os-menu-state", { detail: snapshot }),
  );
  return snapshot;
}

function applyFocus(snapshot) {
  renderCatalogPage(snapshot.page);
  webGpuLayer?.setSelection(snapshot.selectedSlot);
  const slots = document.querySelectorAll(".channels .channel-icon .hover");
  slots.forEach((slot, index) => {
    slot.tabIndex = index === snapshot.selectedSlot ? 0 : -1;
    slot.classList.toggle(
      "wii-os-core-selected",
      index === snapshot.selectedSlot,
    );
    if (index === snapshot.selectedSlot) slot.focus({ preventScroll: true });
  });
  const pageCount = document.querySelector(".page-count");
  if (pageCount)
    pageCount.textContent = `${snapshot.page + 1} / ${snapshot.pageCount}`;
}

function renderCatalogPage(page) {
  if (!channelCatalog.length) return;
  const cells = document.querySelectorAll(".channels .channel-icon");
  const pageChannels = channelCatalog.slice(page * 12, page * 12 + 12);
  cells.forEach((cell, index) => {
    const channel = pageChannels[index];
    const image = cell.querySelector("img");
    const target = cell.querySelector(".hover");
    if (!channel) {
      cell.classList.remove("occupied");
      cell.classList.add("blank");
      image?.remove();
      if (target) {
        target.removeAttribute("id");
        target.removeAttribute("title");
        target.removeAttribute("aria-label");
        delete target.dataset.channelId;
        delete target.dataset.img;
      }
      return;
    }
    cell.classList.remove("blank");
    cell.classList.add("occupied");
    const icon =
      resolveChannelIcon(channel.icon) ??
      new URL("assets/images/channel-icon.png", document.baseURI).href;
    if (icon) {
      const channelImage = image ?? document.createElement("img");
      channelImage.src = icon;
      channelImage.alt = channel.title;
      channelImage.onerror = () => {
        channelImage.onerror = null;
        channelImage.src = new URL(
          "assets/images/channel-icon.png",
          document.baseURI,
        ).href;
      };
      if (!image) cell.prepend(channelImage);
      if (target) {
        target.dataset.img =
          channel.id.includes("disc") || channel.id.includes("budokai")
            ? "assets/images/bt3-title.png"
            : channel.icon;
      }
    } else {
      image?.remove();
      if (target) delete target.dataset.img;
    }
    if (target) {
      target.id = channel.id;
      target.dataset.channelId = channel.id;
      target.title = channel.title;
      target.setAttribute("aria-label", channel.title);
      target.setAttribute("role", "button");
    }
  });
  webGpuLayer?.setPage(pageChannels.filter(Boolean));
}

function moveSelection(dx, dy) {
  if (!session || !document.querySelector(".main-menu")) return;
  const snapshot = publishState(
    session.dispatch(JSON.stringify({ action: "move-selection", dx, dy })),
  );
  applyFocus(snapshot);
}

function pressPrimary() {
  if (document.body.classList.contains("channel-splash")) {
    document.querySelector(".splash-buttons .btn:nth-child(2)")?.click();
    return;
  }
  if (!document.querySelector(".main-menu")) {
    const focused = document.activeElement;
    if (focused?.matches?.("[role=button], button, a, .viewchange")) {
      focused.click();
      return;
    }
    document.querySelector(".viewchange[tabindex='0']")?.focus();
    return;
  }
  document
    .querySelector(".channels .channel-icon .wii-os-core-selected")
    ?.click();
}

function pressBack() {
  if (document.body.classList.contains("channel-splash")) {
    document.querySelector(".menu-btn")?.click();
    return;
  }
  document.querySelector(".settings-footer .back, .mail-back")?.click();
}

function focusViewControl(dx, dy) {
  const controls = [
    ...document.querySelectorAll(
      ".viewchange[tabindex='0'], [role=button][tabindex='0'], button:not(:disabled), a[href]",
    ),
  ].filter((element) => element.getClientRects().length > 0);
  if (!controls.length) return;
  const active = document.activeElement;
  const current = controls.includes(active) ? active : controls[0];
  if (current !== active) current.focus({ preventScroll: true });
  const from = current.getBoundingClientRect();
  const centerX = from.left + from.width / 2;
  const centerY = from.top + from.height / 2;
  const candidates = controls
    .filter((element) => element !== current)
    .map((element) => {
      const rect = element.getBoundingClientRect();
      const deltaX = rect.left + rect.width / 2 - centerX;
      const deltaY = rect.top + rect.height / 2 - centerY;
      const primary = dx ? deltaX * dx : deltaY * dy;
      const cross = dx ? Math.abs(deltaY) : Math.abs(deltaX);
      return { element, primary, cross };
    })
    .filter((candidate) => candidate.primary > 1)
    .sort((a, b) => a.primary + a.cross * 1.5 - (b.primary + b.cross * 1.5));
  candidates[0]?.element.focus({ preventScroll: true });
}

function isPressed(gamepad, index) {
  const button = gamepad.buttons[index];
  return Boolean(button && (button.pressed || button.value > 0.5));
}

function repeatDirection(name, pressed, now) {
  if (!pressed) {
    gamepadRepeatAt.delete(name);
    return false;
  }
  const nextAt = gamepadRepeatAt.get(name);
  if (nextAt === undefined) {
    gamepadRepeatAt.set(name, now + 250);
    return true;
  }
  if (now < nextAt) return false;
  gamepadRepeatAt.set(name, now + 140);
  return true;
}

function stopGamepadPolling() {
  if (gamepadFrame) cancelAnimationFrame(gamepadFrame);
  gamepadFrame = 0;
  activeGamepadIndex = -1;
  gamepadButtons.clear();
  gamepadRepeatAt.clear();
}

function pollGamepads(now) {
  gamepadFrame = 0;
  const gamepad = [...(navigator.getGamepads?.() ?? [])].find(
    (candidate) => candidate?.connected && candidate.mapping === "standard",
  );
  if (!gamepad) {
    stopGamepadPolling();
    return;
  }
  if (activeGamepadIndex !== gamepad.index) {
    activeGamepadIndex = gamepad.index;
    gamepadButtons.clear();
    gamepadRepeatAt.clear();
  }

  const axisX = gamepad.axes[0] ?? 0;
  const axisY = gamepad.axes[1] ?? 0;
  const dpadLeft = isPressed(gamepad, 14);
  const dpadRight = isPressed(gamepad, 15);
  const dpadUp = isPressed(gamepad, 12);
  const dpadDown = isPressed(gamepad, 13);
  const directions = {
    left: dpadLeft || (!dpadRight && axisX < -0.55),
    right: dpadRight || (!dpadLeft && axisX > 0.55),
    up: dpadUp || (!dpadDown && axisY < -0.55),
    down: dpadDown || (!dpadUp && axisY > 0.55),
  };
  const inMenu = Boolean(document.querySelector(".main-menu"));
  for (const [name, direction] of Object.entries({
    left: { dx: -1, dy: 0 },
    right: { dx: 1, dy: 0 },
    up: { dx: 0, dy: -1 },
    down: { dx: 0, dy: 1 },
  })) {
    if (repeatDirection(name, directions[name], now)) {
      if (inMenu) moveSelection(direction.dx, direction.dy);
      else focusViewControl(direction.dx, direction.dy);
    }
  }

  const pressedButtons = new Set(
    [0, 1, 9].filter((index) => isPressed(gamepad, index)),
  );
  if (pressedButtons.has(0) && !gamepadButtons.has(0)) pressPrimary();
  if (pressedButtons.has(1) && !gamepadButtons.has(1)) pressBack();
  if (pressedButtons.has(9) && !gamepadButtons.has(9)) pressPrimary();
  gamepadButtons = pressedButtons;
  gamepadFrame = requestAnimationFrame(pollGamepads);
}

function startGamepadPolling() {
  if (!gamepadFrame) gamepadFrame = requestAnimationFrame(pollGamepads);
}

window.addEventListener("gamepadconnected", startGamepadPolling);
window.addEventListener("gamepaddisconnected", () => {
  if (
    !navigator
      .getGamepads?.()
      .some((gamepad) => gamepad?.connected && gamepad.mapping === "standard")
  ) {
    stopGamepadPolling();
  }
});
window.addEventListener("blur", stopGamepadPolling);
window.addEventListener("focus", startGamepadPolling);
startGamepadPolling();

async function activateCurrentView() {
  try {
    session = await sessionPromise;
    window.wiiOsMenuCoreReady = true;
    if (!document.querySelector(".main-menu")) return;
    const snapshot = publishState(session.snapshot());
    applyFocus(snapshot);
    channelCatalog = JSON.parse(
      await (await fetch("../menu.catalog.json", { cache: "no-store" })).text(),
    ).channels;
    renderCatalogPage(snapshot.page);
    webGpuLayer = new WebGpuChannelLayer(document.querySelector(".main-menu"));
    await webGpuLayer.start();
    webGpuLayer.setPage(
      channelCatalog.slice(snapshot.page * 12, snapshot.page * 12 + 12),
    );
    webGpuLayer.setSelection(snapshot.selectedSlot);
    const pageCount = document.querySelector(".page-count");
    if (pageCount)
      pageCount.textContent = `${snapshot.page + 1} / ${snapshot.pageCount}`;
  } catch (error) {
    // Keep the upstream visual prototype usable when opened without Wii OS build output.
    console.info(
      "Wii OS shared menu core is not available; using the prototype UI.",
      error,
    );
  }
}

document.addEventListener("wii-os-menu-view-ready", activateCurrentView);

document.addEventListener(
  "click",
  (event) => {
    const pageButton = event.target.closest?.(".left-button, .right-button");
    if (pageButton && session) {
      const delta = pageButton.classList.contains("left-button") ? -1 : 1;
      applyFocus(
        publishState(
          session.dispatch(JSON.stringify({ action: "change-page", delta })),
        ),
      );
    }
    if (event.target.closest?.(".menu-btn") && session) {
      publishState(session.dispatch(JSON.stringify({ action: "back" })));
      activeChannelId = undefined;
    }
    if (event.target.closest?.(".wii-button") && session) {
      publishState(
        session.dispatch(JSON.stringify({ action: "open-settings" })),
      );
    }
    if (event.target.closest?.(".mail-button") && session) {
      publishState(session.dispatch(JSON.stringify({ action: "open-mail" })));
    }
    if (event.target.closest?.(".settings-navbtn.info") && session) {
      publishState(session.dispatch(JSON.stringify({ action: "open-about" })));
    }
    if (event.target.closest?.(".settings-footer .back") && session) {
      publishState(session.dispatch(JSON.stringify({ action: "back" })));
    }
    const cell = event.target.closest?.(".channels .channel-icon .hover");
    if (cell && session) {
      const slot = [
        ...document.querySelectorAll(".channels .channel-icon .hover"),
      ].indexOf(cell);
      applyFocus(publishState(session.focus_and_select(slot)));
      activeChannelId = cell.dataset.channelId;
    }
    if (
      event.target.closest?.(".splash-buttons .btn:nth-child(2)") &&
      session
    ) {
      const state = publishState(
        session.dispatch(JSON.stringify({ action: "start" })),
      );
      const channel =
        state.effect?.effect === "launch-channel"
          ? state.effect.channel_id
          : activeChannelId;
      const entry = channelCatalog.find(
        (candidate) => candidate.id === channel,
      );
      if (entry)
        document.dispatchEvent(
          new CustomEvent("wii-os-launch-request", {
            detail: { channelId: channel, appId: entry.appId },
          }),
        );
    }
  },
  true,
);

document.addEventListener("keydown", (event) => {
  if (!session || !document.querySelector(".main-menu")) return;
  const actions = {
    ArrowLeft: { dx: -1, dy: 0 },
    ArrowRight: { dx: 1, dy: 0 },
    ArrowUp: { dx: 0, dy: -1 },
    ArrowDown: { dx: 0, dy: 1 },
  };
  if (actions[event.key]) {
    event.preventDefault();
    moveSelection(actions[event.key].dx, actions[event.key].dy);
  } else if (
    (event.key === "Enter" || event.key === " ") &&
    document.activeElement?.matches(".channels .channel-icon .hover")
  ) {
    document.activeElement.click();
  } else if (
    (event.key === "Enter" || event.key === " ") &&
    document.activeElement?.matches(".left-button, .right-button")
  ) {
    event.preventDefault();
    document.activeElement.click();
  } else if (
    event.key === "Escape" &&
    document.body.classList.contains("channel-splash")
  ) {
    publishState(session.dispatch(JSON.stringify({ action: "back" })));
    document.querySelector(".menu-btn")?.click();
  }
});

document.addEventListener("wii-os-launch-request", async (event) => {
  if (!event.detail?.channelId && !event.detail?.appId) return;
  const channelId = event.detail.channelId || "";
  const entry = channelCatalog.find((candidate) => candidate.id === channelId);
  if (
    entry?.contentKind === "wii-disc" ||
    channelId.includes("disc") ||
    channelId.includes("budokai")
  ) {
    window.location.href = "/games/budokai-tenkaichi-3";
    return;
  }
  if (entry?.contentPath?.startsWith("/games/")) {
    window.location.href = entry.contentPath;
    return;
  }
  try {
    const response = await fetch(
      `/v1/apps/${encodeURIComponent(event.detail.appId)}/launch`,
      { method: "POST" },
    );
    const result = await response.json();
    document.dispatchEvent(
      new CustomEvent("wii-os-launch-result", {
        detail: { ...result, ok: response.ok },
      }),
    );
  } catch (error) {
    document.dispatchEvent(
      new CustomEvent("wii-os-launch-result", {
        detail: {
          ok: false,
          channelId: event.detail.channelId,
          error: String(error),
        },
      }),
    );
  }
});

const style = document.createElement("style");
style.textContent =
  ".channels .hover.wii-os-core-selected{outline:3px solid #54c5e5;outline-offset:2px}";
document.head.append(style);
