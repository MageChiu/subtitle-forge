import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useState, useEffect, useCallback } from "react";
import type {
  PluginInfo,
  AllPluginConfigs,
  ConfigField,
} from "../types/translation";
import { CATEGORY_LABELS, CATEGORY_ORDER } from "../types/translation";

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

function renderConfigField(
  field: ConfigField,
  value: string,
  onChange: (key: string, value: string) => void,
) {
  const inputClass =
    "w-full px-3 py-2 rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 text-sm";

  switch (field.field_type) {
    case "password":
      return (
        <div key={field.key}>
          <label className="block text-sm font-medium mb-1">
            {field.label}
            {field.required && <span className="text-red-500 ml-1">*</span>}
          </label>
          <input
            type="password"
            value={value}
            onChange={(e) => onChange(field.key, e.target.value)}
            placeholder={field.placeholder}
            className={inputClass}
          />
          {field.description && (
            <p className="text-xs text-gray-500 dark:text-gray-400 mt-1">{field.description}</p>
          )}
        </div>
      );
    case "url":
      return (
        <div key={field.key}>
          <label className="block text-sm font-medium mb-1">
            {field.label}
            {field.required && <span className="text-red-500 ml-1">*</span>}
          </label>
          <input
            type="url"
            value={value}
            onChange={(e) => onChange(field.key, e.target.value)}
            placeholder={field.placeholder}
            className={inputClass}
          />
          {field.description && (
            <p className="text-xs text-gray-500 dark:text-gray-400 mt-1">{field.description}</p>
          )}
        </div>
      );
    case "number":
      return (
        <div key={field.key}>
          <label className="block text-sm font-medium mb-1">
            {field.label}
            {field.required && <span className="text-red-500 ml-1">*</span>}
          </label>
          <input
            type="number"
            value={value}
            onChange={(e) => onChange(field.key, e.target.value)}
            placeholder={field.placeholder}
            className={inputClass}
          />
          {field.description && (
            <p className="text-xs text-gray-500 dark:text-gray-400 mt-1">{field.description}</p>
          )}
        </div>
      );
    case "select":
      return (
        <div key={field.key}>
          <label className="block text-sm font-medium mb-1">
            {field.label}
            {field.required && <span className="text-red-500 ml-1">*</span>}
          </label>
          <select
            value={value}
            onChange={(e) => onChange(field.key, e.target.value)}
            className={inputClass}
          >
            {field.options?.map((opt) => (
              <option key={opt.value} value={opt.value}>
                {opt.label}
              </option>
            ))}
          </select>
        </div>
      );
    case "toggle":
      return (
        <div key={field.key} className="flex items-center gap-2">
          <input
            type="checkbox"
            checked={value === "true"}
            onChange={(e) => onChange(field.key, e.target.checked ? "true" : "false")}
            className="rounded"
          />
          <label className="text-sm font-medium">{field.label}</label>
        </div>
      );
    default:
      return (
        <div key={field.key}>
          <label className="block text-sm font-medium mb-1">
            {field.label}
            {field.required && <span className="text-red-500 ml-1">*</span>}
          </label>
          <input
            type="text"
            value={value}
            onChange={(e) => onChange(field.key, e.target.value)}
            placeholder={field.placeholder}
            className={inputClass}
          />
          {field.description && (
            <p className="text-xs text-gray-500 dark:text-gray-400 mt-1">{field.description}</p>
          )}
        </div>
      );
  }
}

