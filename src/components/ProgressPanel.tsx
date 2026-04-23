import { PipelineStage } from "../hooks/usePipeline";

interface ProgressPanelProps {
  stage: PipelineStage;
}

const STAGE_INFO: Record<string, { label: string; icon: string }> = {
  Idle: { label: "Ready", icon: "⏳" },
  ExtractingAudio: { label: "Extracting Audio", icon: "🎵" },
  Transcribing: { label: "Speech Recognition", icon: "🎙️" },
  Translating: { label: "Translating", icon: "🌐" },
  GeneratingSubtitle: { label: "Generating Subtitle", icon: "📝" },
  Completed: { label: "Complete!", icon: "✅" },
  Failed: { label: "Failed", icon: "❌" },
  Cancelled: { label: "Cancelled", icon: "⏹️" },
};

export function ProgressPanel({ stage }: ProgressPanelProps) {
  const info = STAGE_INFO[stage.type] || { label: stage.type, icon: "⏳" };
  const percent = stage.percent ?? 0;

  const bgColor =
    stage.type === "Completed"
      ? "bg-green-50 dark:bg-green-950/30 border-green-200 dark:border-green-800"
      : stage.type === "Failed"
        ? "bg-red-50 dark:bg-red-950/30 border-red-200 dark:border-red-800"
        : "bg-white dark:bg-gray-800 border-gray-200 dark:border-gray-700";

  return (
    <div className={`rounded-xl p-5 border ${bgColor}`}>
      {/* Stage label */}
      <div className="flex items-center justify-between mb-3">
        <span className="font-medium text-base">
          {info.icon} {info.label}
        </span>
        {stage.percent !== undefined && stage.type !== "Completed" && (
          <span className="text-sm text-gray-500 font-mono">
            {Math.round(percent)}%
          </span>
        )}
      </div>

      {/* Progress bar */}
      {stage.type !== "Completed" &&
        stage.type !== "Failed" &&
        stage.type !== "Cancelled" && (
          <div className="w-full bg-gray-200 dark:bg-gray-700 rounded-full h-2 mb-3">
            <div
              className="bg-blue-600 h-2 rounded-full transition-all duration-300 ease-out"
              style={{ width: `${Math.min(percent, 100)}%` }}
            />
          </div>
        )}

      {/* Current ASR text */}
      {stage.current_text && (
        <p className="text-sm text-gray-600 dark:text-gray-400 italic truncate">
          &ldquo;{stage.current_text}&rdquo;
        </p>
      )}

      {/* Translation progress */}
      {stage.translated_count !== undefined && (
        <p className="text-xs text-gray-500 mt-1">
          Translated {stage.translated_count} / {stage.total_count} segments
        </p>
      )}

      {/* Completed info */}
      {stage.type === "Completed" && (
        <div className="space-y-1">
          {stage.segment_count && (
            <p className="text-sm text-green-700 dark:text-green-400">
              {stage.segment_count} subtitle segments generated
            </p>
          )}
          {stage.output_path && (
            <p className="text-sm text-green-600 dark:text-green-500 font-mono text-xs break-all">
              📁 {stage.output_path}
            </p>
          )}
        </div>
      )}

      {/* Error */}
      {stage.error && (
        <p className="text-sm text-red-600 dark:text-red-400 mt-2">
          {stage.error}
        </p>
      )}
    </div>
  );
}
