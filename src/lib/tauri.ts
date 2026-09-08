import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export interface ModSpec {
  kind: "github" | "zip";
  repo: string;
  branch: string;
  path: string;
  enabled: boolean;
}

export interface ModConfig {
  appName: string | null;
  currentLabel: string | null;
  version: string | null;
  include: string[];
  exclude: string[];
  includeAndroid: string[];
  excludeAndroid: string[];
  includeIos: string[];
  excludeIos: string[];
  platforms: { android: boolean; ios: boolean };
  compatibleMods: string[];
  incompatibleMods: string[];
  recommendedVersion: string | null;
  testedVersions: string[];
  incompatibleVersions: string[];
  aiContent: boolean;
  license: string | null;
}

export interface ModInfo {
  config: ModConfig;
  fileCount: number;
  stars: number | null;
  sourceUrl: string | null;
}

export function modInfoKey(m: ModSpec): string {
  return m.kind === "github" ? `gh:${m.repo}@${m.branch}` : `zip:${m.path}`;
}

export interface ProgressEvent {
  step: string;
  detail: string;
  done: number;
  total: number;
}

export interface ApksInfo {
  fileName: string;
  totalSize: number;
  baseApk: string | null;
  splits: { name: string; size: number; kind: string }[];
  singleApk: boolean;
}

export interface SingleApkAnalysis {
  fileName: string;
  size: number;
  packageGuess: string | null;
  entryCount: number;
  hasSplitRequirement: boolean;
  dexCount: number;
  verdict: string;
  detail: string;
}

export interface PatchedCheck {
  sampled: number;
  matched: number;
  coverage: number;
  verdict: "patched" | "clean" | "unknown" | string;
  detail: string;
}

export interface DrmbInfo {
  fileName: string;
  totalSize: number;
  baseFiles: number;
  splitFiles: number;
  smaliFiles: number;
  sample: string[];
}

export interface IpaInfo {
  fileName: string;
  totalSize: number;
  appDir: string;
  bundleId: string | null;
  bundleName: string | null;
  version: string | null;
  totalEntries: number;
  dataEntries: number;
}

export interface AndroidPatchResult {
  outputPath: string;
  totalOverrides: number;
  smaliFiles: number;
  usedApktool: boolean;
  signed: boolean;
  signSchemes: string[];
  warning: string | null;
  appName: string | null;
}

export interface DeviceCompat {
  devices: string[];
  apkAbis: string[];
  deviceAbis: string[];
  compatible: boolean | null;
  note: string;
}

export interface IosPatchResult {
  outputPath: string;
  appDir: string;
  bundleId: string | null;
  totalOverrides: number;
  stripped: number;
  warning: string | null;
  appName: string | null;
}

export interface ToolStatus {
  name: string;
  present: boolean;
  path: string | null;
  hint: string;
  canInstall: boolean;
}

export interface AppleAccount {
  id: string;
  label: string;
  appleId: string;
  appPassword: string;
  note: string;
  p12Password: string;
  hasIdentity: boolean;
}

export interface AppleAccountStore {
  accounts: AppleAccount[];
  activeId: string;
}

export interface IosSignResult {
  signedPath: string;
  accountId: string;
}

