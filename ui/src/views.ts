/** Shapes mirror `hudcon` serde output (same as CLI data). */

export interface CpuFeatures {
  sse: boolean;
  sse2: boolean;
  sse3: boolean;
  ssse3: boolean;
  sse4_1: boolean;
  sse4_2: boolean;
  avx: boolean;
  avx2: boolean;
  avx512: boolean;
  fma: boolean;
  aes: boolean;
  sha: boolean;
  neon: boolean;
}

export interface LscpuInfo {
  vendor: string | null;
  model: string | null;
  cpu_cores: number | null;
  architecture: string | null;
  cpu_mhz: number | null;
  threads_per_core: number | null;
  cores_per_socket: number | null;
  sockets: number | null;
  virtualization: string | null;
  l1d_kb: number | null;
  l1i_kb: number | null;
  l2_kb: number | null;
  l3_kb: number | null;
  features: CpuFeatures;
}

export interface CpuSnapshot {
  lscpu: LscpuInfo | null;
  current_mhz: number;
  max_advertised_mhz: number | null;
  vendor: string | null;
  cpu_model: string | null;
  physical_cores: number | null;
  logical_cores: number;
}

export interface MachineInfo {
  os: string;
  virtualization: string;
  host_name: string;
  local_ip: string;
  machine_model: string;
  cpu_model: string;
  distro_flavor: string;
  kernel_version: string;
  motherboard: string;
}

export interface GpuCardDto {
  index: number;
  name: string;
  bus: string;
  revision: string;
  driver: string;
  memory_total: string | null;
  memory_used: string | null;
  memory_free: string | null;
  utilization: number | null;
  temperature: number | null;
  primary_display: boolean;
  opengl_active: boolean;
  active_for_display: string;
}

export interface GpuInfoDto {
  gpus: GpuCardDto[];
  opengl_renderer: string | null;
}

export interface TopProcess {
  pid: string;
  name: string;
  memory_usage: string;
  memory_percent: number;
  memory_absolute: string;
}

export interface MemoryInfo {
  total_ram: string;
  free_ram: string;
  used_ram: string;
  used_percent: number;
  top_processes: TopProcess[];
}

export interface DiskInfo {
  mount: string;
  total: string;
  used: string;
  available: string;
  used_percent: number;
  filesystem: string;
}

export interface PhysicalDisk {
  device: string;
  size: string;
  model: string;
  disk_type: string;
}

export interface DiskGatherResult {
  disks: DiskInfo[];
  physical_disks: PhysicalDisk[];
}

export interface PackageRepository {
  package_manager: string;
  repository: string;
}

export interface PackageInfo {
  package_manager: string;
  package_formats: string[];
  repositories: PackageRepository[];
}

export interface PathInfo {
  path: string;
  folders: string[];
}

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

function section(title: string): HTMLElement {
  return el("h2", { class: "section-title" }, [title]);
}

function rule(): HTMLElement {
  return el("div", { class: "section-rule" });
}

function kv(label: string, value: string | number | null | undefined): HTMLElement {
  const v =
    value === null || value === undefined || value === ""
      ? "n/a"
      : String(value);
  const row = el("div", { class: "kv-row" });
  row.append(el("span", { class: "kv-label" }, [label]), el("span", { class: "kv-value" }, [v]));
  return row;
}

function fmtMhz(mhz: number): string {
  if (!mhz) return "n/a";
  return `${mhz} MHz`;
}

function fmtMhzOpt(mhz: number | null | undefined): string {
  if (mhz == null || mhz <= 0) return "n/a";
  return `${mhz} MHz`;
}

function fmtOptU32(n: number | null | undefined): string {
  if (n == null) return "n/a";
  return String(n);
}

function fmtOptStr(s: string | null | undefined): string {
  if (s == null || s === "") return "n/a";
  return s;
}

function formatCacheKb(kb: number): string {
  if (kb < 1024) return `${kb} KB`;
  if (kb < 1024 * 1024) return `${(kb / 1024).toFixed(1)} MB`;
  return `${(kb / (1024 * 1024)).toFixed(1)} GB`;
}

function collectCpuFeatures(f: CpuFeatures): { sse: string[]; other: string[] } {
  const sse: string[] = [];
  if (f.sse) sse.push("SSE");
  if (f.sse2) sse.push("SSE2");
  if (f.sse3) sse.push("SSE3");
  if (f.ssse3) sse.push("SSSE3");
  if (f.sse4_1) sse.push("SSE4.1");
  if (f.sse4_2) sse.push("SSE4.2");
  const other: string[] = [];
  if (f.avx) other.push("AVX");
  if (f.avx2) other.push("AVX2");
  if (f.avx512) other.push("AVX512");
  if (f.fma) other.push("FMA");
  if (f.aes) other.push("AES");
  if (f.sha) other.push("SHA");
  if (f.neon) other.push("NEON");
  return { sse, other };
}

function wrap(...nodes: HTMLElement[]): HTMLElement {
  const d = el("div", { class: "view-root" });
  for (const n of nodes) d.append(n);
  return d;
}

