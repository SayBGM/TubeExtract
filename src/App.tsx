import { Navigate, Route, Routes } from "react-router-dom";
import { Toaster } from "sonner";
import { Sidebar } from "./renderer/components/Sidebar";
import { SetupPage } from "./renderer/domains/setup/SetupPage";
import { QueuePage } from "./renderer/domains/queue/QueuePage";
import { SettingsPage } from "./renderer/domains/settings/SettingsPage";
import { useQueueEvents } from "./renderer/hooks/useQueueEvents";
import { useToastBridge } from "./renderer/hooks/useToastBridge";

function App() {
  useQueueEvents();
  useToastBridge();

  return (
    <div className="flex min-h-screen bg-zinc-950 text-white font-sans antialiased overflow-hidden">
      <Sidebar />
      <main className="flex-1 ml-64 overflow-y-auto h-screen bg-zinc-950 p-8">
        <Routes>
          <Route path="/setup" element={<SetupPage />} />
          <Route path="/queue" element={<QueuePage />} />
          <Route path="/settings" element={<SettingsPage />} />
          <Route path="*" element={<Navigate to="/setup" replace />} />
        </Routes>
        <Toaster position="bottom-right" theme="dark" />
      </main>
    </div>
  );
}

export default App;
