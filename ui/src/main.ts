import { invoke } from "@tauri-apps/api/core";
import "./styles.css";

type TabId = "cpu" | "machine" | "gpu" | "memory" | "disk";

const TABS: { id: TabId; label: string }[] = [
  { id: "cpu", label: "CPU" },
  { id: "machine", label: "Machine" },
  { id: "gpu", label: "GPU" },
  { id: "memory", label: "RAM" },
  { id: "disk", label: "Disk" },
];

function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  props?: Partial<HTMLElementTagNameMap[K]> & { class?: string },
  children?: (Node | string)[]
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  if (props) {
    const { class: c, ...rest } = props as Record<string, unknown>;
    if (c) node.className = c as string;
    Object.assign(node, rest);
  }
  if (children) {
    for (const ch of children) {
      node.append(typeof ch === "string" ? document.createTextNode(ch) : ch);
    }
  }
  return node as HTMLElementTagNameMap[K];
}

function renderData(data: unknown): HTMLElement {
  const pre = el("pre", { class: "json" });
  pre.textContent = JSON.stringify(data, null, 2);
  return pre;
}

async function loadTab(tab: TabId): Promise<unknown> {
  switch (tab) {
    case "cpu":
      return invoke("get_cpu_info");
    case "machine":
      return invoke("get_machine_info");
    case "gpu":
      return invoke("get_gpu_info");
    case "memory":
      return invoke("get_memory_info");
    case "disk":
      return invoke("get_disk_info");
  }
}

function mount() {
  const root = document.getElementById("app");
  if (!root) return;

  const shell = el("div", { class: "shell" });
  const header = el("header", { class: "header" }, [
    el("h1", {}, ["HUDcon"]),
    el("p", { class: "tagline" }, ["Same Rust core as the console app."]),
  ]);

  const nav = el("nav", { class: "tabs" });
  const main = el("main", { class: "panel" });
  const status = el("p", { class: "status" }, [""]);
  const content = el("div", { class: "content" });

  main.append(status, content);

  let active: TabId = "cpu";

  async function refresh() {
    status.textContent = "Loading…";
    content.replaceChildren();
    try {
      const data = await loadTab(active);
      status.textContent = "";
      content.append(renderData(data));
    } catch (e) {
      status.textContent = `Error: ${e instanceof Error ? e.message : String(e)}`;
    }
  }

  for (const { id, label } of TABS) {
    const btn = el("button", {
      type: "button",
      class: "tab",
      onclick: () => {
        active = id;
        for (const b of nav.querySelectorAll("button.tab")) {
          b.classList.toggle("tab-active", b === btn);
        }
        void refresh();
      },
    });
    btn.textContent = label;
    if (id === active) btn.classList.add("tab-active");
    nav.append(btn);
  }

  const actions = el("div", { class: "actions" });
  actions.append(
    el(
      "button",
      {
        type: "button",
        class: "btn",
        onclick: () => void refresh(),
      },
      ["Refresh"]
    )
  );

  shell.append(header, nav, actions, main);
  root.append(shell);

  void refresh();
}

mount();
