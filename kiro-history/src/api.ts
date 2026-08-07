import { invoke } from "@tauri-apps/api/core";
import { confirm } from "@tauri-apps/plugin-dialog";

export interface SnippetCollection {
  id: string;
  name: string;
  description: string;
  created_at: number;
  snippet_count: number;
}

export interface SnippetVersion {
  id: string;
  snippet_id: string;
  title: string;
  code: string;
  description: string;
  saved_at: number;
  note: string;
}

export interface ExportItem {
  title: string;
  description: string;
  language: string;
  code: string;
  tags: string[];
  starred: boolean;
  use_count: number;
  created_at: number;
}
import type {
  CodeSnippetWithSession,
  CreateTagParams,
  SavedSnippet,
  SaveSnippetParams,
  SimilarSnippet,
  SnippetSearchParams,
  SnippetStats,
  DiffResult,
  DuplicateGroup,
  ExportFormat,
  FileRef,
  FilterParams,
  IndexStats,
  SessionDetail,
  SessionSummary,
  SmartTagRule,
  StatsData,
  TagMeta,
  TagStat,
} from "./types";

export const api = {
  searchSessions: (query: string, limit?: number, filters?: FilterParams) =>
    invoke<SessionSummary[]>("search_sessions", { query, limit, filters }),

  getSessionDetail: (sessionId: string) =>
    invoke<SessionDetail>("get_session_detail", { sessionId }),

  getRelatedSessions: (cwd: string, excludeId: string) =>
    invoke<SessionSummary[]>("get_related_sessions", { cwd, excludeId }),

  rebuildIndex: () => invoke<void>("rebuild_index"),

  getIndexStats: () => invoke<IndexStats>("get_index_stats"),

  copyToClipboard: (text: string) =>
    invoke<void>("copy_to_clipboard", { text }),

  resumeSession: (sessionId: string, cwd: string) =>
    invoke<void>("resume_session", { sessionId, cwd }),

  toggleBookmark: (sessionId: string) =>
    invoke<boolean>("toggle_bookmark", { sessionId }),

  setTags: (sessionId: string, tags: string[]) =>
    invoke<void>("set_tags", { sessionId, tags }),

  getAllTags: () => invoke<TagStat[]>("get_all_tags"),

  getBookmarkedSessions: () => invoke<SessionSummary[]>("get_bookmarked_sessions"),

  getStats: () => invoke<StatsData>("get_stats"),

  getSnippets: (sessionId: string) =>
    invoke<{ language: string; code: string }[]>("get_snippets", { sessionId }),

  getAllSnippets: (query?: string, langFilter?: string) =>
    invoke<CodeSnippetWithSession[]>("get_all_snippets", {
      query: query ?? "",
      langFilter: langFilter ?? "",
    }),

  getFileRefs: (sessionId: string) =>
    invoke<FileRef[]>("get_file_refs", { sessionId }),

  openInEditor: (path: string) => invoke<void>("open_in_editor", { path }),

  getSessionDiff: (sessionIdA: string, sessionIdB: string) =>
    invoke<DiffResult>("get_session_diff", { sessionIdA, sessionIdB }),

  exportSession: (sessionId: string, format: ExportFormat, outputPath: string) =>
    invoke<void>("export_session_cmd", { sessionId, format, outputPath }),

  exportSessionsZip: (
    sessionIds: string[],
    format: ExportFormat,
    outputPath: string
  ) => invoke<void>("export_sessions_zip_cmd", { sessionIds, format, outputPath }),

  deleteSession: (sessionId: string) =>
    invoke<void>("delete_session", { sessionId }),

  deleteSessionsFiles: (sessionIds: string[]) =>
    invoke<{ deleted: string[]; skipped: { session_id: string; reason: string }[] }>(
      "delete_sessions_files",
      { sessionIds }
    ),

  renameSession: (sessionId: string, newTitle: string) =>
    invoke<void>("rename_session", { sessionId, newTitle }),

  // ── Saved Snippets ───────────────────────────────────────────────────────
  saveSnippet: (params: SaveSnippetParams) =>
    invoke<SavedSnippet>("save_snippet", { params }),

  updateSnippet: (id: string, title: string, description: string, language: string, code: string, tags: string[]) =>
    invoke<void>("update_snippet", { id, title, description, language, code, tags }),

  deleteSnippet: (id: string) =>
    invoke<void>("delete_snippet", { id }),

  toggleSnippetStar: (id: string) =>
    invoke<boolean>("toggle_snippet_star", { id }),

  incrementSnippetUse: (id: string) =>
    invoke<void>("increment_snippet_use", { id }),

  searchSavedSnippets: (searchParams: SnippetSearchParams) =>
    invoke<SavedSnippet[]>("search_saved_snippets", { searchParams }),

  findSimilarSnippets: (code: string, language: string, excludeId?: string) =>
    invoke<SimilarSnippet[]>("find_similar_snippets", { code, language, excludeId }),

  suggestSnippetTitle: (language: string, code: string) =>
    invoke<string>("suggest_snippet_title", { language, code }),

  getSnippetStats: () =>
    invoke<SnippetStats>("get_snippet_stats"),

  // ── Tag Management ───────────────────────────────────────────────────────
  getTagMetadata: () => invoke<TagMeta[]>("get_tag_metadata"),

  createTag: (params: CreateTagParams) =>
    invoke<void>("create_tag", { params }),

  updateTag: (tag: string, color: string, description: string) =>
    invoke<void>("update_tag", { tag, color, description }),

  deleteTagFull: (tag: string) =>
    invoke<number>("delete_tag_full", { tag }),

  renameTag: (oldTag: string, newTag: string) =>
    invoke<number>("rename_tag", { oldTag, newTag }),

  mergeTags: (fromTag: string, toTag: string) =>
    invoke<number>("merge_tags", { fromTag, toTag }),

  setTagOrder: (tags: string[]) =>
    invoke<void>("set_tag_order", { tags }),

  createSmartTag: (rule: SmartTagRule, color: string, description: string) =>
    invoke<void>("create_smart_tag", { rule, color, description }),

  getSessionsByTag: (tags: string[], mode: "AND" | "OR") =>
    invoke<SessionSummary[]>("get_sessions_by_tag", { tags, mode }),

  evaluateSmartTag: (ruleType: string, ruleValue: string) =>
    invoke<SessionSummary[]>("evaluate_smart_tag", { ruleType, ruleValue }),

  suggestTags: (sessionId: string) =>
    invoke<string[]>("suggest_tags", { sessionId }),

  // ── Config / WSL Path Detection ─────────────────────────────────────────────
  getConfig: () =>
    invoke<{ sessions_dir?: string; sqlite_db_path?: string; theme?: string; palette_shortcut_key?: string; palette_shortcut_enabled?: boolean }>("get_config"),
  saveConfig: (config: { sessions_dir?: string; sqlite_db_path?: string; theme?: string; palette_shortcut_key?: string; palette_shortcut_enabled?: boolean }) =>
    invoke<void>("save_config_cmd", { config }),
  detectWslPaths: () =>
    invoke<{ sessions_dir?: string; sqlite_db_path?: string; distro?: string }>("detect_wsl_paths"),
  getCurrentPaths: () =>
    invoke<{ sessions_dir: string; sqlite_db_path: string; index_db_path: string }>(
      "get_current_paths"
    ),

  searchSessionsCursor: (params: {
    query: string;
    limit?: number;
    filters?: import("./types").FilterParams;
    cursor_updated_at?: number;
    cursor_session_id?: string;
  }) => invoke<import("./types").SessionSummary[]>("search_sessions_cursor", { params }),

  getSnippetTags: () => invoke<Array<[string, number]>>("get_snippet_tags"),

  // ── コレクション ────────────────────────────────────────────────────────────
  listSnippetCollections: () => invoke<SnippetCollection[]>("list_snippet_collections"),
  createSnippetCollection: (name: string, description: string) =>
    invoke<SnippetCollection>("create_snippet_collection", { name, description }),
  deleteSnippetCollection: (id: string) =>
    invoke<void>("delete_snippet_collection", { id }),
  setSnippetCollection: (snippetId: string, collectionName: string) =>
    invoke<void>("set_snippet_collection", { snippetId, collectionName }),

  // ── バージョン履歴 ──────────────────────────────────────────────────────────
  listSnippetVersions: (snippetId: string) =>
    invoke<SnippetVersion[]>("list_snippet_versions", { snippetId }),
  restoreSnippetVersion: (versionId: string) =>
    invoke<SavedSnippet>("restore_snippet_version", { versionId }),
  snapshotSnippetVersion: (snippetId: string, note: string) =>
    invoke<void>("snapshot_snippet_version", { snippetId, note }),

  // ── インポート / エクスポート ────────────────────────────────────────────────
  exportSnippets: (ids?: string[]) =>
    invoke<ExportItem[]>("export_snippets", { ids: ids ?? null }),
  importSnippets: (items: ExportItem[], overwrite: boolean) =>
    invoke<[number, number]>("import_snippets", { items, overwrite }),

  // ── プリフェッチ ──────────────────────────────────────────────────────────────
  prefetchSession: (sessionId: string) =>
    invoke<void>("prefetch_session", { sessionId }),

  // ── モデル価格設定 ──────────────────────────────────────────────────────────────
  getModelPricesPath: () => invoke<string>("get_model_prices_path"),
  getModelPrices: () => invoke<{ last_updated: string; models: Array<{ pattern: string; input: number; output: number; ctx: number }> }>("get_model_prices"),
  reloadModelPrices: () => invoke<string>("reload_model_prices"),

  // ── ダイアログ ───────────────────────────────────────────────────────────────
  confirmDelete: (message: string) => confirm(message, { title: "削除の確認", kind: "warning" }),

  // ── Quick Palette & Cleanup ─────────────────────────────────────────────────
  quickSearchSnippets: (query: string) =>
    invoke<SavedSnippet[]>("quick_search_snippets", { query }),
  findDuplicateGroups: (threshold?: number) =>
    invoke<DuplicateGroup[]>("find_duplicate_groups", { threshold }),
  findUnusedSnippets: (days?: number) =>
    invoke<SavedSnippet[]>("find_unused_snippets", { days }),
  bulkDeleteSnippets: (ids: string[]) =>
    invoke<number>("bulk_delete_snippets", { ids }),
  mergeSnippets: (keepId: string, dropIds: string[]) =>
    invoke<void>("merge_snippets", { keepId, dropIds }),
};
