import { Loader2 } from "lucide-react";
import { useTranslation } from "react-i18next";

export function SetupAnalyzingState() {
  const { t } = useTranslation();

  return (
    <div className="bg-zinc-900 border border-zinc-800 rounded-2xl p-6 mt-4">
      <div className="flex items-center gap-3 text-zinc-300 mb-4">
        <Loader2 className="w-5 h-5 animate-spin text-blue-400" />
        <span className="font-medium">{t("setup.actions.analyzing")}</span>
      </div>
      <div className="space-y-3 animate-pulse">
        <div className="h-40 w-full rounded-xl bg-zinc-800" />
        <div className="h-5 w-2/3 rounded bg-zinc-800" />
        <div className="h-4 w-1/2 rounded bg-zinc-800" />
      </div>
    </div>
  );
}
