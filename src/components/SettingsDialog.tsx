import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useMemo, useState } from "react";
import type { AppConfig } from "../types/appConfig";
import type {
  ConfigField,
  ServiceInfo,
  TranslateModeInfo,
  TranslateModeKey,
  TranslationSettings,
} from "../types/translation";

interface ModelInfo {
  key: string;
  name: string;
  size_mb: number;
  description: string;
  path: string;
  downloaded: boolean;
  download_url: string;
}

interface DownloadProgress {
  model_key: string;
  percent: number;
  downloaded_bytes: number;
  total_bytes: number;
}

interface EmbeddedModelInfo {
  key: string;
  name: string;
  size_mb: number;
  description: string;
  path: string;
  downloaded: boolean;
  download_url: string;
  model_id: string;
}

interface EmbeddedDownloadProgress {
  model_key: string;
  percent: number;
  downloaded_bytes: number;
  total_bytes: number;
}

interface AsrFeatureStatus {
  key: string;
  label: string;
  enabled: boolean;
  detail: string;
}

interface AsrRuntimeCapabilities {
  app_version: string;
  whisper_backend: string;
  gpu_backend_available: boolean;
  enabled_gpu_backends: string[];
  features: AsrFeatureStatus[];
}

interface SettingsDialogProps {
  onClose: () => void;
  onModelChange?: () => void;
}

function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const k = 1024;
  const sizes = ["B", "KB", "MB", "GB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + " " + sizes[i];
}

function getSelectOptions(field: ConfigField): Array<{ value: string; label: string }> {
  if (typeof field.field_type === "object" && "select" in field.field_type) {
    return field.field_type.select.options;
  }
  return [];
}

function renderConfigField(
  field: ConfigField,
  value: string,
  onChange: (key: string, value: string) => void,
) {
  const inputClass =
    "w-full px-3 py-2 rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 text-sm";

  if (typeof field.field_type === "object" && "select" in field.field_type) {
    return (
      <div key={field.key}>
        <label className="block text-sm font-medium mb-1">{field.label}</label>
        <select
          value={value}
          onChange={(e) => onChange(field.key, e.target.value)}
          className={inputClass}
        >
          {getSelectOptions(field).map((option) => (
            <option key={option.value} value={option.value}>
              {option.label}
            </option>
          ))}
        </select>
      </div>
    );
  }

  switch (field.field_type) {
    case "password":
      return (
        <div key={field.key}>
          <label className="block text-sm font-medium mb-1">{field.label}</label>
          <input
            type="password"
            value={value}
            onChange={(e) => onChange(field.key, e.target.value)}
            placeholder={field.placeholder}
            className={inputClass}
          />
          {field.description && (
            <p className="mt-1 text-xs text-gray-500 dark:text-gray-400">{field.description}</p>
          )}
        </div>
      );
    case "url":
    case "path":
    case "text":
      return (
        <div key={field.key}>
          <label className="block text-sm font-medium mb-1">{field.label}</label>
          <input
            type="text"
            value={value}
            onChange={(e) => onChange(field.key, e.target.value)}
            placeholder={field.placeholder}
            className={inputClass}
          />
          {field.description && (
            <p className="mt-1 text-xs text-gray-500 dark:text-gray-400">{field.description}</p>
          )}
        </div>
      );
    case "number":
      return (
        <div key={field.key}>
          <label className="block text-sm font-medium mb-1">{field.label}</label>
          <input
            type="number"
            value={value}
            onChange={(e) => onChange(field.key, e.target.value)}
            placeholder={field.placeholder}
            className={inputClass}
          />
          {field.description && (
            <p className="mt-1 text-xs text-gray-500 dark:text-gray-400">{field.description}</p>
          )}
        </div>
      );
    case "toggle":
      return (
        <label key={field.key} className="flex items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={value === "true"}
            onChange={(e) => onChange(field.key, e.target.checked ? "true" : "false")}
          />
          <span>{field.label}</span>
        </label>
      );
    default:
      return null;
  }
}

