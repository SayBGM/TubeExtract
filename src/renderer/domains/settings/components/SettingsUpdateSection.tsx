import { RefreshCcw, ShieldCheck } from "lucide-react";
import { useTranslation } from "react-i18next";

interface SettingsUpdateSectionProps {
  updateMessage: string;
  isPending: boolean;
  onCheckUpdate: () => void;
}

export function SettingsUpdateSection({
  updateMessage,
  isPending,
  onCheckUpdate,
}: SettingsUpdateSectionProps) {
  const { t } = useTranslation();

  return (
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
          type="button"
          onClick={onCheckUpdate}
          disabled={isPending}
          className="bg-zinc-800 hover:bg-zinc-700 text-white px-4 py-2 rounded-xl font-medium transition-colors inline-flex items-center gap-2 disabled:opacity-50 disabled:cursor-not-allowed"
        >
          <RefreshCcw className="w-4 h-4" />
          {t("settings.update.check")}
        </button>
      </div>
    </div>
  );
}
