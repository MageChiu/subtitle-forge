import { useState } from "react";
import { FileDropZone } from "./components/FileDropZone";
import { ProgressPanel } from "./components/ProgressPanel";
import { SettingsDialog } from "./components/SettingsDialog";
import { LanguageSelector } from "./components/LanguageSelector";
import { SubtitlePreview } from "./components/SubtitlePreview";
import { usePipeline, PipelineConfig } from "./hooks/usePipeline";

function App() {
  const { stage, isRunning, start, cancel } = usePipeline();
  const [selectedFile, setSelectedFile] = useState<string | null>(null);
  const [sourceLang, setSourceLang] = useState("auto");
  const [targetLang, setTargetLang] = useState("zh");
  const [outputFormat, setOutputFormat] = useState<"Srt" | "Ass" | "Vtt">("Srt");
  const [showSettings, setShowSettings] = useState(false);

  const handleStart = async () => {
    if (!selectedFile) return;

    const config: PipelineConfig = {
      input_path: selectedFile,
      output_dir: selectedFile.substring(0, selectedFile.lastIndexOf("/")),
      source_language: sourceLang === "auto" ? null : sourceLang,
      target_language: targetLang,
      output_format: outputFormat,
      asr_model: "base", // Will be resolved by backend
      translate_engine: "llm",
      use_gpu: false,
    };

    try {
      await start(config);
    } catch (err) {
      console.error("Pipeline failed:", err);
    }
  };

  return (
    <div className="min-h-screen p-6 max-w-4xl mx-auto">
      {/* Header */}
      <header className="flex items-center justify-between mb-8">
        <div>
          <h1 className="text-2xl font-bold">SubtitleForge</h1>
          <p className="text-sm text-gray-500 dark:text-gray-400">
            Bilingual Subtitle Generator
          </p>
        </div>
        <button
          onClick={() => setShowSettings(true)}
          className="px-4 py-2 text-sm bg-gray-200 dark:bg-gray-700 rounded-lg hover:bg-gray-300 dark:hover:bg-gray-600 transition-colors"
        >
          Settings
        </button>
      </header>

      {/* File Drop Zone */}
      <section className="mb-6">
        <FileDropZone
          onFileSelect={setSelectedFile}
          selectedFile={selectedFile}
        />
      </section>

      {/* Language & Format Selection */}
      {selectedFile && (
        <section className="mb-6 grid grid-cols-3 gap-4">
          <LanguageSelector
            label="Source Language"
            value={sourceLang}
            onChange={setSourceLang}
            includeAuto
          />
          <LanguageSelector
            label="Target Language"
            value={targetLang}
            onChange={setTargetLang}
          />
          <div>
            <label className="block text-sm font-medium mb-1">
              Output Format
            </label>
            <select
              value={outputFormat}
              onChange={(e) => setOutputFormat(e.target.value as "Srt" | "Ass" | "Vtt")}
              className="w-full px-3 py-2 rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 text-sm"
            >
              <option value="Srt">SRT (Most Compatible)</option>
              <option value="Ass">ASS (Rich Styling)</option>
              <option value="Vtt">VTT (Web)</option>
            </select>
          </div>
        </section>
      )}

      {/* Action Buttons */}
      {selectedFile && (
        <section className="mb-6 flex gap-3">
          <button
            onClick={handleStart}
            disabled={isRunning}
            className="flex-1 px-6 py-3 bg-blue-600 text-white font-medium rounded-lg hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
          >
            {isRunning ? "Processing..." : "Generate Subtitles"}
          </button>
          {isRunning && (
            <button
              onClick={cancel}
              className="px-6 py-3 bg-red-600 text-white font-medium rounded-lg hover:bg-red-700 transition-colors"
            >
              Cancel
            </button>
          )}
        </section>
      )}

      {/* Progress Panel */}
      {stage.type !== "Idle" && (
        <section className="mb-6">
          <ProgressPanel stage={stage} />
        </section>
      )}

      {/* Subtitle Preview */}
      {stage.type === "Completed" && stage.output_path && (
        <section className="mb-6">
          <SubtitlePreview filePath={stage.output_path} />
        </section>
      )}

      {/* Settings Dialog */}
      {showSettings && (
        <SettingsDialog onClose={() => setShowSettings(false)} />
      )}
    </div>
  );
}

export default App;
