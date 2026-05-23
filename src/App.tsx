import { useState } from "react";
import { AppLayout } from "./components/layout/AppLayout";
import { VaultPanel } from "./components/vault/VaultPanel";
import { StorePanel } from "./components/store/StorePanel";
import { LibraryPanel } from "./components/library/LibraryPanel";
import { SettingsPanel } from "./components/settings/SettingsPanel";
import { DashboardPanel } from "./components/dashboard/DashboardPanel";
import { ToolsPanel } from "./components/tools/ToolsPanel";
import { AboutPanel } from "./components/about/AboutPanel";

function App() {
  const [activeTab, setActiveTab] = useState("dashboard");

  return (
    <AppLayout activeTab={activeTab} onTabChange={setActiveTab}>
      {activeTab === "dashboard" && <DashboardPanel />}
      {activeTab === "install" && <StorePanel />}
      {activeTab === "library" && <LibraryPanel />}
      {activeTab === "tools" && <ToolsPanel />}
      {activeTab === "vault" && <VaultPanel />}
      {activeTab === "settings" && <SettingsPanel />}
      {activeTab === "about" && <AboutPanel />}
    </AppLayout>
  );
}

export default App;