export const api = {
  inspectApks: (path: string) => invoke<ApksInfo>("inspect_apks", { path }),
  inspectSingleApk: (path: string) => invoke<SingleApkAnalysis>("inspect_single_apk", { path }),
  checkApkPatched: (apkPath: string, markers: string[]) =>
    invoke<PatchedCheck>("check_apk_patched", { apkPath, markers }),
  inspectDrmb: (path: string) => invoke<DrmbInfo>("inspect_drmb", { path }),
  inspectIpa: (path: string) => invoke<IpaInfo>("inspect_ipa", { path }),
  listGithubFiles: (owner: string, repo: string, branch: string) =>
    invoke<string[]>("list_github_files", { owner, repo, branch }),
  modInfo: (spec: ModSpec) => invoke<ModInfo>("mod_info", { spec }),
  patchAndroid: (req: {
    apksPath: string;
    drmbPath?: string | null;
    mods: ModSpec[];
    outputName?: string | null;
    forceSingleApk?: boolean;
    signMode?: string;
    signV1?: boolean;
    signV2?: boolean;
    signV3?: boolean;
    androidVersion?: string | null;
    appName?: string | null;
  }) => invoke<AndroidPatchResult>("patch_android", { req }),
  patchIos: (req: { ipaPath: string; mods: ModSpec[]; outputName?: string | null; appName?: string | null }) =>
    invoke<IosPatchResult>("patch_ios", { req }),
  toolStatus: () => invoke<ToolStatus[]>("tool_status"),
  downloadTool: (name: string) => invoke<string>("download_tool", { name }),
  importToolFile: (name: string, srcPath: string) =>
    invoke<string>("import_tool_file", { name, srcPath }),
  saveAppleAccount: (account: AppleAccount) => invoke<AppleAccountStore>("save_apple_account", { account }),
  listAppleAccounts: () => invoke<AppleAccountStore>("list_apple_accounts"),
  deleteAppleAccount: (id: string) => invoke<AppleAccountStore>("delete_apple_account", { id }),
  setActiveAppleAccount: (id: string) => invoke<AppleAccountStore>("set_active_apple_account", { id }),
  importSigningIdentity: (id: string, p12Src: string, provSrc: string, p12Password: string) =>
    invoke<AppleAccountStore>("import_signing_identity", { id, p12Src, provSrc, p12Password }),
  deleteSigningIdentity: (id: string) => invoke<AppleAccountStore>("delete_signing_identity", { id }),
  signIos: (ipaPath: string, accountId: string | null, outputName: string | null) =>
    invoke<IosSignResult>("sign_ios", { ipaPath, accountId: accountId ?? "", outputName }),
  adbDevices: () => invoke<{ adb: string | null; devices: string[]; raw: string }>("adb_devices"),
  androidInstall: (apkPath: string) => invoke<string>("android_install", { apkPath }),
  checkDeviceCompat: (apkPath: string) =>
    invoke<DeviceCompat>("check_device_compat", { apkPath }),
  iosInstallAttempt: (ipaPath: string) =>
    invoke<{ ok: boolean; tool: string | null; message: string }>("ios_install_attempt", { ipaPath }),
  exportFile: (src: string, dest: string) => invoke<string>("export_file", { src, dest }),
  showInFolder: (path: string) => invoke<void>("show_in_folder", { path }),
  appDirs: () =>
    invoke<{ output: string; data: string; locales: string; account: string }>("app_dirs"),
  extraLocales: () => invoke<{ code: string; json: string }[]>("extra_locales"),
  seedLocales: (files: { code: string; json: string }[], version: number) =>
    invoke<number>("seed_locales", { files, version }),
  loadUserSettings: () => invoke<Record<string, unknown> | null>("load_user_settings"),
  saveUserSettings: (json: string) => invoke<void>("save_user_settings", { json }),
  getOutputDir: (platform: "android" | "ios") => invoke<string>("get_output_dir", { platform }),
  setOutputDir: (platform: "android" | "ios", path: string | null) =>
    invoke<string>("set_output_dir", { platform, path }),
  readTextFile: (path: string) => invoke<string>("read_text_file", { path }),
  writeTextFile: (path: string, text: string) => invoke<void>("write_text_file", { path, text }),
  setAppLogo: (srcPath: string) => invoke<string>("set_app_logo", { srcPath }),
  clearAppLogo: () => invoke<void>("clear_app_logo"),
  appLogoPath: () => invoke<string | null>("app_logo_path"),
};

export function humanizeError(t: (key: string) => string, err: unknown): string {
  const s = String(err);
  if (s.includes("DRMB_REQUIRED:")) return t("errors.drmbRequired");
  if (s.includes("SINGLE_APK_CHECK:")) return t("errors.singleApk");
  if (s.includes("APKTOOL_MISSING:")) return t("errors.apktoolMissing");
  return s;
}

export function onProgress(cb: (e: ProgressEvent) => void) {
  return listen<ProgressEvent>("patch-progress", (ev) => cb(ev.payload));
}

export function fmtBytes(n: number): string {
  if (!n && n !== 0) return "—";
  const u = ["B", "KB", "MB", "GB"];
  let v = n;
  let i = 0;
  while (v >= 1024 && i < u.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v.toFixed(v >= 100 ? 0 : 1)} ${u[i]}`;
}

export function stepLabel(t: (key: string) => string, s: string): string {
  const v = t(`steps.${s}`);
  return v === `steps.${s}` ? s : v;
}
