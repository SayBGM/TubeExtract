import { Bell } from "lucide-react";
import { useTranslation } from "react-i18next";

interface SettingsDiagnosticsSectionProps {
  diagnostics: string;
  isPending: boolean;
  onDiagnose: () => void;
}

export function SettingsDiagnosticsSection({
  diagnostics,
  isPending,
  onDiagnose,
}: SettingsDiagnosticsSectionProps) {
  const { t } = useTranslation();

  return (
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
          type="button"
          onClick={onDiagnose}
          disabled={isPending}
          className="bg-zinc-800 hover:bg-zinc-700 text-white px-4 py-2 rounded-xl font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {t("settings.runDiagnostics")}
        </button>
      </div>
    </div>
  );
}
