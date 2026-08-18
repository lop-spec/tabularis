import { useCallback, useEffect, useMemo, useState } from "react";
import { BrowserRouter, Routes, Route, Navigate } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import { MainLayout } from "./components/layout/MainLayout";
import { ConnectionLayoutProvider } from "./contexts/ConnectionLayoutProvider";
import { RightSidebarProvider } from "./contexts/RightSidebarProvider";
import { KeybindingsProvider } from "./contexts/KeybindingsProvider";
import { AlertProvider } from "./contexts/AlertProvider";
import { Connections } from "./pages/Connections";
import { Editor } from "./pages/Editor";
import { Settings } from "./pages/Settings";
import { SchemaDiagramPage } from "./pages/SchemaDiagramPage";
import { TaskManagerPage } from "./pages/TaskManagerPage";
import { VisualExplainPage } from "./pages/VisualExplainPage";
import { JsonViewerPage } from "./pages/JsonViewerPage";
import { ResultsWindowPage } from "./pages/ResultsWindowPage";
import { RecoveryPage } from "./pages/RecoveryPage";
import { ConnectionHealthMonitor } from "./components/ConnectionHealthMonitor";
import { EditorErrorBoundary } from "./components/ui/EditorErrorBoundary";
import { UpdateNotificationModal } from "./components/modals/UpdateNotificationModal";
import { WhatsNewModal } from "./components/modals/WhatsNewModal";
import { SshAskpassGate } from "./components/modals/SshAskpassGate";
import { useUpdate } from "./hooks/useUpdate";
import { useChangelog } from "./hooks/useChangelog";
import { useResultTypeColors } from "./hooks/useResultTypeColors";
import { APP_VERSION } from "./version";
import { isVersionAtMost, isVersionNewer } from "./utils/versionCompare";

const WHATS_NEW_VERSION_KEY = "tabularis_last_seen_version";

export function App() {
  const {
    updateInfo,
    isDownloading,
    downloadProgress,
    downloadAndInstall,
    dismissUpdate,
    error: updateError,
  } = useUpdate();
  useResultTypeColors();
  const [isDebugMode, setIsDebugMode] = useState(false);

  const lastSeenVersion = localStorage.getItem(WHATS_NEW_VERSION_KEY);
  const [isWhatsNewOpen, setIsWhatsNewOpen] = useState(
    () => lastSeenVersion !== null && isVersionNewer(APP_VERSION, lastSeenVersion),
  );

  const { entries: allEntries, isLoading: isChangelogLoading } = useChangelog();

  const whatsNewEntries = useMemo(() => {
    if (!lastSeenVersion) return [];
    return allEntries.filter(
      (entry) =>
        isVersionNewer(entry.version, lastSeenVersion) &&
        isVersionAtMost(entry.version, APP_VERSION),
    );
  }, [lastSeenVersion, allEntries]);

  const dismissWhatsNew = useCallback(() => {
    localStorage.setItem(WHATS_NEW_VERSION_KEY, APP_VERSION);
    setIsWhatsNewOpen(false);
  }, []);

  useEffect(() => {
    invoke<boolean>("is_debug_mode").then((debugMode) => {
      setIsDebugMode(debugMode);
    });
  }, []);

  useEffect(() => {
    if (isDebugMode) return;

    const handleContextMenu = (e: MouseEvent) => {
      e.preventDefault();
    };

    document.addEventListener("contextmenu", handleContextMenu);

    return () => {
      document.removeEventListener("contextmenu", handleContextMenu);
    };
  }, [isDebugMode]);

  // Ctrl/Cmd+Q quits for real. In close-to-hide mode the window's X button
  // only hides the app (warm relaunch), so this is the supported way out.
  useEffect(() => {
    const handleQuit = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && !e.shiftKey && !e.altKey && e.key.toLowerCase() === "q") {
        e.preventDefault();
        void invoke("quit_app");
      }
    };
    document.addEventListener("keydown", handleQuit);
    return () => document.removeEventListener("keydown", handleQuit);
  }, []);

  return (
    <>
      <AlertProvider>
        <BrowserRouter>
          <ConnectionHealthMonitor />
          <KeybindingsProvider>
            <ConnectionLayoutProvider>
              <RightSidebarProvider>
                  <Routes>
                    <Route path="/" element={<MainLayout />}>
                      <Route
                        index
                        element={<Navigate to="/connections" replace />}
                      />
                      <Route path="connections" element={<Connections />} />
                      <Route
                        path="editor"
                        element={
                          <EditorErrorBoundary>
                            <Editor />
                          </EditorErrorBoundary>
                        }
                      />
                      <Route path="recovery" element={<RecoveryPage />} />
                      <Route path="settings" element={<Settings />} />
                    </Route>
                    <Route
                      path="/schema-diagram"
                      element={<SchemaDiagramPage />}
                    />
                    <Route path="/task-manager" element={<TaskManagerPage />} />
                    <Route path="/visual-explain" element={<VisualExplainPage />} />
                    <Route path="/json-viewer" element={<JsonViewerPage />} />
                    <Route
                      path="/results-window"
                      element={<ResultsWindowPage />}
                    />
                  </Routes>
              </RightSidebarProvider>
            </ConnectionLayoutProvider>
          </KeybindingsProvider>
        </BrowserRouter>
      </AlertProvider>

      <UpdateNotificationModal
        isOpen={!!updateInfo}
        onClose={dismissUpdate}
        updateInfo={updateInfo!}
        isDownloading={isDownloading}
        downloadProgress={downloadProgress}
        onDownloadAndInstall={downloadAndInstall}
        error={updateError}
      />

      <WhatsNewModal
        isOpen={isWhatsNewOpen}
        onClose={dismissWhatsNew}
        entries={whatsNewEntries}
        isLoading={isChangelogLoading}
      />

      <SshAskpassGate />
    </>
  );
}
