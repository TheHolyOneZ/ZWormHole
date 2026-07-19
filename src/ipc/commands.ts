

import { invoke } from "@tauri-apps/api/core";
import type { NearbyDevicePayload } from "../types/discovery";
import type { TrustedDevicePayload } from "../types/trusted";
import type { TransferJob } from "../types/transfer";
import type { ActivePeerPayload } from "../types/peers";
import type { AutoExtractMode, TransferSettingsPayload } from "../types/settings";
import type { SpeedTestOutcome } from "../types/speedtest";
import type { BenchHistoryEntry, BenchmarkOutcome } from "../types/benchmark";
import type { HistoryEntry } from "../types/history";
import type { PersistedJob } from "../types/resumable";
import type { LocalConnectionInfo } from "../types/localinfo";
import type { BlockedDevice } from "../types/blocked";
import type { DeviceProfilePayload, MyProfilePayload } from "../types/profile";
import type { WanStatus } from "../types/wan";
import type { FolderNode } from "../types/tree";

export function ping(message: string): Promise<void> {
  return invoke("ping", { message });
}

export function becomeVisible(): Promise<void> {
  return invoke("become_visible");
}

export function stopVisible(): Promise<void> {
  return invoke("stop_visible");
}

export function startBrowsing(): Promise<void> {
  return invoke("start_browsing");
}

export function stopBrowsing(): Promise<void> {
  return invoke("stop_browsing");
}

export function listNearbyDevices(): Promise<NearbyDevicePayload[]> {
  return invoke("list_nearby_devices");
}

export function getLocalDeviceName(): Promise<string> {
  return invoke("get_local_device_name");
}

export function getLocalConnectionInfo(): Promise<LocalConnectionInfo> {
  return invoke("get_local_connection_info");
}


export function enableInternetReachable(): Promise<WanStatus> {
  return invoke("enable_internet_reachable");
}

export function disableInternetReachable(): Promise<void> {
  return invoke("disable_internet_reachable");
}

export function getWanStatus(): Promise<WanStatus | null> {
  return invoke("get_wan_status");
}

export function listTrustedDevices(): Promise<TrustedDevicePayload[]> {
  return invoke("list_trusted_devices");
}

export function revokeTrustedDevice(fingerprint: string): Promise<void> {
  return invoke("revoke_trusted_device", { fingerprint });
}


export function blockDevice(fingerprint: string, displayName: string): Promise<void> {
  return invoke("block_device", { fingerprint, displayName });
}

export function unblockDevice(fingerprint: string): Promise<void> {
  return invoke("unblock_device", { fingerprint });
}

export function listBlockedDevices(): Promise<BlockedDevice[]> {
  return invoke("list_blocked_devices");
}


export function getMyProfile(): Promise<MyProfilePayload> {
  return invoke("get_my_profile");
}


export function setMyDisplayName(name: string | null): Promise<void> {
  return invoke("set_my_display_name", { name: name && name.trim() ? name : null });
}


export function setMyAvatar(path: string): Promise<void> {
  return invoke("set_my_avatar", { path });
}

export function clearMyAvatar(): Promise<void> {
  return invoke("clear_my_avatar");
}


export function listProfiles(): Promise<DeviceProfilePayload[]> {
  return invoke("list_profiles");
}


export function refreshAdvertisement(): Promise<void> {
  return invoke("refresh_advertisement");
}


export function setDeviceNickname(fingerprint: string, nickname: string): Promise<void> {
  return invoke("set_device_nickname", { fingerprint, nickname: nickname || null });
}

export function setDeviceFavorite(fingerprint: string, favorite: boolean): Promise<void> {
  return invoke("set_device_favorite", { fingerprint, favorite });
}

export function setDeviceAutoAccept(fingerprint: string, autoAccept: boolean): Promise<void> {
  return invoke("set_device_auto_accept", { fingerprint, autoAccept });
}

export function listActivePeers(): Promise<ActivePeerPayload[]> {
  return invoke("list_active_peers");
}


export function sendConnectRequest(
  fullname: string,
  expirySecs?: number | null,
): Promise<string | null> {
  return invoke("send_connect_request", { fullname, expirySecs: expirySecs ?? null });
}

export function confirmPairingCode(requestId: string, accept: boolean): Promise<void> {
  return invoke("confirm_pairing_code", { requestId, accept });
}


export function sendManualConnectRequest(
  address: string,
  expirySecs?: number | null,
): Promise<string | null> {
  return invoke("send_manual_connect_request", { address, expirySecs: expirySecs ?? null });
}

export function sendFiles(deviceKey: string, paths: string[]): Promise<TransferJob> {
  return invoke("send_files", { deviceKey, paths });
}


export function sendDropped(deviceKey: string, paths: string[]): Promise<void> {
  return invoke("send_dropped", { deviceKey, paths });
}


export function sendFolder(
  deviceKey: string,
  folderPath: string,
  selectedFiles?: string[] | null,
): Promise<TransferJob> {
  return invoke("send_folder", { deviceKey, folderPath, selectedFiles: selectedFiles ?? null });
}


