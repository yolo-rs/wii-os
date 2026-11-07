const vertexSource = /* wgsl */ `
struct VertexOut {
  @builtin(position) position: vec4f,
  @location(0) uv: vec2f,
};

@vertex fn vertexMain(@location(0) position: vec2f, @location(1) uv: vec2f) -> VertexOut {
  var out: VertexOut;
  out.position = vec4f(position, 0.0, 1.0);
  out.uv = uv;
  return out;
}

@group(0) @binding(0) var imageTexture: texture_2d<f32>;
@group(0) @binding(1) var imageSampler: sampler;
`;

const sceneShader = `${vertexSource} @fragment fn fragmentMain(input: VertexOut) -> @location(0) vec4f {
  return textureSample(imageTexture, imageSampler, input.uv);
}`;

const channelShader = `${vertexSource} @fragment fn fragmentMain(input: VertexOut) -> @location(0) vec4f {
  let p = input.uv;
  if ((p.y < 0.07 && (p.x < 0.02 || p.x > 0.98)) ||
      (p.y > 0.93 && (p.x < 0.02 || p.x > 0.98)) ||
      (p.y < 0.02 && (p.x < 0.06 || p.x > 0.95)) ||
      (p.y > 0.97 && (p.x < 0.05 || p.x > 0.95))) {
    discard;
  }
  return textureSample(imageTexture, imageSampler, p);
}`;

export class WebGpuChannelLayer {
  constructor(container) {
    this.container = container;
    this.channels = container.querySelector(".channels");
    this.canvas = document.createElement("canvas");
    this.canvas.className = "wii-channel-webgpu";
    this.canvas.setAttribute("aria-hidden", "true");
    this.items = [];
    this.textures = new Map();
    this.frame = 0;
    this.rendering = false;
    this.selection = 0;
    this.deviceLost = false;
    this.resizeObserver = new ResizeObserver(this.schedule);
    this.resizeObserver.observe(container);
    container.addEventListener("pointermove", this.schedule, { passive: true });
    container.addEventListener("pointerleave", this.schedule, {
      passive: true,
    });
  }

  async start() {
    if (!this.container || !this.channels) return false;
    if (!navigator.gpu) return false;
    try {
      const adapter = await navigator.gpu.requestAdapter();
      if (!adapter) throw new Error("No WebGPU adapter is available");
      this.device = await adapter.requestDevice();
      this.context = this.canvas.getContext("webgpu");
      if (!this.context)
        throw new Error("The browser did not provide a WebGPU canvas context");
      const format = navigator.gpu.getPreferredCanvasFormat();
      this.context.configure({
        device: this.device,
        format,
        usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_DST,
        alphaMode: "premultiplied",
      });
      this.textureLayout = this.device.createBindGroupLayout({
        label: "wii-menu-texture-layout",
        entries: [
          {
            binding: 0,
            visibility: GPUShaderStage.FRAGMENT,
            texture: {
              sampleType: "float",
              viewDimension: "2d",
              multisampled: false,
            },
          },
          {
            binding: 1,
            visibility: GPUShaderStage.FRAGMENT,
            sampler: { type: "filtering" },
          },
        ],
      });
      this.pipelineLayout = this.device.createPipelineLayout({
        label: "wii-menu-pipeline-layout",
        bindGroupLayouts: [this.textureLayout],
      });
      this.scenePipeline = this.createPipeline(
        sceneShader,
        format,
        "wii-menu-scene",
      );
      this.channelPipeline = this.createPipeline(
        channelShader,
        format,
        "wii-menu-channel",
      );
      this.sampler = this.device.createSampler({
        magFilter: "linear",
        minFilter: "linear",
      });
      this.canvas.width = 1;
      this.canvas.height = 1;
      this.container.prepend(this.canvas);
      this.container.classList.add("wii-webgpu-active");
      this.channels?.classList.add("wii-webgpu-active");
      this.device.lost.then(() => this.fallback());
      this.schedule();
      return true;
    } catch (error) {
      console.info(
        "Wii menu WebGPU renderer unavailable; retaining DOM artwork.",
        error,
      );
      this.fallback();
      return false;
    }
  }

