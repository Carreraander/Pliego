import { beforeEach, describe, expect, it } from "vitest";
import { jobs, onFinished, onJobFinished, onProgress, resetJobs, runningJobs, trackJob } from "./state";

beforeEach(resetJobs);

describe("centro de tareas", () => {
  it("el progreso crea la tarea aunque llegue antes que trackJob, y trackJob le pone título", () => {
    onProgress({ job_id: 1, done: 1, total: 4, label: "Página 1" });
    expect(runningJobs()).toHaveLength(1);
    trackJob(1, { title: "Comprimir contrato.pdf" });
    expect(jobs()[0]).toMatchObject({ id: 1, title: "Comprimir contrato.pdf", done: 1, total: 4, status: "running" });
  });
  it("termina con resultado, con error o cancelada, y avisa a los oyentes", () => {
    const seen: string[] = [];
    onJobFinished((j) => seen.push(`${j.id}:${j.status}`));
    onProgress({ job_id: 1, done: 0, total: 1, label: "a" });
    onProgress({ job_id: 2, done: 0, total: 1, label: "b" });
    onProgress({ job_id: 3, done: 0, total: 1, label: "c" });
    onFinished({ job_id: 1, result: { outputs: ["x.pdf"], warnings: [], summary: "ok" }, error: null });
    onFinished({ job_id: 2, result: null, error: { code: "other", message: "falló", cancelled: false } });
    onFinished({ job_id: 3, result: null, error: { code: "cancelled", message: "cancelada", cancelled: true } });
    expect(seen).toEqual(["1:done", "2:error", "3:cancelled"]);
    expect(jobs().find((j) => j.id === 1)?.report?.outputs).toEqual(["x.pdf"]);
    expect(jobs().find((j) => j.id === 2)?.error).toBe("falló");
    expect(runningJobs()).toHaveLength(0);
  });
});
