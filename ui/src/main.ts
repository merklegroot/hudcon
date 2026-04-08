import { invoke } from "@tauri-apps/api/core";
import {
  mountDotNetInstallUi,
  mountDotNetPathFixUi,
  renderTab,
  type DotNetInstallResult,
  type DotNetPathConfigureResult,
} from "./views";
import "./styles.css";

type TabId =
  | "cpu"
  | "machine"
  | "gpu"
  | "memory"
  | "disk"
  | "packages"
  | "path"
  | "dotnet"
  | "node";

const TABS: { id: TabId; label: string }[] = [
  { id: "cpu", label: "CPU" },
  { id: "machine", label: "Machine" },
  { id: "gpu", label: "GPU" },
  { id: "memory", label: "RAM" },
  { id: "disk", label: "Disk" },
  { id: "packages", label: "Packages" },
  { id: "path", label: "Path" },
  { id: "dotnet", label: ".NET" },
  { id: "node", label: "Node.js" },
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
    case "packages":
      return invoke("get_package_info");
    case "path":
      return invoke("get_path_info");
    case "dotnet":
      return invoke("get_dotnet_basic_info");
    case "node":
      return invoke("get_nodejs_basic_info");
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
      const panel = renderTab(active, data);
      content.append(panel);
      if (active === "dotnet") {
        mountDotNetPathFixUi(content.querySelector("#dotnet-path-fix-slot"), {
          configure: () => invoke<DotNetPathConfigureResult>("add_dotnet_to_path"),
          onOutcome: (r) => {
            if (!r.success) {
              status.textContent = "PATH update failed — see log below.";
              return;
            }
            status.textContent = r.path_configured
              ? "PATH update finished — click Refresh to reload .NET details."
              : "dotnet is already on your PATH.";
          },
        });
        mountDotNetInstallUi(content.querySelector("#dotnet-install-slot"), {
          install: (major) =>
            invoke<DotNetInstallResult>("install_dotnet_sdk", {
              majorVersion: major,
            }),
          onOutcome: (r) => {
            status.textContent = r.success
              ? r.path_configured
                ? "Install finished — PATH updated; click Refresh to reload .NET details."
                : "Install finished — click Refresh to verify dotnet on PATH."
              : "Install reported failure — see log below.";
          },
        });
      }
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
