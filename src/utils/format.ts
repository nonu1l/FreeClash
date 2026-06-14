export function formatBytes(value: number) {
  if (!Number.isFinite(value)) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let size = value;
  let unit = 0;
  while (size >= 1024 && unit < units.length - 1) {
    size /= 1024;
    unit += 1;
  }
  return `${size >= 10 || unit === 0 ? size.toFixed(0) : size.toFixed(1)} ${units[unit]}`;
}

export function formatSpeed(value: number) {
  return `${formatBytes(value)}/s`;
}

export function nodeDelayLabel(delay: number | null) {
  return delay === null ? "未测速" : `${delay} ms`;
}

export function networkModeLabel(mode: string) {
  return mode === "Proxy" ? "代理节点" : "本地直连";
}

export function fileName(path: string) {
  return path.split(/[\\/]/).pop() || path;
}
