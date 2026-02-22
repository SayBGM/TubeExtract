import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Bell, Download, FolderOpen, RefreshCcw, Save, ShieldCheck } from "lucide-react";
import {
  checkUpdate,
  getSettings,
  pickDownloadDir,
  runDiagnostics,
  setSettings,
} from "../../lib/electronClient";
import { useSettingsStore } from "../../store/settingsStore";

export function SettingsPage() {
  const { t, i18n } = useTranslation();
  const settings = useSettingsStore((state) => state.settings);
  const updateSettings = useSettingsStore((state) => state.setSettings);
  const [diagnostics, setDiagnostics] = useState<string>("-");
  const [updateMessage, setUpdateMessage] = useState<string>("-");

  useEffect(() => {
    getSettings()
      .then(updateSettings)
      .catch(console.error);
  }, [updateSettings]);

  const onSave = async () => {
    await setSettings(settings);
    await i18n.changeLanguage(settings.language);
  };

  const onDiagnose = async () => {
    const result = await runDiagnostics();
    setDiagnostics(result.message);
  };

  const onCheckUpdate = async () => {
    const result = await checkUpdate();
    if (!result.hasUpdate) {
      setUpdateMessage(t("settings.update.latest"));
      return;
    }
    setUpdateMessage(
      t("settings.update.available", {
        version: result.latestVersion,
      }),
    );
    if (result.url) {
      window.open(result.url, "_blank");
    }
  };

  const onPickDownloadDir = async () => {
    const selected = await pickDownloadDir();
    if (!selected) return;
    updateSettings({ ...settings, downloadDir: selected });
  };

  return (
    <section className="max-w-4xl mx-auto pt-10">
      <h1 className="text-3xl font-bold text-white mb-8">{t("settings.title")}</h1>

      <div className="space-y-6">
        <div className="bg-zinc-900 border border-zinc-800 rounded-2xl overflow-hidden">
          <div className="p-6 border-b border-zinc-800">
            <h2 className="text-lg font-semibold text-white flex items-center gap-2">
              <Download className="w-5 h-5 text-blue-500" />
              {t("settings.downloadDefaults")}
            </h2>
          </div>
          <div className="p-6 space-y-6">
            <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
              <div>
                <label className="block text-sm font-medium text-zinc-400 mb-2">{t("settings.language")}</label>
                <select
                  data-testid="settings-language-select"
                  className="w-full bg-zinc-950 border border-zinc-800 text-white rounded-xl px-4 py-3 focus:outline-none focus:ring-2 focus:ring-blue-500/50"
                  value={settings.language}
                  onChange={(e) =>
                    updateSettings({ ...settings, language: e.target.value as "ko" | "en" })
                  }
                >
                  <option value="ko">한국어</option>
                  <option value="en">English</option>
                </select>
              </div>
              <div>
                <label className="block text-sm font-medium text-zinc-400 mb-2">{t("settings.maxRetries")}</label>
                <input
                  className="w-full bg-zinc-950 border border-zinc-800 text-white rounded-xl px-4 py-3 focus:outline-none focus:ring-2 focus:ring-blue-500/50"
                  type="number"
                  min={0}
                  max={10}
                  value={settings.maxRetries}
                  onChange={(e) =>
                    updateSettings({ ...settings, maxRetries: Number(e.target.value) || 0 })
                  }
                />
              </div>
            </div>
            <div>
              <label className="block text-sm font-medium text-zinc-400 mb-2">{t("settings.downloadDir")}</label>
              <div className="flex gap-2">
                <input
                  className="w-full bg-zinc-950 border border-zinc-800 text-zinc-300 rounded-xl px-4 py-3 focus:outline-none"
                  value={settings.downloadDir}
                  onChange={(e) => updateSettings({ ...settings, downloadDir: e.target.value })}
                />
                <button
                  type="button"
                  onClick={() => void onPickDownloadDir()}
                  className="shrink-0 bg-zinc-800 hover:bg-zinc-700 text-white rounded-xl px-4 py-3 inline-flex items-center gap-2 transition-colors"
                >
                  <FolderOpen className="w-4 h-4" />
                  {t("settings.pickFolder")}
                </button>
              </div>
            </div>
          </div>
        </div>

        <div className="bg-zinc-900 border border-zinc-800 rounded-2xl overflow-hidden">
          <div className="p-6 border-b border-zinc-800">
            <h2 className="text-lg font-semibold text-white flex items-center gap-2">
              <Bell className="w-5 h-5 text-yellow-500" />
              {t("settings.diagnostics")}
            </h2>
          </div>
          <div className="p-6 space-y-4">
            <p className="text-sm text-zinc-400">{diagnostics}</p>
            <button
              onClick={() => void onDiagnose()}
              className="bg-zinc-800 hover:bg-zinc-700 text-white px-4 py-2 rounded-xl font-medium transition-colors"
            >
              {t("settings.runDiagnostics")}
            </button>
          </div>
        </div>

        <div className="bg-zinc-900 border border-zinc-800 rounded-2xl overflow-hidden">
          <div className="p-6 border-b border-zinc-800">
            <h2 className="text-lg font-semibold text-white flex items-center gap-2">
              <ShieldCheck className="w-5 h-5 text-emerald-500" />
              {t("settings.update.title")}
            </h2>
          </div>
          <div className="p-6 space-y-4">
            <p className="text-sm text-zinc-400">{updateMessage}</p>
            <button
              onClick={() => void onCheckUpdate()}
              className="bg-zinc-800 hover:bg-zinc-700 text-white px-4 py-2 rounded-xl font-medium transition-colors inline-flex items-center gap-2"
            >
              <RefreshCcw className="w-4 h-4" />
              {t("settings.update.check")}
            </button>
          </div>
        </div>

        <div className="flex justify-end pt-4">
          <button
            data-testid="settings-save-button"
            className="bg-blue-600 hover:bg-blue-500 text-white px-8 py-3 rounded-xl font-bold shadow-lg shadow-blue-900/20 flex items-center gap-2"
            onClick={() => void onSave()}
          >
            <Save className="w-4 h-4" />
            {t("settings.save")}
          </button>
        </div>
      </div>
    </section>
  );
}
