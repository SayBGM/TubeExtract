import { Download, FolderOpen } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Controller, useFormContext, useWatch } from "react-hook-form";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "../../../components/ui/select";
import type { AppSettings } from "../../../types";

interface SettingsDefaultsSectionProps {
  isLoading: boolean;
  onPickDownloadDir: () => Promise<void>;
}

export function SettingsDefaultsSection({
  isLoading,
  onPickDownloadDir,
}: SettingsDefaultsSectionProps) {
  const { t } = useTranslation();
  const { control, register } = useFormContext<AppSettings>();
  const downloadDir = useWatch({ control, name: "downloadDir" });

  return (
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
            <label
              htmlFor="settings-language"
              className="block text-sm font-medium text-zinc-400 mb-2"
            >
              {t("settings.language")}
            </label>
            <Controller
              name="language"
              control={control}
              render={({ field }) => (
                <Select value={field.value} onValueChange={field.onChange} disabled={isLoading}>
                  <SelectTrigger id="settings-language" data-testid="settings-language-select">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="ko">한국어</SelectItem>
                    <SelectItem value="en">English</SelectItem>
                  </SelectContent>
                </Select>
              )}
            />
          </div>
          <div>
            <label
              htmlFor="settings-max-retries"
              className="block text-sm font-medium text-zinc-400 mb-2"
            >
              {t("settings.maxRetries")}
            </label>
            <input
              id="settings-max-retries"
              className="w-full bg-zinc-950 border border-zinc-800 text-white rounded-xl px-4 py-3 focus:outline-none focus:ring-2 focus:ring-blue-500/50"
              type="number"
              min={0}
              max={10}
              disabled={isLoading}
              {...register("maxRetries", {
                setValueAs: (value: string) => Number(value) || 0,
              })}
            />
          </div>
        </div>
        <div>
          <label
            htmlFor="settings-download-dir"
            className="block text-sm font-medium text-zinc-400 mb-2"
          >
            {t("settings.downloadDir")}
          </label>
          <div className="flex gap-2">
            <input
              id="settings-download-dir"
              className="w-full bg-zinc-950 border border-zinc-800 text-zinc-300 rounded-xl px-4 py-3 focus:outline-none font-mono text-sm"
              disabled={isLoading}
              value={downloadDir}
              readOnly
              {...register("downloadDir")}
            />
            <button
              type="button"
              onClick={() => void onPickDownloadDir()}
              disabled={isLoading}
              className="shrink-0 bg-zinc-800 hover:bg-zinc-700 text-white rounded-xl px-4 py-3 inline-flex items-center gap-2 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            >
              <FolderOpen className="w-4 h-4" />
              {t("settings.pickFolder")}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