  createPipeline(code, format, label) {
    const module = this.device.createShaderModule({
      label: `${label}-shader`,
      code,
    });
    return this.device.createRenderPipeline({
      label: `${label}-pipeline`,
      layout: this.pipelineLayout,
      vertex: {
        module,
        entryPoint: "vertexMain",
        buffers: [
          {
            arrayStride: 16,
            attributes: [
              { shaderLocation: 0, offset: 0, format: "float32x2" },
              { shaderLocation: 1, offset: 8, format: "float32x2" },
            ],
          },
        ],
      },
      fragment: {
        module,
        entryPoint: "fragmentMain",
        targets: [
          {
            format,
            blend: {
              color: {
                srcFactor: "src-alpha",
                dstFactor: "one-minus-src-alpha",
                operation: "add",
              },
              alpha: {
                srcFactor: "one",
                dstFactor: "one-minus-src-alpha",
                operation: "add",
              },
            },
          },
        ],
      },
      primitive: { topology: "triangle-list" },
    });
  }

  assetUrl(name) {
    return new URL(`assets/images/${name}`, document.baseURI).href;
  }

  setPage(channels) {
    this.items = channels.map((channel) => ({
      id: channel.id,
      url: new URL(
        typeof channel.icon === "string" &&
          channel.icon.startsWith("assets/") &&
          !channel.icon.includes("..") &&
          !channel.icon.includes("\\") &&
          !/^[a-z]+:/i.test(channel.icon)
          ? channel.icon
          : "assets/images/channel-icon.png",
        document.baseURI,
      ).href,
    }));
    if (this.animationTimer) window.clearInterval(this.animationTimer);
    if (this.channels?.querySelector(".channel-icon.blank")) {
      this.animationTimer = window.setInterval(this.schedule, 100);
    }
    this.schedule();
  }

  setSelection(slot) {
    this.selection = slot;
    this.schedule();
  }

  schedule = () => {
    if (!this.scenePipeline || this.frame || this.rendering || this.deviceLost)
      return;
    this.frame = requestAnimationFrame(() => {
      this.frame = 0;
      this.rendering = true;
      this.render()
        .catch((error) => {
          console.info(
            "Wii menu WebGPU frame failed; restoring DOM artwork.",
            error,
          );
          this.fallback();
        })
        .finally(() => {
          this.rendering = false;
        });
    });
  };

  async loadTexture(url) {
    if (this.textures.has(url)) return this.textures.get(url);
    const pending = (async () => {
      const response = await fetch(url);
      if (!response.ok)
        throw new Error(
          `Unable to load Wii menu artwork (${response.status}): ${url}`,
        );
      const bitmap = await createImageBitmap(await response.blob());
      const texture = this.device.createTexture({
        label: `wii-menu-art-${new URL(url).pathname.split("/").at(-1)}`,
        size: [bitmap.width, bitmap.height],
        format: "rgba8unorm",
        usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST | GPUTextureUsage.RENDER_ATTACHMENT,
      });
      this.device.queue.copyExternalImageToTexture(
        { source: bitmap },
        { texture },
        [bitmap.width, bitmap.height],
      );
      bitmap.close();
      const bindGroup = this.device.createBindGroup({
        layout: this.textureLayout,
        entries: [
          { binding: 0, resource: texture.createView() },
          { binding: 1, resource: this.sampler },
        ],
      });
      return { texture, bindGroup };
    })();
    this.textures.set(url, pending);
    try {
      const texture = await pending;
      this.textures.set(url, texture);
      return texture;
    } catch (error) {
      this.textures.delete(url);
      throw error;
    }
  }

  verticesFor(
    element,
    bounds,
    uv = [
      [0, 0],
      [1, 1],
    ],
  ) {
    const rect = element.getBoundingClientRect();
    if (!rect.width || !rect.height || !bounds.width || !bounds.height)
      return null;
    const x0 = ((rect.left - bounds.left) / bounds.width) * 2 - 1;
    const x1 = ((rect.right - bounds.left) / bounds.width) * 2 - 1;
    const y0 = 1 - ((rect.top - bounds.top) / bounds.height) * 2;
    const y1 = 1 - ((rect.bottom - bounds.top) / bounds.height) * 2;
    const [[u0, v0], [u1, v1]] = uv;
    return [
      x0,
      y0,
      u0,
      v0,
      x1,
      y0,
      u1,
      v0,
      x0,
      y1,
      u0,
      v1,
      x0,
      y1,
      u0,
      v1,
      x1,
      y0,
      u1,
      v0,
      x1,
      y1,
      u1,
      v1,
    ];
  }

