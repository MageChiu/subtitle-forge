export type TranslateModeKey =
  | "online_translate"
  | "online_llm"
  | "local_llm"
  | "embedded_llm";

export interface TranslateModeInfo {
  key: TranslateModeKey;
  name: string;
  description: string;
}

export type ConfigFieldType =
  | "text"
  | "password"
  | "url"
  | "number"
  | "path"
  | "toggle"
  | { select: { options: SelectOption[] } };

export interface SelectOption {
  value: string;
  label: string;
}

export interface ConfigField {
  key: string;
  label: string;
  field_type:
    | "text"
    | "password"
    | "url"
    | "number"
    | "path"
    | "toggle"
    | { select: { options: SelectOption[] } };
  default: string;
  required: boolean;
  placeholder?: string;
  description?: string;
}

export interface ServiceDescriptor {
  key: string;
  name: string;
  description: string;
  mode: TranslateModeKey;
  requires_network: boolean;
  config_schema: ConfigField[];
}

export type HealthStatus = "healthy" | "unknown" | { degraded: string } | { unhealthy: string };

export interface ServiceInfo {
  descriptor: ServiceDescriptor;
  health_status: HealthStatus | null;
}

export interface ServiceConfig {
  service_key: string;
  fields: Record<string, string>;
}

export interface TranslationSettings {
  active_mode: TranslateModeKey;
  active_service: string;
  service_configs: Record<string, ServiceConfig>;
}

export const MODE_LABELS: Record<TranslateModeKey, string> = {
  online_translate: "在线翻译服务",
  online_llm: "在线LLM服务",
  local_llm: "本地LLM服务",
  embedded_llm: "内嵌LLM服务",
};
