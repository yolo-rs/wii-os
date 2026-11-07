// View loader: local templates only, with native DOM APIs and no remote runtime dependency.
const transitionLength = 300;
let currentView = "default";
let previousView = "default";
let viewTimer;

async function waitForViewImages(root) {
  const urls = new Set();
  root.querySelectorAll("img").forEach((image) => {
    if (image.currentSrc || image.src) urls.add(image.currentSrc || image.src);
  });
  for (const element of [root, ...root.querySelectorAll("*")]) {
    const background = getComputedStyle(element).backgroundImage;
    for (const match of background.matchAll(/url\(["']?(.*?)["']?\)/g)) {
      if (match[1]) urls.add(new URL(match[1], document.baseURI).href);
    }
  }
  await Promise.allSettled(
    [...urls].map(
      (url) =>
        new Promise((resolve) => {
          const image = new Image();
          image.onload = resolve;
          image.onerror = resolve;
          image.src = url;
          if (image.complete) resolve();
        }),
    ),
  );
}

async function loadViewContents(view, transition) {
  if (!/^[a-z0-9-]+$/i.test(view)) throw new Error("Invalid local view name");
  const response = await fetch(`views/${encodeURIComponent(view)}.html`, {
    cache: "no-cache",
  });
  if (!response.ok) throw new Error(`Unable to load local view: ${view}`);
  const app = document.querySelector(".app");
  app.innerHTML = await response.text();
  // Scripts inserted through innerHTML are inert. Recreate the local view's
  // scripts so settings controls are initialized whenever the view opens.
  for (const inert of app.querySelectorAll("script")) {
    const script = document.createElement("script");
    for (const attribute of inert.attributes)
      script.setAttribute(attribute.name, attribute.value);
    script.textContent = inert.textContent;
    inert.replaceWith(script);
  }
  await waitForViewImages(app);

  if (view === "menu") {
    const dateElement = app.querySelector(".date");
    if (dateElement) {
      const label = document.createElement("span");
      label.textContent = ` ${window.date ?? ""}`;
      dateElement.replaceChildren(label);
    }
    document.dispatchEvent(new CustomEvent("wii-os-menu-view-ready"));
  }

  if (["settings-main", "licenses-temp", "about", "mail"].includes(view)) {
    window.setTimeout(() => {
      document
        .querySelectorAll(
          ".settings-navcontainer, .settings-header, .settings-footer",
        )
        .forEach((element) => element.classList.add("animate"));
    }, 300);
  }

  currentView = view;
  if (transition === "fade") {
    document.querySelector(".black")?.classList.remove("animate");
    if (view === "menu") {
      const music = document.getElementById("bg-music");
      music?.play().catch(() => {});
    }
    window.setTimeout(() => {
      const cover = document.querySelector(".black");
      if (cover) cover.style.top = "100vh";
    }, transitionLength);
  }
}

function changeView(view, transition = "fade") {
  previousView = currentView;
  if (transition === "fade") {
    const cover = document.querySelector(".black");
    cover?.classList.add("animate");
    if (cover) cover.style.top = "0";
    window.clearTimeout(viewTimer);
    viewTimer = window.setTimeout(() => {
      loadViewContents(view, transition).catch((error) => {
        document.dispatchEvent(
          new CustomEvent("wii-os-view-error", {
            detail: { message: error.message },
          }),
        );
        console.error(error);
      });
    }, transitionLength);
  } else {
    loadViewContents(view, transition).catch((error) => {
      document.dispatchEvent(
        new CustomEvent("wii-os-view-error", {
          detail: { message: error.message },
        }),
      );
      console.error(error);
    });
  }
}

document.addEventListener("DOMContentLoaded", () => {
  document.querySelector(".black")?.classList.remove("hidden");
  document.body.addEventListener("click", (event) => {
    const control = event.target.closest(".viewchange");
    if (!control) return;
    changeView(control.dataset.view, control.dataset.transition || "fade");
  });
  const boot = document.querySelector("#boot-sequence");
  const video = document.querySelector("#boot-video");
  let finished = false;
  const finish = () => {
    if (finished) return;
    finished = true;
    video?.pause();
    if (boot) boot.hidden = true;
    document
      .getElementById("startup")
      ?.play()
      .catch(() => {});
    changeView("menu", "fade");
  };
  document
    .querySelector("#skip-boot")
    ?.addEventListener("click", finish, { once: true });
  video?.addEventListener("ended", finish, { once: true });
  video?.addEventListener("error", finish, { once: true });
  (async () => {
    if (!boot || !video) return finish();
    // Authentic Nintendo Wii logo animation and sound on startup
    window.setTimeout(finish, 1200);
  })().catch(finish);
});
