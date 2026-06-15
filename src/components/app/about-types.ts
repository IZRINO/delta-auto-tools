/** about 模块前端类型定义 */

export interface Dependency {
  name: string;
  kind: "frontend" | "runtime" | "build";
  license: string;
  url: string;
}

export interface AboutBootstrap {
  name: string;
  version: string;
  identifier: string;
  target: string;
  tauriVersion: string;
  license: string;
  licenseUrl: string;
  repositoryUrl: string;
  dependencies: Dependency[];
}

export interface UpdateInfo {
  available: boolean;
  version?: string;
  notes?: string;
  pubDate?: string;
}

export type UpdateProgress =
  | { phase: "checking" }
  | { phase: "notAvailable" }
  | { phase: "available"; version: string; notes?: string }
  | { phase: "downloading"; downloaded: number; total?: number }
  | { phase: "downloaded" }
  | { phase: "installing" }
  | { phase: "installed" }
  | { phase: "error"; message: string };
