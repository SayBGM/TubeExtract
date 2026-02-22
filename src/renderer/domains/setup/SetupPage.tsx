import { useNavigate } from "react-router-dom";
import { AnimatePresence } from "motion/react";
import { useSetupStore } from "../../store/setupStore";
import { useSetupActions } from "./useSetupActions";
import { SetupHeader } from "./components/SetupHeader";
import { SetupUrlForm } from "./components/SetupUrlForm";
import { SetupAnalyzingState } from "./components/SetupAnalyzingState";
import { SetupAnalysisResult } from "./components/SetupAnalysisResult";

export function SetupPage() {
  const navigate = useNavigate();
  const {
    urlInput,
    setUrlInput,
    isAnalyzing,
    analysisResult,
    selectedMode,
    selectedQualityId,
    setSelectedMode,
    setSelectedQualityId,
    analyzeError,
  } = useSetupStore();
  const { qualityOptions, onAnalyze, onEnqueue, onOpenVideoInBrowser } =
    useSetupActions(() => navigate("/queue"));

  return (
    <section className="max-w-4xl mx-auto pt-10">
      <SetupHeader />

      <SetupUrlForm
        urlInput={urlInput}
        isAnalyzing={isAnalyzing}
        analyzeError={analyzeError}
        onUrlChange={setUrlInput}
        onAnalyze={onAnalyze}
      />

      {isAnalyzing ? <SetupAnalyzingState /> : null}

      <AnimatePresence>
        {analysisResult ? (
          <SetupAnalysisResult
            analysisResult={analysisResult}
            selectedMode={selectedMode}
            selectedQualityId={selectedQualityId}
            qualityOptions={qualityOptions}
            onSelectMode={setSelectedMode}
            onSelectQuality={setSelectedQualityId}
            onOpenVideoInBrowser={onOpenVideoInBrowser}
            onEnqueue={() => onEnqueue(false)}
          />
        ) : null}
      </AnimatePresence>
    </section>
  );
}
