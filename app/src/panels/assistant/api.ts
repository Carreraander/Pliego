//! Comandos y eventos del asistente local (src-tauri/src/commands/assistant.rs).
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface ModelInfo {
  id: string;
  name: string;
  /** SPDX de la licencia de los pesos. */
  license: string;
  size_bytes: number;
  recommended: boolean;
  requirements: { ram_gb: number; download_mb: number };
  notes: string;
  multimodal: boolean;
  installed: boolean;
}
export interface Status {
  enabled: boolean;
  model_id: string | null;
  /** La app incluye el motor llama.cpp. */
  engine_available: boolean;
  models_dir: string;
  models: ModelInfo[];
}
export interface Citation {
  /** Página desde 0. */
  page: number;
  snippet: string;
  cited_by_model: boolean;
}
export interface Answer {
  text: string;
  citations: Citation[];
}
export interface Summary {
  text: string;
  pages_read: number;
}
export interface KeyDatum {
  kind: "date" | "amount";
  value: string;
  /** Página desde 0. */
  page: number;
}
export interface AssistantProgress {
  job_id: string;
  done: number;
  total: number;
  label: string;
}

export const assistantApi = {
  status: () => invoke<Status>("assistant_status"),
  setEnabled: (enabled: boolean) => invoke<Status>("assistant_set_enabled", { enabled }),
  selectModel: (id: string) => invoke<Status>("assistant_select_model", { id }),
  cancel: (jobId: string) => invoke<void>("assistant_cancel", { jobId }),
  download: (id: string, jobId: string) => invoke<Status>("assistant_download", { id, jobId }),
  ask: (docId: number, question: string, jobId: string) => invoke<Answer>("assistant_ask", { docId, question, jobId }),
  summarize: (docId: number, jobId: string) => invoke<Summary>("assistant_summarize", { docId, jobId }),
  keyData: (docId: number, jobId: string) => invoke<KeyDatum[]>("assistant_keydata", { docId, jobId }),
};

export const assistantEvents = {
  onProgress: (f: (e: AssistantProgress) => void): Promise<UnlistenFn> =>
    listen<AssistantProgress>("assistant-progress", (e) => f(e.payload)),
};
