import { Loader2, Search } from "lucide-react";
import { useTranslation } from "react-i18next";
import { cn } from "../../../lib/cn";

interface SetupUrlFormProps {
  urlInput: string;
  isAnalyzing: boolean;
  analyzeError?: string;
  onUrlChange: (value: string) => void;
  onAnalyze: () => Promise<void>;
}

export function SetupUrlForm({
  urlInput,
  isAnalyzing,
  analyzeError,
  onUrlChange,
  onAnalyze,
}: SetupUrlFormProps) {
  const { t } = useTranslation();

  return (
    <>
      <form
        onSubmit={(event) => {
          event.preventDefault();
          void onAnalyze();
        }}
        className="bg-zinc-900/50 border border-zinc-800 p-2 rounded-2xl flex items-center shadow-xl shadow-black/20 mb-2 relative z-10"
      >
        <div className="pl-4 pr-3 text-zinc-500">
          <Search className="w-6 h-6" />
        </div>
        <input
          data-testid="setup-url-input"
          type="text"
          value={urlInput}
          onChange={(e) => onUrlChange(e.target.value)}
          placeholder={t("setup.urlPlaceholder")}
          className="bg-transparent border-none text-white text-lg w-full focus:outline-none placeholder:text-zinc-600 py-3 flex-1"
        />
        <button
          data-testid="setup-analyze-button"
          type="submit"
          disabled={isAnalyzing || !urlInput.trim()}
          className={cn(
            "px-6 py-3 rounded-xl font-semibold transition-all duration-200",
            isAnalyzing || !urlInput.trim()
              ? "bg-zinc-800 text-zinc-500 cursor-not-allowed"
              : "bg-blue-600 hover:bg-blue-500 text-white shadow-lg shadow-blue-900/20",
          )}
        >
          {isAnalyzing ? (
            <Loader2 className="w-5 h-5 animate-spin" />
          ) : (
            t("setup.actions.analyze")
          )}
        </button>
      </form>
      {analyzeError ? (
        <p className="text-rose-400 text-sm mt-2">{analyzeError}</p>
      ) : null}
    </>
  );
}
