import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { FileDropZone } from "./components/FileDropZone";
import { LogPanel } from "./components/LogPanel";
import { ProgressPanel } from "./components/ProgressPanel";
import { SettingsDialog } from "./components/SettingsDialog";
import { LanguageSelector } from "./components/LanguageSelector";
import { SubtitlePreview } from "./components/SubtitlePreview";
import { usePipeline, PipelineConfig } from "./hooks/usePipeline";
import { useLogs } from "./hooks/useLogs";
import type { AppConfig } from "./types/appConfig";
import type { TranslationSettings } from "./types/translation";

interface ModelInfo {
  key: string;
  name: string;
  size_mb: number;
  description: string;
  path: string;
  downloaded: boolean;
  download_url: string;
}

function App() {
  const { stage, isRunning, start, cancel } = usePipeline();
  const { logs, isOpen: isLogOpen, toggleOpen: toggleLog, clearLogs } = useLogs();
  const [selectedFile, setSelectedFile] = useState<string | null>(null);
  const [sourceLang, setSourceLang] = useState("auto");
  const [targetLang, setTargetLang] = useState("zh");
  const [outputFormat, setOutputFormat] = useState<"Srt" | "Ass" | "Vtt">("Srt");
  const [asrModel, setAsrModel] = useState("base");
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [skipTranslation, setSkipTranslation] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [modelError, setModelError] = useState<string | null>(null);
  const [activeService, setActiveService] = useState("deepseek");
  const [appConfig, setAppConfig] = useState<AppConfig | null>(null);

  const loadModels = async () => {
    try {
      const list = await invoke<ModelInfo[]>("list_models");
      setModels(list);
    } catch (err) {
      console.error("Failed to load models:", err);
    }
  };

  const loadTranslateSettings = useCallback(async () => {
    try {
      const settings = await invoke<TranslationSettings>("get_translate_settings");
      setActiveService(settings.active_service);
    } catch (err) {
      console.error("Failed to load translate settings:", err);
    }
  }, []);

  const loadAppConfig = useCallback(async () => {
    try {
      const config = await invoke<AppConfig>("get_app_config");
      setAppConfig(config);
    } catch (err) {
      console.error("Failed to load app config:", err);
    }
  }, []);

  useEffect(() => {
    loadModels();
    loadTranslateSettings();
    loadAppConfig();
  }, [loadAppConfig, loadTranslateSettings]);

  const selectedModelInfo = models.find((m) => m.key === asrModel);
  const modelNotDownloaded = selectedModelInfo && !selectedModelInfo.downloaded;

  const handleStart = async () => {
    if (!selectedFile) return;

    if (modelNotDownloaded) {
      setModelError(
        `Whisper model "${selectedModelInfo.name}" has not been downloaded yet. Please download it in Settings.`
      );
      return;
    }

    setModelError(null);

    const config: PipelineConfig = {
      input_path: selectedFile,
      output_dir: selectedFile.substring(0, selectedFile.lastIndexOf("/")),
      source_language: sourceLang === "auto" ? null : sourceLang,
      target_language: targetLang,
      output_format: outputFormat,
      asr_model: asrModel,
      translate_engine: activeService,
      use_gpu: appConfig?.general.use_gpu ?? false,
      n_threads: appConfig?.asr.n_threads ?? null,
      skip_translation: skipTranslation,
    };

    try {
      await start(config);
    } catch (err) {
      const errMsg = String(err);
      if (errMsg.includes("not found") || errMsg.includes("download")) {
        setModelError(errMsg);
      }
    }
  };

  const errorCount = logs.filter((l) => l.level === "ERROR").length;
  const warnCount = logs.filter((l) => l.level === "WARN").length;

  return (
    <div className="min-h-screen p-6 max-w-4xl mx-auto">
      <header className="flex items-center justify-between mb-8">
        <div>
          <h1 className="text-2xl font-bold">SubtitleForge</h1>
          <p className="text-sm text-gray-500 dark:text-gray-400">
            Bilingual Subtitle Generator
          </p>
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={toggleLog}
            className={`relative px-4 py-2 text-sm rounded-lg transition-colors ${
              isLogOpen
                ? "bg-blue-600 text-white"
                : "bg-gray-200 dark:bg-gray-700 hover:bg-gray-300 dark:hover:bg-gray-600"
            }`}
          >
            📋 Log
            {errorCount > 0 && (
              <span className="absolute -top-1 -right-1 bg-red-500 text-white text-[10px] font-bold rounded-full w-4 h-4 flex items-center justify-center">
                {errorCount > 9 ? "9+" : errorCount}
              </span>
            )}
            {!isLogOpen && warnCount > 0 && errorCount === 0 && (
              <span className="absolute -top-1 -right-1 bg-yellow-500 text-white text-[10px] font-bold rounded-full w-4 h-4 flex items-center justify-center">
                {warnCount > 9 ? "9+" : warnCount}
              </span>
            )}
          </button>
          <button
            onClick={() => setShowSettings(true)}
            className="px-4 py-2 text-sm bg-gray-200 dark:bg-gray-700 rounded-lg hover:bg-gray-300 dark:hover:bg-gray-600 transition-colors"
          >
            ⚙ Settings
          </button>
        </div>
      </header>

      <section className="mb-6">
        <FileDropZone
          onFileSelect={setSelectedFile}
          selectedFile={selectedFile}
        />
      </section>

      {selectedFile && (
        <section className="mb-6 grid grid-cols-2 gap-4">
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
        </section>
      )}

      {selectedFile && (
        <section className="mb-6">
          <div className="grid grid-cols-2 gap-4">
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
            <div>
              <label className="block text-sm font-medium mb-1">
                ASR Model
                {modelNotDownloaded && (
                  <span className="ml-2 text-amber-500 text-xs font-normal">
                    ⚠ Not downloaded
                  </span>
                )}
              </label>
              <select
                value={asrModel}
                onChange={(e) => {
                  setAsrModel(e.target.value);
                  setModelError(null);
                }}
                className="w-full px-3 py-2 rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 text-sm"
              >
                {models.map((model) => (
                  <option key={model.key} value={model.key}>
                    {model.name} {model.downloaded ? "✓" : "(not downloaded)"}
                  </option>
                ))}
              </select>
            </div>
          </div>
          <div className="mt-3 flex items-center gap-3">
            <label className="flex items-center gap-2 cursor-pointer">
              <input
                type="checkbox"
                checked={skipTranslation}
                onChange={(e) => setSkipTranslation(e.target.checked)}
                className="w-4 h-4 rounded border-gray-300 text-blue-600 focus:ring-blue-500"
              />
              <span className="text-sm font-medium">
                Source language only (skip translation)
              </span>
            </label>
            {skipTranslation && (
              <span className="text-xs text-gray-500 dark:text-gray-400">
                — Generate monolingual subtitle for debugging ASR
              </span>
            )}
          </div>
        </section>
      )}

      {modelError && (
        <section className="mb-6">
          <div className="p-4 bg-amber-500/10 border border-amber-500/30 rounded-xl">
            <div className="flex items-start gap-3">
              <span className="text-2xl">⚠️</span>
              <div className="flex-1">
                <h3 className="font-medium text-amber-600 dark:text-amber-400 mb-1">
                  Whisper Model Not Found
                </h3>
                <p className="text-sm text-gray-600 dark:text-gray-400 mb-3">
                  {modelError}
                </p>
                <button
                  onClick={() => {
                    setModelError(null);
                    setShowSettings(true);
                  }}
                  className="px-4 py-2 text-sm font-medium bg-amber-500 text-white rounded-lg hover:bg-amber-600 transition-colors"
                >
                  Go to Settings → Download Model
                </button>
              </div>
            </div>
          </div>
        </section>
      )}

      {selectedFile && (
        <section className="mb-6 flex gap-3">
          <button
            onClick={handleStart}
            disabled={isRunning || !!modelNotDownloaded}
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

      {stage.type !== "Idle" && (
        <section className="mb-6">
          <ProgressPanel stage={stage} />
        </section>
      )}

      {stage.type === "Completed" &&
        (stage.bilingual_output_path || stage.source_output_path || stage.output_path) && (
        <section className="mb-6">
          <SubtitlePreview
            filePath={
              stage.bilingual_output_path || stage.source_output_path || stage.output_path || ""
            }
          />
        </section>
      )}

      {showSettings && (
        <SettingsDialog
          onClose={() => {
            setShowSettings(false);
            loadAppConfig();
            loadTranslateSettings();
          }}
          onModelChange={() => {
            loadModels();
            setModelError(null);
            loadAppConfig();
            loadTranslateSettings();
          }}
        />
      )}

      {isLogOpen && (
        <LogPanel logs={logs} onClose={toggleLog} onClear={clearLogs} />
      )}
    </div>
  );
}

export default App;
