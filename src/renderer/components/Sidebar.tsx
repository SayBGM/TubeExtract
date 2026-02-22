import { NavLink } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { Disc, Download, ListVideo, Settings } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useQueueStore } from "../store/queueStore";
import { cn } from "../lib/cn";
import { getStorageStats } from "../lib/electronClient";
import type { StorageStats } from "../types";

export function Sidebar() {
  const { t } = useTranslation();
  const jobs = useQueueStore((state) => state.jobs);
  const activeCount = jobs.filter((job) => job.status === "downloading").length;
  const [storageStats, setStorageStats] = useState<StorageStats>();

  useEffect(() => {
    const loadStorageStats = async () => {
      const stats = await getStorageStats();
      setStorageStats(stats);
    };

    loadStorageStats().catch(console.error);
    const timer = window.setInterval(() => {
      loadStorageStats().catch(console.error);
    }, 10_000);

    return () => {
      window.clearInterval(timer);
    };
  }, []);

  const storageLabel = useMemo(() => {
    if (!storageStats) return t("sidebar.loading");
    return t("sidebar.storageUsedOf", {
      used: formatBytes(storageStats.usedBytes),
      total: formatBytes(storageStats.totalBytes),
    });
  }, [storageStats, t]);

  const downloadFolderLabel = useMemo(() => {
    if (!storageStats) return "-";
    return t("sidebar.downloads", { size: formatBytes(storageStats.downloadDirBytes) });
  }, [storageStats, t]);

  const navItems = [
    { label: t("sidebar.setup"), to: "/setup", icon: Download },
    { label: t("sidebar.queue"), to: "/queue", icon: ListVideo, badge: activeCount || undefined },
    { label: t("sidebar.settings"), to: "/settings", icon: Settings },
  ];

  return (
    <aside className="w-64 bg-zinc-950 border-r border-zinc-800 h-screen flex flex-col fixed left-0 top-0">
      <div className="p-6 flex items-center gap-3">
        <Disc className="w-8 h-8 text-red-600 animate-spin-slow" />
        <h1 className="text-xl font-bold tracking-tight text-white">TubeExtract</h1>
      </div>
      <nav className="flex-1 px-4 space-y-2 mt-4">
        {navItems.map((item) => (
          <NavLink
            key={item.to}
            to={item.to}
            className={({ isActive }) =>
              cn(
                "w-full flex items-center gap-3 px-4 py-3 rounded-xl transition-all duration-200 group relative text-left",
                isActive
                  ? "bg-zinc-800 text-white shadow-lg shadow-black/20"
                  : "text-zinc-400 hover:text-white hover:bg-zinc-800/50",
              )
            }
          >
            <item.icon className="w-5 h-5" />
            <span className="font-medium">{item.label}</span>
            {item.badge ? (
              <span className="absolute right-4 bg-red-600 text-white text-xs font-bold px-2 py-0.5 rounded-full">
                {item.badge}
              </span>
            ) : null}
          </NavLink>
        ))}
      </nav>
      <div className="p-6 border-t border-zinc-800">
        <div className="bg-zinc-900 rounded-xl p-4">
          <p className="text-xs text-zinc-500 font-medium uppercase mb-2">{t("sidebar.storage")}</p>
          <div className="w-full bg-zinc-800 rounded-full h-1.5 mb-2 overflow-hidden">
            <div
              className="bg-blue-500 h-full rounded-full transition-all duration-500"
              style={{ width: `${Math.max(0, Math.min(100, storageStats?.usedPercent ?? 0))}%` }}
            />
          </div>
          <p className="text-xs text-zinc-400">{storageLabel}</p>
          <p className="text-[11px] text-zinc-500 mt-1">{downloadFolderLabel}</p>
        </div>
      </div>
    </aside>
  );
}

function formatBytes(value: number) {
  if (!Number.isFinite(value) || value <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let size = value;
  let unitIndex = 0;
  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024;
    unitIndex += 1;
  }
  const decimals = unitIndex >= 3 ? 1 : 0;
  return `${size.toFixed(decimals)} ${units[unitIndex]}`;
}
