import { useState, useEffect, useRef, useCallback } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypeHighlight from "rehype-highlight";
import "highlight.js/styles/github-dark.css";
import hljs from "highlight.js/lib/core";
import typescript from "highlight.js/lib/languages/typescript";
import javascript from "highlight.js/lib/languages/javascript";
import python from "highlight.js/lib/languages/python";
import rust from "highlight.js/lib/languages/rust";
import bash from "highlight.js/lib/languages/bash";
import sql from "highlight.js/lib/languages/sql";
import json from "highlight.js/lib/languages/json";
import yaml from "highlight.js/lib/languages/yaml";
import css from "highlight.js/lib/languages/css";
import xml from "highlight.js/lib/languages/xml";
import go from "highlight.js/lib/languages/go";
import java from "highlight.js/lib/languages/java";
// Register only needed languages (reduces bundle ~300KB)
hljs.registerLanguage("typescript", typescript);
hljs.registerLanguage("javascript", javascript);
hljs.registerLanguage("python", python);
hljs.registerLanguage("rust", rust);
hljs.registerLanguage("bash", bash);
hljs.registerLanguage("shell", bash);
hljs.registerLanguage("sql", sql);
hljs.registerLanguage("json", json);
hljs.registerLanguage("yaml", yaml);
hljs.registerLanguage("css", css);
hljs.registerLanguage("html", xml);
hljs.registerLanguage("xml", xml);
hljs.registerLanguage("go", go);
hljs.registerLanguage("java", java);
import { Copy, Check, RotateCcw, Download, ChevronDown, ChevronRight, Tag, X, Pencil, Scissors } from "lucide-react";
import { useToast } from "../ui/Toast";
import type { SessionDetail, SessionSummary } from "../../types";
import { SaveSnippetModal } from "../snippets/SnippetCard";
import { api } from "../../api";

interface PreviewPaneProps {
  sessionId: string | null;
  onSelectSession?: (id: string) => void;
  onRename?: (id: string, newTitle: string) => void;
}

