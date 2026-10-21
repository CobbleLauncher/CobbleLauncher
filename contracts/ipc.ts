/** Proposed renderer contract. No implementation or secret-bearing fields. */
export type Id = string;
export type TaskStatus = "queued" | "running" | "waiting_user" | "blocked_external" | "succeeded" | "failed" | "cancelled";
export interface RequestContext { request_id: Id }
export interface AccountPublic { id: Id; display_name: string; player_uuid: string | null; status: string }
export interface InstanceSummary { id: Id; name: string; minecraft_version: string; loader_label: string | null; state: "empty" | "ready" | "installing" | "running" | "blocked"; active_revision: Id | null }
export interface TaskSnapshot { schema_version: 1; task_id: Id; sequence: number; status: TaskStatus; phase: string; completed_bytes: number; total_bytes: number | null; error_code: string | null }
export interface CoreError { code: string; message_key: string; retryable: boolean; action: string | null; context: Record<string, string | number | boolean | null> }
export type Result<T> = { ok: true; value: T } | { ok: false; error: CoreError };
export interface PlanSummary { plan_id: Id; approval_digest: string; instance_id: Id; add_count: number; update_count: number; remove_count: number; download_bytes: number | null; risks: string[]; blocked_reasons: string[] }
export interface ImportSummary { import_handle: Id; format: "mrpack" | "curseforge_zip" | "local_mods"; name: string; minecraft_version: string; warnings: string[] }
export interface SessionSummary { session_id: Id; instance_id: Id; state: "preparing" | "spawned" | "running" | "exited"; exit_code: number | null }
export interface LauncherApi {
  listInstances(): Promise<Result<InstanceSummary[]>>;
  listAccounts(): Promise<Result<AccountPublic[]>>;
  startLogin(context: RequestContext): Promise<Result<{ flow_id: Id }>>;
  inspectImport(context: RequestContext, file_handle: Id): Promise<Result<ImportSummary>>;
  resolveImport(context: RequestContext, import_handle: Id, instance_id: Id): Promise<Result<PlanSummary>>;
  approvePlan(context: RequestContext, plan_id: Id, approval_digest: string): Promise<Result<{ task_id: Id }>>;
  launchInstance(context: RequestContext, instance_id: Id, account_id: Id): Promise<Result<{ task_id: Id }>>;
  stopSession(context: RequestContext, session_id: Id, force: boolean): Promise<Result<{ task_id: Id }>>;
  listSessions(): Promise<Result<SessionSummary[]>>;
  getTasks(): Promise<Result<TaskSnapshot[]>>;
  cancelTask(context: RequestContext, task_id: Id): Promise<Result<{ cancellation_requested: true }>>;
  openInstanceFolder(instance_id: Id): Promise<Result<null>>;
}
// Host owns picker/file handles, consent, tokens, paths, and authoritative state.
