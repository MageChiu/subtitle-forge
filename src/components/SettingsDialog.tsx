interface SettingsDialogProps {
  onClose: () => void;
}

export function SettingsDialog({ onClose }: SettingsDialogProps) {
  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
      <div className="bg-white dark:bg-gray-800 rounded-2xl p-6 w-full max-w-lg max-h-[80vh] overflow-y-auto shadow-xl">
        <div className="flex items-center justify-between mb-6">
          <h2 className="text-xl font-bold">Settings</h2>
          <button
            onClick={onClose}
            className="text-gray-500 hover:text-gray-700 dark:hover:text-gray-300 text-xl"
          >
            ✕
          </button>
        </div>

        {/* ASR Settings */}
        <section className="mb-6">
          <h3 className="font-medium mb-3 text-sm text-gray-500 uppercase tracking-wider">
            Speech Recognition
          </h3>
          <div className="space-y-3">
            <div>
              <label className="block text-sm font-medium mb-1">
                Whisper Model
              </label>
              <select className="w-full px-3 py-2 rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 text-sm">
                <option value="tiny">Tiny (75 MB) — Fastest</option>
                <option value="base" selected>Base (142 MB) — Balanced</option>
                <option value="small">Small (466 MB) — Better</option>
                <option value="medium">Medium (1.5 GB) — High</option>
                <option value="large">Large V3 (3.1 GB) — Best</option>
              </select>
            </div>
            <div className="flex items-center gap-2">
              <input type="checkbox" id="gpu" className="rounded" />
              <label htmlFor="gpu" className="text-sm">
                Enable GPU Acceleration (CUDA/Metal)
              </label>
            </div>
          </div>
        </section>

        {/* Translation Settings */}
        <section className="mb-6">
          <h3 className="font-medium mb-3 text-sm text-gray-500 uppercase tracking-wider">
            Translation
          </h3>
          <div className="space-y-3">
            <div>
              <label className="block text-sm font-medium mb-1">Engine</label>
              <select className="w-full px-3 py-2 rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 text-sm">
                <option value="llm">LLM API (OpenAI / DeepSeek)</option>
                <option value="deepl">DeepL</option>
                <option value="offline">Offline (Opus-MT)</option>
              </select>
            </div>
            <div>
              <label className="block text-sm font-medium mb-1">API Key</label>
              <input
                type="password"
                placeholder="sk-..."
                className="w-full px-3 py-2 rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 text-sm"
              />
            </div>
            <div>
              <label className="block text-sm font-medium mb-1">
                API Base URL
              </label>
              <input
                type="text"
                defaultValue="https://api.openai.com/v1"
                className="w-full px-3 py-2 rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 text-sm"
              />
            </div>
            <div>
              <label className="block text-sm font-medium mb-1">Model</label>
              <input
                type="text"
                defaultValue="gpt-4o-mini"
                className="w-full px-3 py-2 rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 text-sm"
              />
            </div>
          </div>
        </section>

        {/* Subtitle Settings */}
        <section className="mb-6">
          <h3 className="font-medium mb-3 text-sm text-gray-500 uppercase tracking-wider">
            Subtitle Output
          </h3>
          <div className="space-y-3">
            <div>
              <label className="block text-sm font-medium mb-1">
                Default Format
              </label>
              <select className="w-full px-3 py-2 rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 text-sm">
                <option value="srt">SRT (Most Compatible)</option>
                <option value="ass">ASS (Rich Styling)</option>
                <option value="vtt">VTT (Web)</option>
              </select>
            </div>
          </div>
        </section>

        {/* Save Button */}
        <div className="flex gap-3 justify-end">
          <button
            onClick={onClose}
            className="px-4 py-2 text-sm rounded-lg border border-gray-300 dark:border-gray-600 hover:bg-gray-100 dark:hover:bg-gray-700"
          >
            Cancel
          </button>
          <button
            onClick={onClose}
            className="px-4 py-2 text-sm bg-blue-600 text-white rounded-lg hover:bg-blue-700"
          >
            Save
          </button>
        </div>
      </div>
    </div>
  );
}
