import { createResource, Show } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { t } from "../../i18n";
import { Button, Field, Notice, Radio, RadioGroup, Select, TextInput } from "../../ui";
import type { KeyChoice } from "./request";
import { st } from "./text";

export interface CertInfo { thumbprint: string; subject: string; issuer: string; not_after: string; has_private_key: boolean }

const fileName = (p: string) => p.split(/[\\/]/).pop() ?? p;

/** Elección de la clave de firma: certificado del almacén de Windows (incluye el DNIe) o archivo PFX. */
export function KeyChooser(p: { value: KeyChoice; onChange: (k: KeyChoice) => void }) {
  const [certs] = createResource(() => invoke<CertInfo[]>("sign_list_certificates").catch(() => [] as CertInfo[]));
  const usable = () => (certs() ?? []).filter((c) => c.has_private_key);
  const set = (patch: Partial<KeyChoice>) => p.onChange({ ...p.value, ...patch });
  const pick = async () => {
    const path = await invoke<string | null>("pick_key_file", { title: st("sign.pfx.choose"), filter: t("dialog.filter.key") });
    if (path) set({ pfxPath: path });
  };
  return (
    <>
      <RadioGroup legend={st("sign.key")} value={p.value.source} onChange={(v) => set({ source: v as KeyChoice["source"] })}>
        <Radio value="store" label={st("sign.key.store")} />
        <Radio value="pfx" label={st("sign.key.pfx")} />
      </RadioGroup>
      <Show when={p.value.source === "store"}>
        <Show when={!certs.loading} fallback={<p class="pl-muted">{st("sign.cert.loading")}</p>}>
          <Show when={usable().length > 0} fallback={<Notice tone="warn" inline>{st("sign.cert.empty")}</Notice>}>
            <Field label={st("sign.cert")}>
              <Select
                value={p.value.thumbprint} onChange={(e) => set({ thumbprint: e.currentTarget.value })}
                options={[{ value: "", label: st("sign.cert.none") }, ...usable().map((c) => ({
                  value: c.thumbprint, label: `${c.subject} · ${c.issuer} · ${st("sign.cert.until", { date: c.not_after.slice(0, 10) })}`,
                }))]}
              />
            </Field>
          </Show>
        </Show>
      </Show>
      <Show when={p.value.source === "pfx"}>
        <Field label={st("sign.pfx.file")}>
          <div class="pl-sign-row">
            <span class="pl-sign-file" title={p.value.pfxPath}>{p.value.pfxPath ? fileName(p.value.pfxPath) : st("sign.pfx.none")}</span>
            <Button onClick={() => void pick()}>{st("sign.pfx.choose")}</Button>
          </div>
        </Field>
        <Field label={st("sign.pfx.password")}>
          <TextInput type="password" autocomplete="off" value={p.value.password} onInput={(e) => set({ password: e.currentTarget.value })} />
        </Field>
      </Show>
    </>
  );
}

