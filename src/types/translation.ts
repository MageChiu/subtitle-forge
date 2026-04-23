export interface ConfigField {
  key: string;
  label: string;
  field_type: "text" | "password" | "url" | "number" | "select" | "toggle";
  default: string;
  required: boolean;
  placeholder?: string;
  description?: string;
  options?: SelectOption[];
}

export interface SelectOption {
  value: string;
  label: string;
}

export interface PluginMetadata {
  namespace: string;
  display_name: string;
  description: string;
  version: string;
  category: "remote_api" | "remote_llm" | "local_llm";
  requires_network: boolean;
  config_schema: ConfigField[];
}

export type HealthStatus = "healthy" | { degraded: string } | { unhealthy: string } | "unknown";

export interface PluginInfo {
  metadata: PluginMetadata;
  is_available: boolean;
  health_status: HealthStatus | null;
}

export interface PluginConfig {
  namespace: string;
  fields: Record<string, string>;
}

export interface AllPluginConfigs {
  active_plugin: string;
  configs: Record<string, PluginConfig>;
}

export const CATEGORY_LABELS: Record<string, string> = {
  remote_api: "Online Translation Services",
  remote_llm: "Remote LLM Services",
  local_llm: "Local LLM Services",
};

export const CATEGORY_ORDER = ["remote_api", "remote_llm", "local_llm"];
