import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useState, useCallback, useEffect } from "react";

export interface PipelineConfig {
  input_path: string;
  output_dir: string;
  source_language: string | null;
  target_language: string;
  output_format: "Srt" | "Ass" | "Vtt";
  asr_model: string;
  translate_engine: string;
  use_gpu: boolean;
}

export interface PipelineStage {
  type: string;
  stage?: string;
  percent?: number;
  current_text?: string;
  translated_count?: number;
  total_count?: number;
  output_path?: string;
  segment_count?: number;
  duration_ms?: number;
  error?: string;
}

const STAGE_MAP: Record<string, string> = {
  idle: "Idle",
  extracting_audio: "ExtractingAudio",
  transcribing: "Transcribing",
  translating: "Translating",
  generating_subtitle: "GeneratingSubtitle",
  completed: "Completed",
  failed: "Failed",
  cancelled: "Cancelled",
};

export function usePipeline() {
  const [stage, setStage] = useState<PipelineStage>({ type: "Idle" });
  const [isRunning, setIsRunning] = useState(false);

  useEffect(() => {
    const unlisten = listen<Record<string, unknown>>("pipeline-progress", (event) => {
      const payload = event.payload;
      const stageKey = (payload.stage as string) || "idle";
      const mapped: PipelineStage = {
        type: STAGE_MAP[stageKey] || stageKey,
        percent: payload.percent as number | undefined,
        current_text: payload.current_text as string | undefined,
        translated_count: payload.translated_count as number | undefined,
        total_count: payload.total_count as number | undefined,
        output_path: payload.output_path as string | undefined,
        segment_count: payload.segment_count as number | undefined,
        duration_ms: payload.duration_ms as number | undefined,
        error: payload.error as string | undefined,
      };

      setStage(mapped);

      if (
        mapped.type === "Completed" ||
        mapped.type === "Failed" ||
        mapped.type === "Cancelled"
      ) {
        setIsRunning(false);
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const start = useCallback(async (config: PipelineConfig) => {
    setIsRunning(true);
    setStage({ type: "ExtractingAudio", percent: 0 });
    try {
      const result = await invoke<string>("start_pipeline", { config });
      return result;
    } catch (err) {
      setStage({ type: "Failed", error: String(err) });
      setIsRunning(false);
      throw err;
    }
  }, []);

  const cancel = useCallback(async () => {
    try {
      await invoke("cancel_pipeline");
    } catch (err) {
      console.error("Cancel failed:", err);
    }
  }, []);

  return { stage, isRunning, start, cancel };
}
