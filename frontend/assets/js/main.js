// Shared by the inline handlers in the locally loaded menu templates.
function playSound(id) {
  document
    .getElementById(id)
    ?.play()
    .catch(() => {});
}
function hover() {
  playSound("hover");
}
function select() {
  playSound("select");
}
function zip() {
  playSound("zip");
  select();
  document.getElementById("bg-music")?.pause();
}
function back() {
  playSound("back");
}

const weekdays = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const today = new Date();
window.date = `${weekdays[today.getDay()]} ${today.getMonth() + 1}/${today.getDate()}`;

document.addEventListener("DOMContentLoaded", () => {
  let transitionTimer;
  document.body.addEventListener("click", (event) => {
    const target = event.target;
    if (!(target instanceof Element)) return;
    const channel = target.closest(".occupied .hover");
    if (channel) {
      if (window.wiiOsRustInteractionsReady) return;
      const bounds = channel.getBoundingClientRect();
      const origin = `${bounds.left + window.scrollX + bounds.width / 2}px ${bounds.top + window.scrollY + bounds.height / 2}px 0px`;
      const menu = document.querySelector(".main-menu");
      const splash = document.querySelector(".splash-screen");
      if (menu) {
        menu.style.transformOrigin = origin;
        menu.classList.add("channel-splash");
      }
      if (splash) {
        splash.style.transformOrigin = origin;
        splash.style.backgroundImage = `url(${JSON.stringify(channel.dataset.img || "")})`;
      }
      document.body.classList.add("channel-splash");
      clearTimeout(transitionTimer);
      transitionTimer = setTimeout(
        () => document.body.classList.remove("splash-switch"),
        900,
      );
      return;
    }
    if (target.closest(".menu-btn")) {
      if (window.wiiOsRustInteractionsReady) return;
      document.querySelector(".main-menu")?.classList.remove("channel-splash");
      document.body.classList.remove("channel-splash");
      document.body.classList.add("splash-switch");
      clearTimeout(transitionTimer);
      transitionTimer = setTimeout(
        () => document.body.classList.remove("splash-switch"),
        900,
      );
      return;
    }
    if (target.closest(".splash-bar .btn:not(.menu-btn)")) {
      // The shared menu adapter handles real catalog launches. The prototype
      // remains usable when the optional menu core has not been staged.
      if (!window.wiiOsMenuCoreReady) {
        select();
        clearTimeout(transitionTimer);
        transitionTimer = setTimeout(() => {
          window.location.href = "/games/budokai-tenkaichi-3";
        }, 350);
      }
      return;
    }
    target.closest(".screen-message")?.classList.add("hidden");
  });
});