export interface SkippedTarget {
  device_key: string;
  reason: string;
}


export interface MultiSendResult {
  jobs: TransferJob[];
  skipped: SkippedTarget[];
}


export function sendFilesMulti(deviceKeys: string[], paths: string[]): Promise<MultiSendResult> {
  return invoke("send_files_multi", { deviceKeys, paths });
}


export function sendFolderMulti(
  deviceKeys: string[],
  folderPath: string,
  selectedFiles?: string[] | null,
): Promise<MultiSendResult> {
  return invoke("send_folder_multi", {
    deviceKeys,
    folderPath,
    selectedFiles: selectedFiles ?? null,
  });
}

export function readFolderTree(path: string): Promise<FolderNode> {
  return invoke("read_folder_tree", { path });
}

export function sendText(deviceKey: string, text: string): Promise<void> {
  return invoke("send_text", { deviceKey, text });
}


export function sendClipboardImage(deviceKey: string): Promise<TransferJob> {
  return invoke("send_clipboard_image", { deviceKey });
}

export function listTransfers(): Promise<TransferJob[]> {
  return invoke("list_transfers");
}

export function listTransferHistory(): Promise<HistoryEntry[]> {
  return invoke("list_transfer_history");
}

export function clearTransferHistory(): Promise<void> {
  return invoke("clear_transfer_history");
}

export function cancelTransfer(jobId: string): Promise<void> {
  return invoke("cancel_transfer", { jobId });
}

export function pauseTransfer(jobId: string): Promise<void> {
  return invoke("pause_transfer", { jobId });
}

export function listResumableTransfers(): Promise<PersistedJob[]> {
  return invoke("list_resumable_transfers");
}


export function resumeDeviceTransfers(deviceKey: string): Promise<void> {
  return invoke("resume_device_transfers", { deviceKey });
}


export function dismissResumable(jobId: string): Promise<void> {
  return invoke("dismiss_resumable", { jobId });
}

export function resumeTransfer(jobId: string): Promise<void> {
  return invoke("resume_transfer", { jobId });
}


export function openExternal(url: string): Promise<void> {
  return invoke("open_external_url", { url });
}


export function openPath(path: string): Promise<void> {
  return invoke("open_path", { path });
}


export function revealPath(path: string): Promise<void> {
  return invoke("reveal_path", { path });
}


export function saveTextFile(path: string, contents: string): Promise<void> {
  return invoke("save_text_file", { path, contents });
}

export function getBackgroundListenerEnabled(): Promise<boolean> {
  return invoke("get_background_listener_enabled");
}

export function setBackgroundListenerEnabled(enabled: boolean): Promise<void> {
  return invoke("set_background_listener_enabled", { enabled });
}

export function getTransferSettings(): Promise<TransferSettingsPayload> {
  return invoke("get_transfer_settings");
}

export function setAutoAccept(enabled: boolean): Promise<void> {
  return invoke("set_auto_accept", { enabled });
}

export function setSaveDir(dir: string | null): Promise<void> {
  return invoke("set_save_dir", { dir });
}

export function setAutoExtract(mode: AutoExtractMode): Promise<void> {
  return invoke("set_auto_extract", { mode });
}


export function setMaxSendMbS(value: number): Promise<void> {
  return invoke("set_max_send_mb_s", { value });
}


export function setMaxConcurrentSends(value: number): Promise<void> {
  return invoke("set_max_concurrent_sends", { value });
}


export function setQuicPort(port: number): Promise<number> {
  return invoke("set_quic_port", { port });
}

export function runSpeedTest(deviceKey: string, sizeMb: number): Promise<SpeedTestOutcome> {
  return invoke("run_speed_test", { deviceKey, sizeMb });
}

export function runBenchmark(deviceKey: string, sizeMb: number): Promise<BenchmarkOutcome> {
  return invoke("run_benchmark", { deviceKey, sizeMb });
}

export function recordBenchmark(
  fingerprint: string,
  displayName: string,
  upAvgMbps: number,
  downAvgMbps: number,
): Promise<void> {
  return invoke("record_benchmark", { fingerprint, displayName, upAvgMbps, downAvgMbps });
}

export function listBenchmarkHistory(): Promise<BenchHistoryEntry[]> {
  return invoke("list_benchmark_history");
}

export function clearBenchmarkHistory(): Promise<void> {
  return invoke("clear_benchmark_history");
}

export function exportBenchmark(path: string, contents: string): Promise<void> {
  return invoke("export_benchmark", { path, contents });
}

export function respondToConnectionTest(
  requestId: string,
  accept: boolean,
  remember: boolean,
): Promise<void> {
  return invoke("respond_to_connection_test", { requestId, accept, remember });
}

export function respondToIncomingTransfer(
  requestId: string,
  accept: boolean,
  saveDir?: string,
  extractMode?: AutoExtractMode,
): Promise<void> {
  return invoke("respond_to_incoming_transfer", {
    requestId,
    accept,
    saveDir,
    extractMode: extractMode ?? null,
  });
}