export function renderCpu(data: CpuSnapshot): HTMLElement {
  const blocks: HTMLElement[] = [section("CPU"), rule()];

  if (data.lscpu) {
    const info = data.lscpu;
    blocks.push(
      kv("Vendor", fmtOptStr(info.vendor)),
      kv("CPU Model", fmtOptStr(info.model)),
      kv("CPU Cores", fmtOptU32(info.cpu_cores)),
      kv("Architecture", fmtOptStr(info.architecture)),
      kv("CPU Frequency", info.cpu_mhz != null ? `${info.cpu_mhz} MHz` : "n/a"),
      kv("Threads per Core", fmtOptU32(info.threads_per_core)),
      kv("Cores per Socket", fmtOptU32(info.cores_per_socket)),
      kv("Sockets", fmtOptU32(info.sockets)),
      kv("Virtualization", fmtOptStr(info.virtualization)),
      kv("L1d Cache", info.l1d_kb != null ? formatCacheKb(info.l1d_kb) : "n/a"),
      kv("L1i Cache", info.l1i_kb != null ? formatCacheKb(info.l1i_kb) : "n/a"),
      kv("L2 Cache", info.l2_kb != null ? formatCacheKb(info.l2_kb) : "n/a"),
      kv("L3 Cache", info.l3_kb != null ? formatCacheKb(info.l3_kb) : "n/a"),
      kv("Current MHz", fmtMhz(data.current_mhz)),
      kv("Max (advertised)", fmtMhzOpt(data.max_advertised_mhz))
    );
    const { sse, other } = collectCpuFeatures(info.features);
    if (sse.length || other.length) {
      blocks.push(section("CPU features"), rule());
      if (sse.length) blocks.push(kv("SSE family", sse.join(", ")));
      if (other.length) blocks.push(kv("AVX / other", other.join(", ")));
    }
    return wrap(...blocks);
  }

  if (data.vendor) blocks.push(kv("Vendor", data.vendor));
  if (data.cpu_model && data.cpu_model.length > 0) {
    blocks.push(kv("CPU Model", data.cpu_model));
  } else {
    blocks.push(kv("CPU Model", "(unavailable)"));
  }
  if (data.vendor || (data.cpu_model && data.cpu_model.length > 0)) {
    blocks.push(
      kv("Current MHz", fmtMhz(data.current_mhz)),
      kv("Max (advertised)", fmtMhzOpt(data.max_advertised_mhz))
    );
  }
  if (data.physical_cores != null) blocks.push(kv("Physical cores", data.physical_cores));
  blocks.push(kv("Logical cores", data.logical_cores));

  return wrap(...blocks);
}

export function renderMachine(data: MachineInfo): HTMLElement {
  return wrap(
    section("Machine"),
    rule(),
    kv("OS", data.os),
    kv("Virtualization", data.virtualization),
    section("System details"),
    rule(),
    kv("Machine Name", data.host_name),
    kv("Local IP Address", data.local_ip),
    kv("Machine Model", data.machine_model),
    kv("CPU Model", data.cpu_model),
    kv("Distro Flavor", data.distro_flavor),
    kv("Kernel Version", data.kernel_version),
    kv("Motherboard", data.motherboard)
  );
}

export function renderGpu(data: GpuInfoDto): HTMLElement {
  const blocks: HTMLElement[] = [section("Graphics"), rule()];

  if (data.opengl_renderer && data.opengl_renderer.length > 0) {
    blocks.push(kv("OpenGL Renderer", data.opengl_renderer));
  }

  const summaryParts: string[] = [];
  for (const g of data.gpus) {
    const tags: string[] = [];
    if (g.opengl_active) tags.push("OpenGL");
    if (g.primary_display) tags.push("boot VGA");
    if (tags.length) {
      const short =
        g.name.length > 48 ? `${g.name.slice(0, 46)}…` : g.name;
      summaryParts.push(`GPU ${g.index} (${short}): ${tags.join(" + ")}`);
    }
  }
  if (summaryParts.length) {
    blocks.push(kv("In use (summary)", summaryParts.join(" | ")));
  }

  if (!data.gpus.length) {
    blocks.push(kv("GPUs", "No GPU information available"));
    return wrap(...blocks);
  }

  for (const card of data.gpus) {
    blocks.push(
      section(`GPU ${card.index}: ${card.name}`),
      rule(),
      kv("Active for", card.active_for_display),
      kv("Driver", card.driver)
    );
    if (card.bus !== "Unknown" && card.bus !== "n/a") blocks.push(kv("Bus", card.bus));
    if (card.revision !== "Unknown" && card.revision !== "n/a") {
      blocks.push(kv("Revision", card.revision));
    }
    if (card.memory_total) blocks.push(kv("Memory Total", card.memory_total));
    if (card.memory_used) blocks.push(kv("Memory Used", card.memory_used));
    if (card.memory_free) blocks.push(kv("Memory Free", card.memory_free));
    if (card.utilization != null && card.utilization > 0) {
      blocks.push(kv("Utilization", `${card.utilization}%`));
    }
    if (card.temperature != null && card.temperature > 0) {
      blocks.push(kv("Temperature", `${card.temperature}°C`));
    }
  }

  return wrap(...blocks);
}