export function SettingsDialog({ onClose, onModelChange }: SettingsDialogProps) {
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [downloading, setDownloading] = useState<string | null>(null);
  const [downloadProgress, setDownloadProgress] = useState<DownloadProgress | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [showPluginConfig, setShowPluginConfig] = useState(false);

  const [plugins, setPlugins] = useState<PluginInfo[]>([]);
  const [allConfigs, setAllConfigs] = useState<AllPluginConfigs | null>(null);
  const [healthStatus, setHealthStatus] = useState<Record<string, string>>({});

  const loadModels = useCallback(async () => {
    try {
      const list = await invoke<ModelInfo[]>("list_models");
      setModels(list);
    } catch (err) {
      console.error("Failed to load models:", err);
    }
  }, []);

  const loadPlugins = useCallback(async () => {
    try {
      const [pluginList, configs] = await Promise.all([
        invoke<PluginInfo[]>("list_translate_plugins"),
        invoke<AllPluginConfigs>("get_plugin_configs"),
      ]);
      setPlugins(pluginList);
      setAllConfigs(configs);
    } catch (err) {
      console.error("Failed to load plugins:", err);
    }
  }, []);

  useEffect(() => {
    loadModels();
    loadPlugins();

    const unlisten = listen<DownloadProgress>("model-download-progress", (event) => {
      setDownloadProgress(event.payload);
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [loadModels, loadPlugins]);

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

  const handleOpenDirectory = async () => {
    try {
      await invoke("open_model_directory");
    } catch (err) {
      console.error("Failed to open directory:", err);
    }
  };

  const handleSelectPlugin = (namespace: string) => {
    if (!allConfigs) return;
    setAllConfigs({ ...allConfigs, active_plugin: namespace });
  };

  const handleConfigChange = (namespace: string, key: string, value: string) => {
    if (!allConfigs) return;
    const config = allConfigs.configs[namespace];
    if (!config) return;
    setAllConfigs({
      ...allConfigs,
      configs: {
        ...allConfigs.configs,
        [namespace]: {
          ...config,
          fields: { ...config.fields, [key]: value },
        },
      },
    });
  };

  const handleSave = async () => {
    if (!allConfigs) return;
    try {
      await invoke("save_plugin_configs", { configs: allConfigs });
    } catch (err) {
      console.error("Failed to save plugin configs:", err);
    }
  };

  const handleHealthCheck = async (namespace: string) => {
    try {
      const status = await invoke<string>("health_check_plugin", { namespace });
      setHealthStatus((prev) => ({ ...prev, [namespace]: status }));
    } catch (err) {
      setHealthStatus((prev) => ({ ...prev, [namespace]: `Error: ${err}` }));
    }
  };

  const activePlugin = allConfigs?.active_plugin ?? "google/v1";
  const activePluginInfo = plugins.find((p) => p.metadata.namespace === activePlugin);
  const activeConfig = allConfigs?.configs[activePlugin];
  const activeCategory = activePluginInfo?.metadata.category ?? CATEGORY_ORDER[0];
  const activeCategoryPlugins = plugins.filter((p) => p.metadata.category === activeCategory);

  const handleSelectCategory = (category: string) => {
    if (!allConfigs) return;
    const nextPlugin = plugins.find((p) => p.metadata.category === category);
    if (!nextPlugin) return;
    setAllConfigs({ ...allConfigs, active_plugin: nextPlugin.metadata.namespace });
  };

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
      <div className="bg-white dark:bg-gray-800 rounded-2xl p-6 w-full max-w-lg max-h-[85vh] overflow-y-auto shadow-xl">
        <div className="flex items-center justify-between mb-6">
          <h2 className="text-xl font-bold">Settings</h2>
          <button
            onClick={onClose}
            className="text-gray-500 hover:text-gray-700 dark:hover:text-gray-300 text-xl"
          >
            ✕
          </button>
        </div>

        <section className="mb-6">
          <div className="flex items-center justify-between mb-3">
            <h3 className="font-medium text-sm text-gray-500 uppercase tracking-wider">
              Whisper Models
            </h3>
            <button
              onClick={handleOpenDirectory}
              className="text-xs text-blue-500 hover:text-blue-400 underline"
            >
              Open Models Folder
            </button>
          </div>

          <div className="space-y-2">
            {models.map((model) => (
              <div
                key={model.key}
                className={`flex items-center justify-between p-3 rounded-lg border ${
                  model.downloaded
                    ? "border-green-500/30 bg-green-500/5"
                    : "border-gray-300 dark:border-gray-600"
                }`}
              >
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2">
                    <span className="font-medium text-sm">{model.name}</span>
                    {model.downloaded ? (
                      <span className="text-[10px] px-1.5 py-0.5 bg-green-500/20 text-green-600 dark:text-green-400 rounded-full font-medium">
                        Downloaded
                      </span>
                    ) : null}
                  </div>
                  <p className="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
                    {model.description}
                  </p>
                  {downloading === model.key && downloadProgress && (
                    <div className="mt-2">
                      <div className="flex items-center justify-between text-xs text-gray-500 mb-1">
                        <span>
                          {formatBytes(downloadProgress.downloaded_bytes)} /{" "}
                          {formatBytes(downloadProgress.total_bytes)}
                        </span>
                        <span>{downloadProgress.percent.toFixed(1)}%</span>
                      </div>
                      <div className="w-full bg-gray-200 dark:bg-gray-700 rounded-full h-1.5">
                        <div
                          className="bg-blue-500 h-1.5 rounded-full transition-all duration-300"
                          style={{ width: `${downloadProgress.percent}%` }}
                        />
                      </div>
                    </div>
                  )}
                </div>
                <div className="ml-3 shrink-0">
                  {model.downloaded ? (
                    <span className="text-green-500 text-lg">✓</span>
                  ) : (
                    <button
                      onClick={() => handleDownload(model.key)}
                      disabled={downloading !== null}
                      className="px-3 py-1.5 text-xs font-medium bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
                    >
                      {downloading === model.key ? "Downloading..." : "Download"}
                    </button>
                  )}
                </div>
              </div>
            ))}
          </div>

          {error && (
            <div className="mt-3 p-2 bg-red-500/10 border border-red-500/30 rounded-lg text-xs text-red-500">
              {error}
            </div>
          )}
        </section>

        <section className="mb-6">
          <h3 className="font-medium mb-3 text-sm text-gray-500 uppercase tracking-wider">
            Translation Plugin
          </h3>

          <div className="space-y-4">
            <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
              <div>
                <label className="block text-sm font-medium mb-1">Plugin Type</label>
                <select
                  value={activeCategory}
                  onChange={(e) => handleSelectCategory(e.target.value)}
                  className="w-full px-3 py-2 rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 text-sm"
                >
                  {CATEGORY_ORDER.map((category) => (
                    <option key={category} value={category}>
                      {CATEGORY_LABELS[category]}
                    </option>
                  ))}
                </select>
              </div>
              <div>
                <label className="block text-sm font-medium mb-1">Plugin</label>
                <select
                  value={activePlugin}
                  onChange={(e) => handleSelectPlugin(e.target.value)}
                  className="w-full px-3 py-2 rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 text-sm"
                >
                  {activeCategoryPlugins.map((plugin) => (
                    <option key={plugin.metadata.namespace} value={plugin.metadata.namespace}>
                      {plugin.metadata.display_name}
                    </option>
                  ))}
                </select>
              </div>
            </div>

            {activePluginInfo && (
              <div className="p-3 rounded-lg border border-gray-200 dark:border-gray-700 bg-gray-50/60 dark:bg-gray-900/30">
                <div className="flex items-start justify-between gap-3">
                  <div className="min-w-0">
                    <div className="flex items-center gap-2 flex-wrap">
                      <span className="font-medium text-sm">{activePluginInfo.metadata.display_name}</span>
                      <span className="text-[10px] px-1.5 py-0.5 bg-gray-200 dark:bg-gray-700 rounded-full text-gray-500">
                        v{activePluginInfo.metadata.version}
                      </span>
                      <span className="text-[10px] px-1.5 py-0.5 bg-blue-500/10 text-blue-600 dark:text-blue-400 rounded-full">
                        {CATEGORY_LABELS[activePluginInfo.metadata.category]}
                      </span>
                    </div>
                    <p className="text-xs text-gray-500 dark:text-gray-400 mt-1">
                      {activePluginInfo.metadata.description}
                    </p>
                    {healthStatus[activePlugin] && (
                      <p className="text-xs text-gray-400 mt-1">
                        Status: {healthStatus[activePlugin]}
                      </p>
                    )}
                  </div>
                  <div className="flex items-center gap-2 shrink-0">
                    <button
                      onClick={() => handleHealthCheck(activePlugin)}
                      className="px-2.5 py-1.5 text-xs rounded-lg border border-gray-300 dark:border-gray-600 hover:bg-gray-100 dark:hover:bg-gray-700"
                    >
                      Health Check
                    </button>
                    <button
                      onClick={() => setShowPluginConfig((v) => !v)}
                      className="px-2.5 py-1.5 text-xs rounded-lg border border-gray-300 dark:border-gray-600 hover:bg-gray-100 dark:hover:bg-gray-700"
                    >
                      {showPluginConfig ? "Hide Config" : "Show Config"}
                    </button>
                  </div>
                </div>
              </div>
            )}

            {activePluginInfo && activeConfig && (
              <div
                className={`border border-gray-200 dark:border-gray-700 rounded-lg ${
                  showPluginConfig ? "p-4" : "hidden"
                }`}
              >
                <h4 className="text-sm font-medium mb-3">
                  {activePluginInfo.metadata.display_name} Configuration
                </h4>
                <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
                  {activePluginInfo.metadata.config_schema.length === 0 ? (
                    <p className="text-xs text-gray-500 dark:text-gray-400 md:col-span-2">
                      No configuration required for this plugin.
                    </p>
                  ) : (
                    activePluginInfo.metadata.config_schema.map((field) =>
                      renderConfigField(
                        field,
                        activeConfig.fields[field.key] ?? field.default,
                        (key, value) => handleConfigChange(activePlugin, key, value),
                      ),
                    )
                  )}
                </div>
              </div>
            )}

            <button
              onClick={handleSave}
              className="w-full px-4 py-2 text-sm font-medium bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors"
            >
              Save Settings
            </button>
          </div>
        </section>

        <div className="flex gap-3 justify-end">
          <button
            onClick={onClose}
            className="px-4 py-2 text-sm rounded-lg border border-gray-300 dark:border-gray-600 hover:bg-gray-100 dark:hover:bg-gray-700"
          >
            Close
          </button>
        </div>
      </div>
    </div>
  );
}
