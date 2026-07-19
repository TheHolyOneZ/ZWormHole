import { useEffect } from "react";
import { BusyOverlay } from "../components/BusyOverlay";
import { DeviceFooter } from "../components/DeviceFooter";
import { DropZoneOverlay } from "../components/DropZoneOverlay";
import { IncomingTransferModal } from "../components/IncomingTransferModal";
import { NearbyPane } from "../components/NearbyPane";
import { PairingModal } from "../components/PairingModal";
import { SettingsButton } from "../components/SettingsButton";
import { SettingsPanel } from "../components/SettingsPanel";
import { BenchmarkModal } from "../components/BenchmarkModal";
import { ConnectionTestModal } from "../components/ConnectionTestModal";
import { Dashboard } from "../components/Dashboard";
import { FocusView } from "../components/FocusView";
import { FolderTreeModal } from "../components/FolderTreeModal";
import { ToastStack } from "../components/ToastStack";
import { TopBar } from "../components/TopBar";
import { TextReceivedModal } from "../components/TextReceivedModal";
import { ThisDeviceModal } from "../components/ThisDeviceModal";
import { TransferDetailModal } from "../components/TransferDetailModal";
import { TrustedPane } from "../components/TrustedPane";
import { useAppStore } from "../state/store";

export function MainScreen() {
  const initialize = useAppStore((s) => s.initialize);
  const viewMode = useAppStore((s) => s.viewMode);

  useEffect(() => {
    void initialize();
  }, [initialize]);


  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.key !== "Escape") return;
      const s = useAppStore.getState();
      if (s.folderPick) return s.setFolderPick(null);
      if (s.benchmarkTarget) return s.setBenchmarkTarget(null);
      if (s.thisDeviceOpen) return s.setThisDeviceOpen(false);
      if (s.incomingText) return s.clearIncomingText();
      if (s.selectedTransferId) return s.setSelectedTransfer(null);
      if (s.settingsOpen) return s.setSettingsOpen(false);
    }
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, []);


  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (!(e.ctrlKey || e.metaKey) || e.altKey) return;
      const s = useAppStore.getState();
      if (e.key === "1") {
        e.preventDefault();
        s.setViewMode("dashboard");
      } else if (e.key === "2") {
        e.preventDefault();
        s.setViewMode("focus");
      } else if (e.key === ",") {
        e.preventDefault();
        s.setSettingsOpen(true);
      }
    }
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, []);

  return (
    <div className="app-shell">
      <TopBar />
      <div className="app-shell__body">
        <aside className="sidebar">
          <NearbyPane />
          <TrustedPane />
          <div className="sidebar__foot">
            <DeviceFooter />
            <SettingsButton />
          </div>
        </aside>
        {viewMode === "focus" ? <FocusView /> : <Dashboard />}
      </div>
      <PairingModal />
      <IncomingTransferModal />
      <TransferDetailModal />
      <BenchmarkModal />
      <FolderTreeModal />
      <ConnectionTestModal />
      <TextReceivedModal />
      <ThisDeviceModal />
      <SettingsPanel />
      <ToastStack />
      <BusyOverlay />
      <DropZoneOverlay />
    </div>
  );
}