  async render() {
    if (!this.context || this.container.classList.contains("channel-splash"))
      return;
    const bounds = this.container.getBoundingClientRect();
    if (!bounds.width || !bounds.height) return;
    const ratio = Math.min(window.devicePixelRatio || 1, 2);
    const width = Math.max(1, Math.round(bounds.width * ratio));
    const height = Math.max(1, Math.round(bounds.height * ratio));
    if (this.canvas.width !== width || this.canvas.height !== height) {
      this.canvas.width = width;
      this.canvas.height = height;
    }

    const draws = [];
    const add = async (url, element, { clipped = false, uv } = {}) => {
      if (!element) return;
      const vertices = this.verticesFor(element, bounds, uv);
      if (!vertices) return;
      draws.push({ texture: await this.loadTexture(url), clipped, vertices });
    };
    const mainMenu = this.container;
    await add(this.assetUrl("bg-pattern.png"), mainMenu);

    const bottom = mainMenu.querySelector(".bottom-section");
    await add(this.assetUrl("bottom-bg.png"), bottom);

    const cellElements = [...mainMenu.querySelectorAll(".channel-icon")];
    const channelsById = new Map(this.items.map((item) => [item.id, item]));
    const spriteFrame = Math.floor((performance.now() % 300) / 100);
    const spriteUrl = this.assetUrl("channel-spritesheet.png");
    const logoUrl = this.assetUrl("channel-wiilogo.png");
    const borderUrl = this.assetUrl("channel-border.png");
    const hoverUrl = this.assetUrl("channel-hover.png");

    for (const [slot, cell] of cellElements.entries()) {
      const target = cell.querySelector(".hover");
      const channel =
        target?.dataset.channelId && channelsById.get(target.dataset.channelId);
      if (channel && cell.classList.contains("occupied")) {
        await add(channel.url, cell, { clipped: true });
      } else {
        await add(spriteUrl, cell, {
          uv: [
            [spriteFrame / 3, 0],
            [(spriteFrame + 1) / 3, 1],
          ],
        });
        await add(logoUrl, cell);
      }
      if (target && (slot === this.selection || target.matches(":hover")))
        await add(hoverUrl, cell);
      await add(borderUrl, cell);
    }

    for (const [selector, image] of [
      [".bottom-title", "bottom-title.png"],
      [".left-button", "left-button.png"],
      [".right-button", "right-button.png"],
      [".wii-button", "wii-button.png"],
      [".mail-button", "mail-button.png"],
    ]) {
      await add(this.assetUrl(image), mainMenu.querySelector(selector));
    }

    const vertices = new Float32Array(draws.flatMap((draw) => draw.vertices));
    if (vertices.length) {
      this.vertexBuffer?.destroy();
      this.vertexBuffer = this.device.createBuffer({
        label: "wii-menu-scene-vertices",
        size: vertices.byteLength,
        usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
      });
      this.device.queue.writeBuffer(this.vertexBuffer, 0, vertices);
    }
    const encoder = this.device.createCommandEncoder({
      label: "wii-menu-scene-frame",
    });
    const pass = encoder.beginRenderPass({
      colorAttachments: [
        {
          view: this.context.getCurrentTexture().createView(),
          clearValue: { r: 0, g: 0, b: 0, a: 0 },
          loadOp: "clear",
          storeOp: "store",
        },
      ],
    });
    if (vertices.length) {
      pass.setVertexBuffer(0, this.vertexBuffer);
      let firstVertex = 0;
      for (const draw of draws) {
        pass.setPipeline(
          draw.clipped ? this.channelPipeline : this.scenePipeline,
        );
        pass.setBindGroup(0, draw.texture.bindGroup);
        pass.draw(6, 1, firstVertex);
        firstVertex += 6;
      }
    }
    pass.end();
    this.device.queue.submit([encoder.finish()]);
  }

  fallback() {
    if (this.deviceLost) return;
    this.deviceLost = true;
    if (this.animationTimer) window.clearInterval(this.animationTimer);
    this.container.classList.remove("wii-webgpu-active");
    this.channels?.classList.remove("wii-webgpu-active");
    this.canvas.remove();
    this.resizeObserver.disconnect();
    this.container.removeEventListener("pointermove", this.schedule);
    this.container.removeEventListener("pointerleave", this.schedule);
  }
}