export function SettingsDialog({ onClose, onModelChange }: SettingsDialogProps) {
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [embeddedModels, setEmbeddedModels] = useState<EmbeddedModelInfo[]>([]);
  const [selectedWhisperModelKey, setSelectedWhisperModelKey] = useState<string>("");
  const [selectedEmbeddedModelKey, setSelectedEmbeddedModelKey] = useState<string>("");
  const [downloading, setDownloading] = useState<string | null>(null);
  const [downloadProgress, setDownloadProgress] = useState<DownloadProgress | null>(null);
  const [embeddedDownloading, setEmbeddedDownloading] = useState<string | null>(null);
  const [embeddedDownloadProgress, setEmbeddedDownloadProgress] =
    useState<EmbeddedDownloadProgress | null>(null);
  const [error, setError] = useState<string | null>(null);

  const [modes, setModes] = useState<TranslateModeInfo[]>([]);
  const [services, setServices] = useState<ServiceInfo[]>([]);
  const [settings, setSettings] = useState<TranslationSettings | null>(null);
  const [appConfig, setAppConfig] = useState<AppConfig | null>(null);
  const [healthStatus, setHealthStatus] = useState<string | null>(null);
  const [asrCapabilities, setAsrCapabilities] = useState<AsrRuntimeCapabilities | null>(null);
  const [showAsrCapabilities, setShowAsrCapabilities] = useState(false);

  const loadModels = useCallback(async () => {
    const list = await invoke<ModelInfo[]>("list_models");
    setModels(list);
  }, []);

  const loadEmbeddedModels = useCallback(async () => {
    const list = await invoke<EmbeddedModelInfo[]>("list_embedded_models");
    setEmbeddedModels(list);
  }, []);

  const loadTranslateSettings = useCallback(async () => {
    const [modeList, serviceList, saved, savedAppConfig, runtimeCapabilities] = await Promise.all([
      invoke<TranslateModeInfo[]>("list_translate_modes"),
      invoke<ServiceInfo[]>("list_translate_services"),
      invoke<TranslationSettings>("get_translate_settings"),
      invoke<AppConfig>("get_app_config"),
      invoke<AsrRuntimeCapabilities>("get_asr_runtime_capabilities"),
    ]);
    setModes(modeList);
    setServices(serviceList);
    setSettings(saved);
    setAppConfig(savedAppConfig);
    setAsrCapabilities(runtimeCapabilities);
  }, []);

  useEffect(() => {
    loadModels().catch((err) => console.error("Failed to load models:", err));
    loadEmbeddedModels().catch((err) =>
      console.error("Failed to load embedded models:", err),
    );
    loadTranslateSettings().catch((err) =>
      console.error("Failed to load translate settings:", err),
    );

    const unlisten = listen<DownloadProgress>("model-download-progress", (event) => {
      setDownloadProgress(event.payload);
    });
    const unlistenEmbedded = listen<EmbeddedDownloadProgress>(
      "embedded-model-download-progress",
      (event) => {
        setEmbeddedDownloadProgress(event.payload);
      },
    );

    return () => {
      unlisten.then((fn) => fn());
      unlistenEmbedded.then((fn) => fn());
    };
  }, [loadModels, loadEmbeddedModels, loadTranslateSettings]);

  useEffect(() => {
    if (!selectedWhisperModelKey && models.length > 0) {
      setSelectedWhisperModelKey(models.find((model) => model.downloaded)?.key ?? models[0].key);
    }
  }, [models, selectedWhisperModelKey]);

  const activeMode = settings?.active_mode ?? "online_translate";
  const filteredServices = useMemo(
    () => services.filter((service) => service.descriptor.mode === activeMode),
    [services, activeMode],
  );
  const activeServiceInfo = filteredServices.find(
    (service) => service.descriptor.key === settings?.active_service,
  );
  const activeConfig = settings?.service_configs[settings.active_service];
  const selectedWhisperModel =
    models.find((model) => model.key === selectedWhisperModelKey) ?? models[0] ?? null;
  const selectedEmbeddedModel =
    embeddedModels.find((model) => model.key === selectedEmbeddedModelKey) ?? embeddedModels[0] ?? null;

  useEffect(() => {
    const currentEmbeddedKey = settings?.service_configs.llama_cpp?.fields.model_key;
    if (currentEmbeddedKey) {
      setSelectedEmbeddedModelKey(currentEmbeddedKey);
      return;
    }
    if (!selectedEmbeddedModelKey && embeddedModels.length > 0) {
      setSelectedEmbeddedModelKey(
        embeddedModels.find((model) => model.downloaded)?.key ?? embeddedModels[0].key,
      );
    }
  }, [settings, embeddedModels, selectedEmbeddedModelKey]);

  const debugSelection = useCallback(
    async (nextSettings: TranslationSettings, modeKey: TranslateModeKey, serviceKey: string) => {
      try {
        await invoke("debug_select_translate_service", {
          modeKey,
          serviceKey,
          settings: nextSettings,
        });
      } catch (err) {
        console.error("Failed to debug selection:", err);
      }
    },
    [],
  );

  const handleModeChange = async (modeKey: TranslateModeKey) => {
    if (!settings) return;
    const nextService =
      services.find((service) => service.descriptor.mode === modeKey)?.descriptor.key ?? "";
    const nextSettings: TranslationSettings = {
      ...settings,
      active_mode: modeKey,
      active_service: nextService,
    };
    setSettings(nextSettings);
    setHealthStatus(null);
    await debugSelection(nextSettings, modeKey, nextService);
  };

  const handleServiceChange = async (serviceKey: string) => {
    if (!settings) return;
    const nextSettings: TranslationSettings = {
      ...settings,
      active_service: serviceKey,
    };
    setSettings(nextSettings);
    setHealthStatus(null);
    await debugSelection(nextSettings, nextSettings.active_mode, serviceKey);
  };

  const handleConfigChange = (key: string, value: string) => {
    if (!settings || !activeServiceInfo) return;
    setSettings({
      ...settings,
      service_configs: {
        ...settings.service_configs,
        [activeServiceInfo.descriptor.key]: {
          service_key: activeServiceInfo.descriptor.key,
          fields: {
            ...settings.service_configs[activeServiceInfo.descriptor.key]?.fields,
            [key]: value,
          },
        },
      },
    });
  };

  const handleEmbeddedModelSelect = (modelKey: string) => {
    setSelectedEmbeddedModelKey(modelKey);
    if (!settings) return;
    setSettings({
      ...settings,
      active_mode: "embedded_llm",
      active_service: "llama_cpp",
      service_configs: {
        ...settings.service_configs,
        llama_cpp: {
          service_key: "llama_cpp",
          fields: {
            ...settings.service_configs.llama_cpp?.fields,
            model_key: modelKey,
          },
        },
      },
    });
  };

  const handleSave = async () => {
    if (!settings || !appConfig) return;
    await invoke("save_translate_settings", { settings });
    await invoke("save_app_config", { config: appConfig });
    onModelChange?.();
  };

  const updateGpuSetting = (useGpu: boolean) => {
    if (!appConfig) return;
    setAppConfig({
      ...appConfig,
      general: {
        ...appConfig.general,
        use_gpu: useGpu,
      },
    });
  };

  const updateThreadSetting = (value: string) => {
    if (!appConfig) return;
    const parsed = Number.parseInt(value, 10);
    setAppConfig({
      ...appConfig,
      asr: {
        ...appConfig.asr,
        n_threads: Number.isFinite(parsed) && parsed >= 0 ? parsed : 0,
      },
    });
  };

  const handleHealthCheck = async () => {
    if (!settings) return;
    const status = await invoke("health_check_translate_service", {
      serviceKey: settings.active_service,
    });
    setHealthStatus(JSON.stringify(status));
  };

  const handleDownload = async (modelKey: string) => {
    setDownloading(modelKey);
    setError(null);
    setDownloadProgress(null);
    try {
      await invoke<string>("download_model", { modelKey });
      await loadModels();
      onModelChange?.();
    } catch (err) {
      setError(String(err));
    } finally {
      setDownloading(null);
      setDownloadProgress(null);
    }
  };

  const handleEmbeddedDownload = async (modelKey: string) => {
    setEmbeddedDownloading(modelKey);
    setError(null);
    setEmbeddedDownloadProgress(null);
    try {
      await invoke<string>("download_embedded_model", { modelKey });
      await loadEmbeddedModels();
      onModelChange?.();
    } catch (err) {
      setError(String(err));
    } finally {
      setEmbeddedDownloading(null);
      setEmbeddedDownloadProgress(null);
    }
  };

  const handleOpenDirectory = async () => {
    try {
      await invoke("open_model_directory");
    } catch (err) {
      console.error("Failed to open directory:", err);
    }
  };

  const handleOpenEmbeddedDirectory = async () => {
    try {
      await invoke("open_embedded_model_directory");
    } catch (err) {
      console.error("Failed to open embedded model directory:", err);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="max-h-[85vh] w-full max-w-2xl overflow-y-auto rounded-2xl bg-white p-6 shadow-xl dark:bg-gray-800">
        <div className="mb-6 flex items-center justify-between">
          <h2 className="text-xl font-bold">设置</h2>
          <button onClick={onClose} className="text-xl text-gray-500">
            ✕
          </button>
        </div>

        <section className="mb-6">
          <div className="mb-3 flex items-center justify-between gap-3">
            <h3 className="text-sm font-medium uppercase tracking-wider text-gray-500">ASR 性能</h3>
            <button
              onClick={() => setShowAsrCapabilities((value) => !value)}
              className="rounded-lg border border-gray-300 px-3 py-1.5 text-xs hover:bg-gray-100 dark:border-gray-600 dark:hover:bg-gray-700"
            >
              {showAsrCapabilities ? "收起当前构建特性" : "查看当前构建特性"}
            </button>
          </div>

          <div className="rounded-lg border border-gray-200 p-4 dark:border-gray-700">
            <div className="mb-4 flex flex-wrap items-center gap-2 text-xs text-gray-500 dark:text-gray-400">
              <span className="rounded-full bg-gray-100 px-2 py-1 dark:bg-gray-700">
                当前版本 {asrCapabilities ? `v${asrCapabilities.app_version}` : "加载中..."}
              </span>
              <span className="rounded-full bg-gray-100 px-2 py-1 dark:bg-gray-700">
                ASR 引擎 {asrCapabilities?.whisper_backend ?? "加载中..."}
              </span>
              <span className="rounded-full bg-gray-100 px-2 py-1 dark:bg-gray-700">
                GPU 后端{" "}
                {asrCapabilities
                  ? asrCapabilities.enabled_gpu_backends.length > 0
                    ? asrCapabilities.enabled_gpu_backends.join(" / ")
                    : "未启用"
                  : "加载中..."}
              </span>
            </div>

            {showAsrCapabilities && asrCapabilities && (
              <div className="mb-4 rounded-lg bg-gray-50 p-3 dark:bg-gray-900/60">
                <div className="mb-3 text-xs text-gray-500 dark:text-gray-400">
                  当前版本中已编译的 ASR 性能相关能力如下，GPU 开关是否生效取决于这些特性是否可用。
                </div>
                <div className="grid grid-cols-1 gap-2 md:grid-cols-2">
                  {asrCapabilities.features.map((feature) => (
                    <div
                      key={feature.key}
                      className="rounded-lg border border-gray-200 bg-white p-3 dark:border-gray-700 dark:bg-gray-800"
                    >
                      <div className="flex items-center justify-between gap-2">
                        <span className="text-sm font-medium">{feature.label}</span>
                        <span
                          className={`rounded-full px-1.5 py-0.5 text-[10px] font-medium ${
                            feature.enabled
                              ? "bg-green-500/15 text-green-600 dark:text-green-400"
                              : "bg-gray-500/15 text-gray-600 dark:text-gray-400"
                          }`}
                        >
                          {feature.enabled ? "已启用" : "未启用"}
                        </span>
                      </div>
                      <p className="mt-1 text-xs text-gray-500 dark:text-gray-400">
                        {feature.detail}
                      </p>
                    </div>
                  ))}
                </div>
              </div>
            )}

            <label className="flex items-center gap-2 text-sm">
              <input
                type="checkbox"
                checked={appConfig?.general.use_gpu ?? false}
                onChange={(e) => updateGpuSetting(e.target.checked)}
              />
              <span>启用 GPU 加速</span>
            </label>
            <p className="mt-2 text-xs text-gray-500 dark:text-gray-400">
              {asrCapabilities?.gpu_backend_available
                ? `当前构建已启用 ${asrCapabilities.enabled_gpu_backends.join(" / ")} 等 ASR GPU 后端，打开后可尝试使用硬件加速。`
                : "当前构建未启用可用的 ASR GPU 后端，打开该选项通常不会生效。"}
            </p>

            <div className="mt-4">
              <label className="mb-1 block text-sm font-medium">ASR 线程数</label>
              <input
                type="number"
                min={0}
                step={1}
                value={appConfig?.asr.n_threads ?? 0}
                onChange={(e) => updateThreadSetting(e.target.value)}
                className="w-full rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800"
              />
              <p className="mt-1 text-xs text-gray-500 dark:text-gray-400">
                `0` 表示自动选择推荐线程数，通常会比直接使用全部逻辑核心更快。
              </p>
            </div>
          </div>
        </section>

        <section className="mb-6">
          <div className="mb-3 flex items-center justify-between">
            <h3 className="text-sm font-medium uppercase tracking-wider text-gray-500">
              Whisper 模型库
            </h3>
            <button
              onClick={handleOpenDirectory}
              className="text-xs text-blue-500 hover:text-blue-400"
            >
              打开模型目录
            </button>
          </div>

          <div className="rounded-lg border border-gray-200 p-4 dark:border-gray-700">
            <div className="grid grid-cols-1 gap-3 md:grid-cols-[1fr_auto_auto] md:items-end">
              <div>
                <label className="mb-1 block text-sm font-medium">Whisper 模型</label>
                <select
                  value={selectedWhisperModelKey}
                  onChange={(e) => setSelectedWhisperModelKey(e.target.value)}
                  className="w-full rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800"
                >
                  {models.map((model) => (
                    <option key={model.key} value={model.key}>
                      {model.name} {model.downloaded ? "✓" : "(未下载)"}
                    </option>
                  ))}
                </select>
              </div>
              <button
                onClick={() => selectedWhisperModel && handleDownload(selectedWhisperModel.key)}
                disabled={
                  !selectedWhisperModel ||
                  selectedWhisperModel.downloaded ||
                  downloading !== null ||
                  embeddedDownloading !== null
                }
                className="rounded-lg bg-blue-600 px-3 py-2 text-xs font-medium text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50"
              >
                {selectedWhisperModel && downloading === selectedWhisperModel.key ? "下载中..." : "下载"}
              </button>
              <button
                onClick={handleOpenDirectory}
                className="rounded-lg border border-gray-300 px-3 py-2 text-xs hover:bg-gray-100 dark:border-gray-600 dark:hover:bg-gray-700"
              >
                打开目录
              </button>
            </div>

            {selectedWhisperModel && (
              <div className="mt-3">
                <div className="flex flex-wrap items-center gap-2">
                  <span className="text-sm font-medium">{selectedWhisperModel.name}</span>
                  <span
                    className={`rounded-full px-1.5 py-0.5 text-[10px] font-medium ${
                      selectedWhisperModel.downloaded
                        ? "bg-green-500/15 text-green-600 dark:text-green-400"
                        : "bg-amber-500/15 text-amber-600 dark:text-amber-400"
                    }`}
                  >
                    {selectedWhisperModel.downloaded ? "已下载" : "未下载"}
                  </span>
                </div>
                <p className="mt-1 text-xs text-gray-500 dark:text-gray-400">
                  {selectedWhisperModel.description}
                </p>
                {downloading === selectedWhisperModel.key && downloadProgress && (
                  <div className="mt-2">
                    <div className="mb-1 flex items-center justify-between text-xs text-gray-500">
                      <span>
                        {formatBytes(downloadProgress.downloaded_bytes)} /{" "}
                        {formatBytes(downloadProgress.total_bytes)}
                      </span>
                      <span>{downloadProgress.percent.toFixed(1)}%</span>
                    </div>
                    <div className="h-1.5 w-full rounded-full bg-gray-200 dark:bg-gray-700">
                      <div
                        className="h-1.5 rounded-full bg-blue-500 transition-all duration-300"
                        style={{ width: `${downloadProgress.percent}%` }}
                      />
                    </div>
                  </div>
                )}
              </div>
            )}
          </div>

          {error && (
            <div className="mt-3 rounded-lg border border-red-500/30 bg-red-500/10 p-2 text-xs text-red-500">
              {error}
            </div>
          )}
        </section>

        <section className="mb-6">
          <div className="mb-3 flex items-center justify-between">
            <h3 className="text-sm font-medium uppercase tracking-wider text-gray-500">
              内嵌 LLM 模型库
            </h3>
            <button
              onClick={handleOpenEmbeddedDirectory}
              className="text-xs text-blue-500 hover:text-blue-400"
            >
              打开模型目录
            </button>
          </div>

          <div className="rounded-lg border border-gray-200 p-4 dark:border-gray-700">
            <div className="grid grid-cols-1 gap-3 md:grid-cols-[1fr_auto_auto] md:items-end">
              <div>
                <label className="mb-1 block text-sm font-medium">内嵌 LLM 模型</label>
                <select
                  value={selectedEmbeddedModelKey}
                  onChange={(e) => handleEmbeddedModelSelect(e.target.value)}
                  className="w-full rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800"
                >
                  {embeddedModels.map((model) => (
                    <option key={model.key} value={model.key}>
                      {model.name} {model.downloaded ? "✓" : "(未下载)"}
                    </option>
                  ))}
                </select>
              </div>
              <button
                onClick={() => selectedEmbeddedModel && handleEmbeddedDownload(selectedEmbeddedModel.key)}
                disabled={
                  !selectedEmbeddedModel ||
                  selectedEmbeddedModel.downloaded ||
                  embeddedDownloading !== null ||
                  downloading !== null
                }
                className="rounded-lg bg-blue-600 px-3 py-2 text-xs font-medium text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50"
              >
                {selectedEmbeddedModel && embeddedDownloading === selectedEmbeddedModel.key
                  ? "下载中..."
                  : "下载"}
              </button>
              <button
                onClick={handleOpenEmbeddedDirectory}
                className="rounded-lg border border-gray-300 px-3 py-2 text-xs hover:bg-gray-100 dark:border-gray-600 dark:hover:bg-gray-700"
              >
                打开目录
              </button>
            </div>

            {selectedEmbeddedModel && (
              <div className="mt-3">
                <div className="flex flex-wrap items-center gap-2">
                  <span className="text-sm font-medium">{selectedEmbeddedModel.name}</span>
                  <span
                    className={`rounded-full px-1.5 py-0.5 text-[10px] font-medium ${
                      settings?.active_service === "llama_cpp" &&
                      settings?.service_configs.llama_cpp?.fields.model_key === selectedEmbeddedModel.key
                        ? "bg-blue-500/15 text-blue-600 dark:text-blue-400"
                        : selectedEmbeddedModel.downloaded
                          ? "bg-green-500/15 text-green-600 dark:text-green-400"
                          : "bg-amber-500/15 text-amber-600 dark:text-amber-400"
                    }`}
                  >
                    {settings?.active_service === "llama_cpp" &&
                    settings?.service_configs.llama_cpp?.fields.model_key === selectedEmbeddedModel.key
                      ? "当前使用"
                      : selectedEmbeddedModel.downloaded
                        ? "已下载"
                        : "未下载"}
                  </span>
                </div>
                <p className="mt-1 text-xs text-gray-500 dark:text-gray-400">
                  {selectedEmbeddedModel.description}
                </p>
                {embeddedDownloading === selectedEmbeddedModel.key && embeddedDownloadProgress && (
                  <div className="mt-2">
                    <div className="mb-1 flex items-center justify-between text-xs text-gray-500">
                      <span>
                        {formatBytes(embeddedDownloadProgress.downloaded_bytes)} /{" "}
                        {formatBytes(embeddedDownloadProgress.total_bytes)}
                      </span>
                      <span>{embeddedDownloadProgress.percent.toFixed(1)}%</span>
                    </div>
                    <div className="h-1.5 w-full rounded-full bg-gray-200 dark:bg-gray-700">
                      <div
                        className="h-1.5 rounded-full bg-blue-500 transition-all duration-300"
                        style={{ width: `${embeddedDownloadProgress.percent}%` }}
                      />
                    </div>
                  </div>
                )}
              </div>
            )}
          </div>
        </section>

        <section className="mb-6">
          <h3 className="mb-3 text-sm font-medium uppercase tracking-wider text-gray-500">
            翻译设置
          </h3>

          <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
            <div>
              <label className="mb-1 block text-sm font-medium">翻译模式</label>
              <select
                value={activeMode}
                onChange={(e) => void handleModeChange(e.target.value as TranslateModeKey)}
                className="w-full rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800"
              >
                {modes.map((mode) => (
                  <option key={mode.key} value={mode.key}>
                    {mode.name}
                  </option>
                ))}
              </select>
              <p className="mt-1 text-xs text-gray-500 dark:text-gray-400">
                {modes.find((mode) => mode.key === activeMode)?.description}
              </p>
            </div>

            <div>
              <label className="mb-1 block text-sm font-medium">翻译服务</label>
              <select
                value={settings?.active_service ?? ""}
                onChange={(e) => void handleServiceChange(e.target.value)}
                className="w-full rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800"
              >
                {filteredServices.map((service) => (
                  <option key={service.descriptor.key} value={service.descriptor.key}>
                    {service.descriptor.name}
                  </option>
                ))}
              </select>
              <p className="mt-1 text-xs text-gray-500 dark:text-gray-400">
                {activeServiceInfo?.descriptor.description}
              </p>
            </div>
          </div>

          {activeServiceInfo && activeConfig && (
            <div className="mt-4 rounded-lg border border-gray-200 p-4 dark:border-gray-700">
              <div className="mb-3 flex items-center justify-between">
                <div>
                  <h4 className="text-sm font-medium">{activeServiceInfo.descriptor.name}</h4>
                  <p className="text-xs text-gray-500 dark:text-gray-400">
                    当前服务配置项
                  </p>
                </div>
                <button
                  onClick={() => void handleHealthCheck()}
                  className="rounded-lg border border-gray-300 px-2.5 py-1.5 text-xs hover:bg-gray-100 dark:border-gray-600 dark:hover:bg-gray-700"
                >
                  健康检查
                </button>
              </div>

              {healthStatus && (
                <div className="mb-3 rounded-lg bg-gray-100 px-3 py-2 text-xs text-gray-600 dark:bg-gray-900 dark:text-gray-300">
                  {healthStatus}
                </div>
              )}

              <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
                {activeServiceInfo.descriptor.config_schema
                  .filter((field) => !(activeServiceInfo.descriptor.key === "llama_cpp" && field.key === "model_key"))
                  .map((field) =>
                  renderConfigField(
                    field,
                    activeConfig.fields[field.key] ?? field.default,
                    handleConfigChange,
                  ),
                )}
              </div>
            </div>
          )}

          <button
            onClick={() => void handleSave()}
            className="mt-4 w-full rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700"
          >
            保存设置
          </button>
        </section>

        <div className="flex justify-end gap-3">
          <button
            onClick={onClose}
            className="rounded-lg border border-gray-300 px-4 py-2 text-sm hover:bg-gray-100 dark:border-gray-600 dark:hover:bg-gray-700"
          >
            关闭
          </button>
        </div>
      </div>
    </div>
  );
}