export function renderMemory(data: MemoryInfo): HTMLElement {
  const blocks: HTMLElement[] = [
    section("Memory usage"),
    rule(),
    kv("RAM Usage", `${data.used_percent}% used`),
    kv("Total RAM", data.total_ram),
    kv("Free RAM", data.free_ram),
    kv("Used RAM", data.used_ram),
    section("Top RAM consuming processes"),
    rule()
  ];

  if (!data.top_processes.length) {
    blocks.push(kv("Processes", "No process information available"));
    return wrap(...blocks);
  }

  data.top_processes.forEach((p, i) => {
    blocks.push(
      section(`${i + 1}. ${p.name}`),
      rule(),
      kv("PID", p.pid),
      kv("Memory", p.memory_absolute),
      kv("% of RAM", p.memory_usage)
    );
  });

  return wrap(...blocks);
}

export function renderPackages(data: PackageInfo): HTMLElement {
  const formats =
    data.package_formats.length === 1
      ? data.package_formats[0]
      : data.package_formats.join(", ");

  const blocks: HTMLElement[] = [
    section("Package information"),
    rule(),
    kv("Package Manager", data.package_manager || "Unknown"),
    kv("Package Formats", formats || "Unknown"),
    section("Package repositories"),
    rule(),
  ];

  if (!data.repositories.length) {
    blocks.push(kv("Repositories", "No repositories found"));
    return wrap(...blocks);
  }

  for (const repo of data.repositories) {
    const card = el("div", { class: "repo-card" });
    card.append(
      el("div", { class: "repo-manager" }, [repo.package_manager]),
      el("p", { class: "repo-uri" }, [repo.repository])
    );
    blocks.push(card);
  }

  return wrap(...blocks);
}

export function renderPath(data: PathInfo): HTMLElement {
  const intro = el("p", { class: "path-intro" }, [
    "PATH entries for this process (split, deduplicated, sorted like hudsse).",
  ]);

  const blocks: HTMLElement[] = [
    intro,
    section("PATH variable"),
    rule(),
  ];

  if (!data.path) {
    blocks.push(kv("PATH", "(empty or unset)"));
  } else {
    blocks.push(
      el("pre", { class: "path-raw" }, [data.path])
    );
  }

  blocks.push(
    section(`Path folders (${data.folders.length})`),
    rule()
  );

  if (!data.folders.length) {
    blocks.push(el("p", { class: "path-empty" }, ["No path folders found"]));
    return wrap(...blocks);
  }

  const list = el("div", { class: "path-folder-list" });
  data.folders.forEach((folder, i) => {
    const row = el("div", { class: "path-folder-row" });
    row.append(
      el("span", { class: "path-folder-idx" }, [String(i + 1)]),
      el("code", { class: "path-folder-code" }, [folder])
    );
    list.append(row);
  });
  blocks.push(list);

  return wrap(...blocks);
}

export function renderDisk(data: DiskGatherResult): HTMLElement {
  const blocks: HTMLElement[] = [section("Physical disks"), rule()];

  if (!data.physical_disks.length) {
    blocks.push(kv("Drives", "No physical disk information available"));
  } else {
    for (const pd of data.physical_disks) {
      blocks.push(section(pd.device), rule());
      if (pd.disk_type !== "Unknown") blocks.push(kv("Type", pd.disk_type));
      blocks.push(kv("Size", pd.size), kv("Model", pd.model));
    }
  }

  blocks.push(section("Disk usage (partitions)"), rule());

  if (!data.disks.length) {
    blocks.push(kv("Mounts", "No disk usage information available"));
  } else {
    for (const d of data.disks) {
      blocks.push(
        section(d.mount),
        rule(),
        kv("Filesystem", d.filesystem),
        kv("Usage", `${d.used_percent}% used`),
        kv("Total", d.total),
        kv("Used", d.used),
        kv("Available", d.available)
      );
    }
  }

  return wrap(...blocks);
}

export function renderTab(
  tab: "cpu" | "machine" | "gpu" | "memory" | "disk" | "packages" | "path",
  raw: unknown
): HTMLElement {
  const err = (msg: string) => el("p", { class: "error" }, [msg]);

  try {
    switch (tab) {
      case "cpu":
        return renderCpu(raw as CpuSnapshot);
      case "machine":
        return renderMachine(raw as MachineInfo);
      case "gpu":
        return renderGpu(raw as GpuInfoDto);
      case "memory":
        return renderMemory(raw as MemoryInfo);
      case "disk":
        return renderDisk(raw as DiskGatherResult);
      case "packages":
        return renderPackages(raw as PackageInfo);
      case "path":
        return renderPath(raw as PathInfo);
    }
  } catch {
    return err("Could not render this view.");
  }
}