// CopyButton helper (used inline below)
function CopyDropdown({ content }: { content: string }) {
  const [open, setOpen] = useState(false);
  const [copied, setCopied] = useState(false);
  const toast = useToast();

  const copyAs = async (asMarkdown: boolean) => {
    let text = content;
    if (!asMarkdown) {
      text = content
        .replace(/#{1,6}\s/g, "")
        .replace(/\*\*(.*?)\*\*/g, "$1")
        .replace(/\*(.*?)\*/g, "$1")
        .replace(/`{3}[\s\S]*?`{3}/g, (m) => m.replace(/`{3}[^\n]*\n?/g, "").trim())
        .replace(/`([^`]+)`/g, "$1")
        .replace(/\[([^\]]+)\]\([^)]+\)/g, "$1")
        .replace(/^[-*+]\s/gm, "")
        .replace(/^\d+\.\s/gm, "");
    }
    await api.copyToClipboard(text);
    toast.success(asMarkdown ? "Markdownでコピーしました" : "プレーンテキストでコピーしました");
    setCopied(true);
    setOpen(false);
    setTimeout(() => setCopied(false), 1500);
  };

  return (
    <div className="relative">
      <div className="flex items-center rounded overflow-hidden" style={{ border: "1px solid var(--border)" }}>
        <button
          onClick={() => copyAs(true)}
          className="flex items-center gap-1 text-xs px-2 py-1 cursor-pointer"
          style={{ color: "var(--text-secondary)" }}
        >
          {copied ? <Check size={12} style={{ color: "var(--accent)" }} /> : <Copy size={12} />}
          {copied ? "Copied!" : "Copy"}
        </button>
        <button
          onClick={() => setOpen(!open)}
          className="px-1 py-1 cursor-pointer"
          style={{
            color: "var(--text-muted)",
            borderLeft: "1px solid var(--border)",
          }}
          aria-label="Copy format options"
        >
          <ChevronDown size={12} />
        </button>
      </div>

      {open && (
        <>
          <div className="fixed inset-0 z-10" onClick={() => setOpen(false)} />
          <div
            className="absolute right-0 mt-1 w-40 rounded shadow-lg z-20 py-1"
            style={{ background: "var(--surface)", border: "1px solid var(--border)" }}
          >
            <button
              onClick={() => copyAs(true)}
              className="w-full text-left px-3 py-1.5 text-xs cursor-pointer"
              style={{ color: "var(--text-primary)" }}
            >
              Copy as Markdown
            </button>
            <button
              onClick={() => copyAs(false)}
              className="w-full text-left px-3 py-1.5 text-xs cursor-pointer"
              style={{ color: "var(--text-primary)" }}
            >
              Copy as Plain Text
            </button>
          </div>
        </>
      )}
    </div>
  );
}

// 仮想化メッセージリスト（スクロールコンテナ込み）
function VirtualMessageList({
  messages,
  scrollRef,
  onSaveSnippet,
}: {
  messages: Array<{ role: string; content: string }>;
  scrollRef: React.RefObject<HTMLDivElement | null>;
  onSaveSnippet?: (code: string) => void;
}) {
  const virtualizer = useVirtualizer({
    count: messages.length,
    getScrollElement: () => scrollRef.current as HTMLDivElement | null,
    estimateSize: useCallback((i: number) => {
      const len = messages[i]?.content?.length ?? 0;
      return Math.max(80, Math.min(800, 80 + Math.floor(len / 80) * 20));
    }, [messages]),
    overscan: 3,
  });

  const items = virtualizer.getVirtualItems();

  return (
    <div
      ref={scrollRef as React.RefObject<HTMLDivElement>}
      className="flex-1 overflow-auto px-4 py-4"
    >
      {messages.length === 0 ? (
        <p className="text-sm text-center mt-8" style={{ color: "var(--text-muted)" }}>
          No messages
        </p>
      ) : (
        <div
          style={{
            height: `${virtualizer.getTotalSize()}px`,
            width: "100%",
            position: "relative",
          }}
        >
          {items.map((item) => {
            const msg = messages[item.index];
            return (
              <div
                key={item.key}
                data-index={item.index}
                ref={virtualizer.measureElement}
                style={{
                  position: "absolute",
                  top: 0,
                  left: 0,
                  width: "100%",
                  transform: `translateY(${item.start}px)`,
                }}
              >
                <MessageBubble role={msg.role as "User" | "Kiro"} content={msg.content} onSaveSnippet={onSaveSnippet} />
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}

function MessageBubble({
  role, content, onSaveSnippet,
}: {
  role: "User" | "Kiro";
  content: string;
  onSaveSnippet?: (code: string) => void;
}) {
  const [showCopy, setShowCopy] = useState(false);
  const [open, setOpen] = useState(false);
  const [copied, setCopied] = useState(false);
  // IntersectionObserver による遅延ハイライト
  // ビューポートに入ってから初めてrehypeHighlightを有効化
  const [visible, setVisible] = useState(false);
  const bubbleRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const el = bubbleRef.current;
    if (!el) return;
    // IntersectionObserver が未対応の環境（古い WebView / テスト環境）では即 visible=true
    if (typeof IntersectionObserver === "undefined") {
      setVisible(true);
      return;
    }
    const observer = new IntersectionObserver(
      ([entry]) => { if (entry.isIntersecting) { setVisible(true); observer.disconnect(); } },
      { rootMargin: "200px" }
    );
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  const copyAs = async (asMarkdown: boolean) => {
    let text = content;
    if (!asMarkdown) {
      text = content
        .replace(/#{1,6}\s/g, "")
        .replace(/\*\*(.*?)\*\*/g, "$1")
        .replace(/`{3}[\s\S]*?`{3}/g, (m) => m.replace(/`{3}[^\n]*\n?/g, "").trim())
        .replace(/`([^`]+)`/g, "$1")
        .replace(/\[([^\]]+)\]\([^)]+\)/g, "$1");
    }
    await api.copyToClipboard(text);
    setCopied(true);
    setOpen(false);
    setTimeout(() => setCopied(false), 1500);
  };

  const isUser = role === "User";

  return (
    <div
      ref={bubbleRef}
      className={`flex ${isUser ? "justify-end" : "justify-start"} mb-4 group`}
      onMouseEnter={() => setShowCopy(true)}
      onMouseLeave={() => { setShowCopy(false); setOpen(false); }}
    >
      <div
        className="relative max-w-[85%] rounded-lg px-4 py-3"
        style={{
          background: isUser ? "rgba(34,197,94,0.15)" : "var(--surface)",
          border: isUser ? "1px solid rgba(34,197,94,0.3)" : "1px solid var(--border)",
        }}
      >
        {/* Role label */}
        <div className="flex items-center justify-between mb-2 gap-3">
          <span
            className="text-xs font-semibold uppercase tracking-wider"
            style={{ color: isUser ? "var(--accent)" : "var(--text-muted)", fontFamily: "'JetBrains Mono', monospace" }}
          >
            {role}
          </span>
          {showCopy && (
            <div className="relative flex items-center gap-0">
              <button
                onClick={() => copyAs(true)}
                className="flex items-center gap-1 text-xs px-1.5 py-0.5 rounded cursor-pointer"
                style={{ color: "var(--text-muted)" }}
                title="Copy message"
              >
                {copied ? <Check size={11} style={{ color: "var(--accent)" }} /> : <Copy size={11} />}
              </button>
              <button
                onClick={() => setOpen(!open)}
                className="text-xs px-0.5 py-0.5 cursor-pointer"
                style={{ color: "var(--text-muted)" }}
              >
                <ChevronDown size={10} />
              </button>
              {open && (
                <>
                  <div className="fixed inset-0 z-10" onClick={() => setOpen(false)} />
                  <div
                    className="absolute right-0 top-6 w-40 rounded shadow-lg z-20 py-1"
                    style={{ background: "var(--surface)", border: "1px solid var(--border)" }}
                  >
                    <button onClick={() => copyAs(true)} className="w-full text-left px-3 py-1.5 text-xs cursor-pointer" style={{ color: "var(--text-primary)" }}>Copy as Markdown</button>
                    <button onClick={() => copyAs(false)} className="w-full text-left px-3 py-1.5 text-xs cursor-pointer" style={{ color: "var(--text-primary)" }}>Copy as Plain Text</button>
                    <div style={{ height: 1, background: "var(--border)", margin: "2px 8px" }} />
                    <button
                      onClick={() => {
                        setOpen(false);
                        // メッセージ全文をそのままスニペットとして保存（Copy と同じ範囲）
                        onSaveSnippet?.(content);
                      }}
                      className="w-full text-left px-3 py-1.5 text-xs cursor-pointer flex items-center gap-2"
                      style={{ color: "var(--accent)" }}
                    >
                      <Scissors size={11} />
                      スニペットとして保存
                    </button>
                  </div>
                </>
              )}
            </div>
          )}
        </div>

        {/* Content */}
        {isUser ? (
          <p className="text-sm whitespace-pre-wrap" style={{ color: "var(--text-primary)" }}>
            {content}
          </p>
        ) : (
          <div
            className="prose prose-sm max-w-none"
            style={{
              color: "var(--text-primary)",
              "--tw-prose-body": "var(--text-primary)",
              "--tw-prose-headings": "var(--text-primary)",
              "--tw-prose-code": "var(--text-primary)",
            } as React.CSSProperties}
          >
            <ReactMarkdown
              remarkPlugins={[remarkGfm]}
              rehypePlugins={visible ? [[rehypeHighlight, { detect: true, ignoreMissing: true }]] : []}
              components={{
                pre: ({ children, ...props }) => {
                  const codeEl = (children as React.ReactElement)?.props;
                  const lang = codeEl?.className?.replace("language-", "") ?? "";
                  return (
                    <div className="relative group/code">
                      {lang && <span className="code-lang-badge">{lang}</span>}
                      <pre {...props} style={{ position: "relative" }}>{children}</pre>
                    </div>
                  );
                },
              }}
            >
              {content}
            </ReactMarkdown>
          </div>
        )}
      </div>
    </div>
  );
}

function TagEditor({ sessionId, tags, onUpdate }: { sessionId: string; tags: string[]; onUpdate: (tags: string[]) => void }) {
  const [input, setInput] = useState("");
  const [allTags, setAllTags] = useState<string[]>([]);
  const [filtered, setFiltered] = useState<string[]>([]);
  const [activeIdx, setActiveIdx] = useState(-1);
  const [open, setOpen] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  // 既存タグ一覧を取得
  useEffect(() => {
    api.getTagMetadata().then((metas) => {
      setAllTags(metas.map((m) => m.tag));
    }).catch(() => {});
  }, []);

  const applyTag = async (tag: string) => {
    const t = tag.startsWith("#") ? tag : `#${tag}`;
    if (!t || tags.includes(t) || tags.length >= 10) return;
    const newTags = [...tags, t];
    await api.setTags(sessionId, newTags);
    onUpdate(newTags);
    setInput("");
    setOpen(false);
    setActiveIdx(-1);
  };

  const removeTag = async (tag: string) => {
    const newTags = tags.filter((t) => t !== tag);
    await api.setTags(sessionId, newTags);
    onUpdate(newTags);
  };

  const handleInput = (val: string) => {
    setInput(val);
    setActiveIdx(-1);
    if (val.trim().length === 0) {
      setFiltered([]);
      setOpen(false);
      return;
    }
    const q = val.toLowerCase().replace(/^#/, "");
    const matches = allTags.filter(
      (t) => t.toLowerCase().replace(/^#/, "").includes(q) && !tags.includes(t)
    ).slice(0, 8);
    setFiltered(matches);
    setOpen(matches.length > 0);
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setActiveIdx((i) => Math.min(i + 1, filtered.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setActiveIdx((i) => Math.max(i - 1, -1));
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (activeIdx >= 0 && filtered[activeIdx]) {
        applyTag(filtered[activeIdx]);
      } else {
        applyTag(input.trim());
      }
    } else if (e.key === "Escape") {
      setOpen(false);
      setActiveIdx(-1);
    }
  };

  return (
    <div className="flex items-center gap-1.5 flex-wrap">
      <Tag size={12} style={{ color: "var(--text-muted)" }} />
      {tags.map((tag) => (
        <span
          key={tag}
          className="flex items-center gap-1 text-xs px-2 py-0.5 rounded-full"
          style={{ background: "var(--border)", color: "var(--text-secondary)" }}
        >
          {tag}
          <button onClick={() => removeTag(tag)} className="cursor-pointer" style={{ color: "var(--text-muted)" }}>
            <X size={10} />
          </button>
        </span>
      ))}

      {/* 入力 + ドロップダウン */}
      <div className="relative">
        <input
          ref={inputRef}
          value={input}
          onChange={(e) => handleInput(e.target.value)}
          onKeyDown={handleKeyDown}
          onBlur={() => setTimeout(() => setOpen(false), 150)}
          onFocus={() => input.trim() && setOpen(filtered.length > 0)}
          placeholder="Add tag..."
          className="text-xs bg-transparent outline-none"
          style={{ color: "var(--text-primary)", width: "80px" }}
          aria-label="Add tag"
          aria-autocomplete="list"
          aria-expanded={open}
        />
        {open && (
          <ul
            role="listbox"
            className="absolute left-0 z-50 rounded-lg shadow-xl overflow-hidden"
            style={{
              top: "calc(100% + 4px)",
              minWidth: "160px",
              background: "var(--surface)",
              border: "1px solid var(--border)",
            }}
          >
            {filtered.map((tag, i) => (
              <li
                key={tag}
                role="option"
                aria-selected={i === activeIdx}
                onMouseDown={() => applyTag(tag)}
                onMouseEnter={() => setActiveIdx(i)}
                className="px-3 py-1.5 text-xs cursor-pointer"
                style={{
                  color: "var(--text-primary)",
                  background: i === activeIdx ? "var(--accent)" : "transparent",
                  opacity: i === activeIdx ? 1 : 0.85,
                }}
              >
                {tag}
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}

function TagSuggestions({ sessionId, currentTags, onAdd }: {
  sessionId: string;
  currentTags: string[];
  onAdd: (tag: string) => void;
}) {
  const [suggestions, setSuggestions] = useState<string[]>([]);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    api.suggestTags(sessionId).then((s) => {
      if (!cancelled) setSuggestions(s.filter((t) => !currentTags.includes(t)));
    }).catch(() => {}).finally(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, [sessionId, currentTags.join(",")]);

  if (loading || suggestions.length === 0) return null;

  return (
    <div className="flex items-center gap-1.5 flex-wrap mt-1">
      <span className="text-xs" style={{ color: "var(--text-muted)", flexShrink: 0 }}>提案:</span>
      {suggestions.map((tag) => (
        <button
          key={tag}
          onClick={() => onAdd(tag)}
          className="flex items-center gap-1 text-xs px-2 py-0.5 rounded-full cursor-pointer transition-colors"
          style={{
            background: "var(--border)",
            color: "var(--text-muted)",
            border: "1px dashed var(--border)",
          }}
          onMouseEnter={(e) => {
            e.currentTarget.style.borderColor = "var(--accent)";
            e.currentTarget.style.color = "var(--accent)";
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.borderColor = "var(--border)";
            e.currentTarget.style.color = "var(--text-muted)";
          }}
          title={`${tag} を追加`}
        >
          + {tag}
        </button>
      ))}
    </div>
  );
}

export function PreviewPane({ sessionId, onSelectSession, onRename }: PreviewPaneProps) {
  const toast = useToast();
  const [detail, setDetail] = useState<SessionDetail | null>(null);
  const [related, setRelated] = useState<SessionSummary[]>([]);
  const [tags, setTags] = useState<string[]>([]);
  const [showRelated, setShowRelated] = useState(false);
  const [viewMode, setViewMode] = useState<"all" | "user" | "kiro">("all");
  const scrollRef = useRef<HTMLDivElement | null>(null);
  // スニペット保存モーダル（仮想化リスト外で管理して再マウント問題を回避）
  const [snippetToSave, setSnippetToSave] = useState<string | null>(null);

  useEffect(() => {
    if (!sessionId) {
      setDetail(null);
      return;
    }
    let cancelled = false;
    api.getSessionDetail(sessionId).then((d) => {
      if (!cancelled) {
        setDetail(d);
        setTags(d.summary.tags);
        scrollRef.current?.scrollTo({ top: 0 });
        // Load related
        api.getRelatedSessions(d.summary.cwd, sessionId).then((r) => {
          if (!cancelled) setRelated(r);
        });
      }
    });
    return () => { cancelled = true; };
  }, [sessionId]);

  // Ctrl+R = Resume, Ctrl+Y = Copy
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (!detail) return;
      if ((e.ctrlKey || e.metaKey) && e.key === "r") {
        e.preventDefault();
        api.resumeSession(detail.summary.session_id, detail.summary.cwd).catch(async () => {
          const cmd = `cd "${detail.summary.cwd}" && kiro-cli chat --resume-id ${detail.summary.session_id}`;
          await api.copyToClipboard(cmd);
        });
      }
      if ((e.ctrlKey || e.metaKey) && e.key === "y") {
        e.preventDefault();
        const text = detail.messages
          .filter((m) => {
            if (viewMode === "user") return m.role === "User";
            if (viewMode === "kiro") return m.role === "Kiro";
            return true;
          })
          .map((m) => `[${m.role}]\n${m.content}`)
          .join("\n\n---\n\n");
        api.copyToClipboard(text);
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [detail, viewMode]);

  if (!sessionId || !detail) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <p className="text-sm" style={{ color: "var(--text-muted)" }}>
          Select a session to preview
        </p>
      </div>
    );
  }

  const { summary, messages } = detail;

  // Filter messages by viewMode
  const filteredMessages = messages.filter((m) => {
    if (viewMode === "user") return m.role === "User";
    if (viewMode === "kiro") return m.role === "Kiro";
    return true;
  });

  const fullText = filteredMessages
    .map((m) => `[${m.role}]\n${m.content}`)
    .join("\n\n---\n\n");

  return (
    <div className="flex-1 flex flex-col overflow-hidden">
      {/* Header */}
      <div className="flex-shrink-0 px-4 py-3" style={{ borderBottom: "1px solid var(--border)" }}>
        <PreviewTitle
          sessionId={summary.session_id}
          title={summary.title || "Untitled"}
          onRename={(newTitle) => {
            if (detail) setDetail({ ...detail, summary: { ...detail.summary, title: newTitle } });
            // セッションリスト側にも反映
            if (sessionId) onRename?.(sessionId, newTitle);
          }}
        />
        <div className="flex items-center gap-3 text-xs flex-wrap" style={{ color: "var(--text-muted)" }}>
          <span title="Directory">{summary.cwd}</span>
          <span>{new Date(summary.updated_at).toLocaleDateString("ja-JP")}</span>
          <span>{summary.message_count} messages</span>
          {summary.total_duration_secs > 0 && (
            <span>{Math.round(summary.total_duration_secs / 60)}m</span>
          )}
          {summary.max_context_pct && (
            <span style={{ color: summary.max_context_pct >= 80 ? "#F59E0B" : "inherit" }}>
              ctx {summary.max_context_pct.toFixed(1)}%
            </span>
          )}
        </div>
        <div className="mt-2 flex flex-col gap-1">
          <div className="flex items-center justify-between gap-2">
            <TagEditor sessionId={summary.session_id} tags={tags} onUpdate={setTags} />
          <TagSuggestions
            sessionId={summary.session_id}
            currentTags={tags}
            onAdd={async (tag) => {
              const newTags = [...tags, tag];
              await api.setTags(summary.session_id, newTags);
              setTags(newTags);
            }}
          />
          </div>
          {/* View mode toggle */}
          <div
            className="flex items-center rounded overflow-hidden flex-shrink-0"
            style={{ border: "1px solid var(--border)" }}
          >
            {(["all", "user", "kiro"] as const).map((mode) => {
              const labels = { all: "All", user: "User", kiro: "Kiro" };
              const isActive = viewMode === mode;
              return (
                <button
                  key={mode}
                  onClick={() => setViewMode(mode)}
                  className="text-xs px-2 py-1 cursor-pointer"
                  style={{
                    background: isActive ? "var(--accent)" : "transparent",
                    color: isActive ? "#000" : "var(--text-muted)",
                    fontWeight: isActive ? 600 : 400,
                  }}
                >
                  {labels[mode]}
                </button>
              );
            })}
          </div>
        </div>
      </div>

      {/* Messages - 仮想化スクロール（scrollRef はコンポーネント内でスクロールコンテナを兼ねる） */}
      <VirtualMessageList
        messages={filteredMessages}
        scrollRef={scrollRef}
        onSaveSnippet={(code) => setSnippetToSave(code)}
      />

      {/* Related sessions */}
      {related.length > 0 && (
        <div className="flex-shrink-0 px-4 py-2" style={{ borderTop: "1px solid var(--border)" }}>
          <button
            onClick={() => setShowRelated(!showRelated)}
            className="flex items-center gap-1 text-xs cursor-pointer"
            style={{ color: "var(--text-muted)" }}
          >
            {showRelated ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
            Related sessions ({related.length})
          </button>
          {showRelated && (
            <div className="mt-1 space-y-1">
              {related.map((r) => (
                <button
                  key={r.session_id}
                  onClick={() => onSelectSession?.(r.session_id)}
                  className="w-full text-left px-2 py-1.5 rounded text-xs cursor-pointer"
                  style={{
                    color: "var(--text-secondary)",
                    background: "transparent",
                  }}
                  onMouseEnter={(e) =>
                    (e.currentTarget.style.background = "var(--surface-hover)")
                  }
                  onMouseLeave={(e) =>
                    (e.currentTarget.style.background = "transparent")
                  }
                  title={r.session_id}
                >
                  <div className="truncate font-medium">{r.title || "Untitled"}</div>
                  <div className="text-xs mt-0.5" style={{ color: "var(--text-muted)" }}>
                    {new Date(r.updated_at).toLocaleDateString("ja-JP")}
                    {" · "}
                    {r.message_count} msgs
                  </div>
                </button>
              ))}
            </div>
          )}
        </div>
      )}

      {/* Footer */}
      <div
        className="flex-shrink-0 px-4 py-2"
        style={{ borderTop: "1px solid var(--border)", background: "var(--surface)" }}
      >
        {/* Resume row */}
        <div className="flex items-center gap-2 flex-wrap">
          <button
            onClick={async () => {
              try {
                await api.resumeSession(summary.session_id, summary.cwd);
                toast.success("kiro-cli を起動しました");
              } catch (e) {
                const cmd = `cd "${summary.cwd}" && kiro-cli chat --resume-id ${summary.session_id}`;
                await api.copyToClipboard(cmd);
                toast.error("ターミナルを起動できませんでした。コマンドをクリップボードにコピーしました。");
              }
            }}
            className="flex items-center gap-1.5 text-xs px-3 py-1.5 rounded cursor-pointer font-medium flex-shrink-0"
            style={{ background: "var(--accent)", color: "#000" }}
            title="Resume session (Ctrl+R)"
          >
            <RotateCcw size={12} />
            Resume
          </button>

          {/* Resume command display */}
          <ResumeCommandBar sessionId={summary.session_id} />
        </div>

        {/* Export / Copy row */}
        <div className="flex items-center justify-end gap-2 mt-2">
          <ExportDropdown sessionId={summary.session_id} />
          <CopyDropdown content={fullText} />
        </div>
      </div>

      {/* スニペット保存モーダル（仮想化リスト外でマウント） */}
      {snippetToSave !== null && (
        <SaveSnippetModal
          code={snippetToSave}
          language="markdown"
          sessionId={detail?.summary.session_id}
          sessionTitle={detail?.summary.title}
          onClose={() => setSnippetToSave(null)}
          onSaved={() => setSnippetToSave(null)}
        />
      )}
    </div>
  );
}

function PreviewTitle({
  sessionId,
  title,
  onRename,
}: {
  sessionId: string;
  title: string;
  onRename: (newTitle: string) => void;
}) {
  const [editing, setEditing] = useState(false);
  const divRef = useRef<HTMLDivElement>(null);
  const toast = useToast();

  const startEdit = () => {
    setEditing(true);
    setTimeout(() => {
      if (divRef.current) {
        divRef.current.focus();
        const range = document.createRange();
        range.selectNodeContents(divRef.current);
        const sel = window.getSelection();
        sel?.removeAllRanges();
        sel?.addRange(range);
      }
    }, 0);
  };

  const commit = async () => {
    const trimmed = (divRef.current?.innerText ?? "").trim();
    if (trimmed && trimmed !== title) {
      try {
        await api.renameSession(sessionId, trimmed);
        onRename(trimmed);
      } catch (err) {
        toast.error(`リネームに失敗しました: ${err}`);
      }
    }
    setEditing(false);
  };

  const cancel = () => setEditing(false);

  if (editing) {
    return (
      <div className="flex items-center gap-2 mb-1 w-full">
        <div
          ref={divRef}
          contentEditable
          suppressContentEditableWarning
          onKeyDown={(e) => {
            if (e.nativeEvent.isComposing) return;
            if (e.key === "Enter") { e.preventDefault(); commit(); }
            if (e.key === "Escape") cancel();
          }}
          className="flex-1 text-base font-semibold rounded px-2 py-0.5 outline-none"
          style={{
            background: "var(--bg)",
            border: "1px solid var(--accent)",
            color: "var(--text-primary)",
            minHeight: "1.75rem",
            lineHeight: "1.75rem",
            whiteSpace: "nowrap",
            overflow: "hidden",
          }}
          aria-label="タイトルを編集"
        >
          {title}
        </div>
        <button
          onClick={commit}
          className="text-xs px-2 py-1 rounded cursor-pointer flex-shrink-0 font-semibold"
          style={{ background: "var(--accent)", color: "#000" }}
          title="確定 (Enter)"
        >✓</button>
        <button
          onClick={cancel}
          className="text-xs px-2 py-1 rounded cursor-pointer flex-shrink-0"
          style={{ background: "var(--border)", color: "var(--text-muted)" }}
          title="キャンセル (Esc)"
        >✕</button>
      </div>
    );
  }

  return (
    <div className="flex items-center gap-2 mb-1 group/title">
      <h2 className="text-base font-semibold" style={{ color: "var(--text-primary)" }}>
        {title}
      </h2>
      <button
        onClick={startEdit}
        className="opacity-0 group-hover/title:opacity-100 cursor-pointer"
        style={{ color: "var(--text-muted)" }}
        title="名前を変更"
        aria-label="タイトルを編集"
      >
        <Pencil size={13} />
      </button>
    </div>
  );
}

function ResumeCommandBar({ sessionId }: { sessionId: string }) {
  const [copied, setCopied] = useState(false);
  const cmd = `kiro-cli chat --resume-id ${sessionId}`;
  // Show only the flag + short ID for display
  const shortId = sessionId.slice(0, 8) + "…";
  const displayCmd = `kiro-cli chat --resume-id ${shortId}`;

  const copy = async () => {
    await api.copyToClipboard(cmd);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  };

  return (
    <div
      className="flex items-center gap-1.5 rounded px-2 py-1 flex-shrink-0"
      style={{ background: "var(--bg)", border: "1px solid var(--border)" }}
      title={cmd}
    >
      <code
        className="text-xs whitespace-nowrap"
        style={{ color: "var(--text-muted)", fontFamily: "'JetBrains Mono', monospace" }}
      >
        {displayCmd}
      </code>
      <button
        onClick={copy}
        className="flex items-center flex-shrink-0 cursor-pointer"
        style={{ color: "var(--text-muted)" }}
        title="Copy full command"
        aria-label="Copy resume command"
      >
        {copied ? (
          <Check size={12} style={{ color: "var(--accent)" }} />
        ) : (
          <Copy size={12} />
        )}
      </button>
    </div>
  );
}

function ExportDropdown({ sessionId }: { sessionId: string }) {
  const [open, setOpen] = useState(false);

  const exportAs = async (format: "markdown" | "html" | "pdf") => {
    const { save } = await import("@tauri-apps/plugin-dialog");
    const ext = format === "markdown" ? "md" : "html";
    const path = await save({
      filters: [{ name: format.toUpperCase(), extensions: [ext] }],
    });
    if (path) {
      await api.exportSession(sessionId, format, path);
    }
    setOpen(false);
  };

  return (
    <div className="relative">
      <button
        onClick={() => setOpen(!open)}
        className="flex items-center gap-1.5 text-xs px-2 py-1.5 rounded cursor-pointer"
        style={{ background: "var(--border)", color: "var(--text-secondary)" }}
      >
        <Download size={12} />
        Export
        <ChevronDown size={10} />
      </button>
      {open && (
        <>
          <div className="fixed inset-0 z-10" onClick={() => setOpen(false)} />
          <div
            className="absolute right-0 bottom-8 w-36 rounded shadow-lg z-20 py-1"
            style={{ background: "var(--surface)", border: "1px solid var(--border)" }}
          >
            <button onClick={() => exportAs("markdown")} className="w-full text-left px-3 py-1.5 text-xs cursor-pointer" style={{ color: "var(--text-primary)" }}>Export as Markdown</button>
            <button onClick={() => exportAs("html")} className="w-full text-left px-3 py-1.5 text-xs cursor-pointer" style={{ color: "var(--text-primary)" }}>Export as HTML</button>
          </div>
        </>
      )}
    </div>
  );
}
