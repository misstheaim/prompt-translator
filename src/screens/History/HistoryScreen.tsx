import { useEffect, useRef, useState } from "react";

import type { HistoryEntry } from "../../bindings/HistoryEntry";
import type { HistoryEntrySummary } from "../../bindings/HistoryEntrySummary";
import type { Mode } from "../../bindings/Mode";
import type { ProviderKind } from "../../bindings/ProviderKind";
import { Banner } from "../../components/Banner";
import { Button } from "../../components/Button";
import { ConfirmDialog } from "../../components/ConfirmDialog";
import { ErrorBanner } from "../../components/ErrorBanner";
import { Spinner } from "../../components/Spinner";
import {
  copyResult,
  historyDelete,
  historyGet,
  historyList,
  toAppError,
  type AppError,
} from "../../lib/api";
import { cx } from "../../lib/cx";
import { useAppStore } from "../../store/app";
import { useTranslateStore } from "../Translate/store";
import { IssueList } from "./IssueList";

const PAGE_SIZE = 50;
const DEBOUNCE_MS = 300;

const MODE_LABELS: Record<Mode, string> = {
  translate: "Translate",
  improve: "Improve",
  correct: "Correct",
};

const PROVIDER_LABELS: Record<ProviderKind, string> = {
  ollama: "Ollama",
  claudeCode: "Claude Code",
  mock: "Mock (dev)",
};

function formatDate(iso: string): string {
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? iso : d.toLocaleString();
}

function ModeBadge({ mode }: { mode: Mode }) {
  return (
    <span className="rounded bg-surface-raised px-1.5 py-0.5 text-xs font-medium text-muted">
      {MODE_LABELS[mode]}
    </span>
  );
}

export function HistoryScreen() {
  const settings = useAppStore((s) => s.settings);
  const setScreen = useAppStore((s) => s.setScreen);
  const showToast = useAppStore((s) => s.showToast);

  const [searchText, setSearchText] = useState("");
  const [query, setQuery] = useState("");
  const [items, setItems] = useState<HistoryEntrySummary[]>([]);
  const [hasMore, setHasMore] = useState(false);
  const [loadedQuery, setLoadedQuery] = useState<string | null>(null);
  const [moreLoading, setMoreLoading] = useState(false);
  const [reloadKey, setReloadKey] = useState(0);
  const [listError, setListError] = useState<AppError | null>(null);

  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [detail, setDetail] = useState<HistoryEntry | null>(null);
  const [detailError, setDetailError] = useState<AppError | null>(null);
  const [confirmDelete, setConfirmDelete] = useState(false);

  // Only the latest list/detail request may write its result.
  const listRequest = useRef(0);
  const detailRequest = useRef(0);

  // Debounce the search box.
  useEffect(() => {
    const t = window.setTimeout(() => setQuery(searchText.trim()), DEBOUNCE_MS);
    return () => window.clearTimeout(t);
  }, [searchText]);

  // The first page is loading while the list shows another query than the box.
  const loading = loadedQuery !== query || moreLoading;

  // First page for the current query (also reruns on retry).
  useEffect(() => {
    const id = ++listRequest.current;
    historyList({ query: query === "" ? null : query, limit: PAGE_SIZE, offset: 0 })
      .then((page) => {
        if (id !== listRequest.current) return;
        setItems(page);
        setHasMore(page.length === PAGE_SIZE);
        setListError(null);
        setLoadedQuery(query);
      })
      .catch((e: unknown) => {
        if (id !== listRequest.current) return;
        setListError(toAppError(e));
        setLoadedQuery(query);
      });
  }, [query, reloadKey]);

  function loadMore() {
    const id = ++listRequest.current;
    setMoreLoading(true);
    historyList({
      query: query === "" ? null : query,
      limit: PAGE_SIZE,
      offset: items.length,
    })
      .then((page) => {
        if (id !== listRequest.current) return;
        setItems((prev) => [...prev, ...page]);
        setHasMore(page.length === PAGE_SIZE);
      })
      .catch((e: unknown) => {
        if (id === listRequest.current) setListError(toAppError(e));
      })
      .finally(() => {
        if (id === listRequest.current) setMoreLoading(false);
      });
  }

  function retry() {
    setLoadedQuery(null);
    setReloadKey((k) => k + 1);
  }

  function select(id: number) {
    setSelectedId(id);
    setDetail(null);
    setDetailError(null);
    const req = ++detailRequest.current;
    historyGet(id)
      .then((entry) => {
        if (req === detailRequest.current) setDetail(entry);
      })
      .catch((e: unknown) => {
        if (req === detailRequest.current) setDetailError(toAppError(e));
      });
  }

  async function onCopy() {
    if (!detail) return;
    try {
      await copyResult(detail.outputText, detail.id);
      showToast("success", "Copied to clipboard");
    } catch (e) {
      showToast("error", toAppError(e).message);
    }
  }

  async function onDelete() {
    if (!detail) return;
    const id = detail.id;
    setConfirmDelete(false);
    try {
      await historyDelete(id);
      setItems((prev) => prev.filter((x) => x.id !== id));
      setSelectedId(null);
      setDetail(null);
      showToast("success", "Entry deleted");
    } catch (e) {
      showToast("error", toAppError(e).message);
    }
  }

  function onOpenInTranslate() {
    if (!detail) return;
    useTranslateStore.setState({
      input: detail.sourceText,
      mode: detail.mode,
      provider: detail.provider,
      status: "success",
      requestId: null,
      result: {
        historyId: detail.id,
        output: detail.outputText,
        issues: detail.issues,
        tokenWarnings: detail.tokenWarnings,
        provider: detail.provider,
        model: detail.model,
        durationMs: detail.durationMs,
      },
      output: detail.outputText,
      error: null,
    });
    setScreen("translate");
  }

  const historyDisabled = settings !== null && !settings.historyEnabled;
  const searching = query !== "";

  return (
    <div className="mx-auto flex max-w-5xl flex-col gap-4">
      <div className="flex items-center gap-3">
        <h1 className="text-lg font-semibold text-text">History</h1>
        <input
          type="search"
          value={searchText}
          onChange={(e) => setSearchText(e.target.value)}
          placeholder="Search source and output"
          aria-label="Search history"
          className="h-8 min-w-0 flex-1 rounded-md border border-border bg-surface px-3 text-sm text-text placeholder:text-muted focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
        />
      </div>

      {historyDisabled && (
        <Banner
          variant="info"
          actions={<Button onClick={() => setScreen("settings")}>Open settings</Button>}
        >
          History is turned off, so new translations aren't saved. Existing entries stay here.
        </Banner>
      )}

      {listError && <ErrorBanner error={listError} onRetry={retry} />}

      <div className="grid gap-4 md:grid-cols-[minmax(0,1fr)_minmax(0,1.2fr)]">
        <section aria-label="Entries" className="flex min-w-0 flex-col gap-2">
          {items.length === 0 && !loading && !listError && (
            <p className="py-8 text-center text-muted">
              {searching ? "No matching entries" : "No history yet"}
            </p>
          )}
          <ul className="flex flex-col gap-2">
            {items.map((item) => (
              <li key={item.id}>
                <button
                  type="button"
                  aria-current={item.id === selectedId ? "true" : undefined}
                  onClick={() => select(item.id)}
                  className={cx(
                    "flex w-full flex-col gap-1 rounded-md border bg-surface p-2 text-left text-sm transition-colors duration-150 hover:bg-surface-raised focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent motion-reduce:transition-none",
                    item.id === selectedId ? "border-accent" : "border-border",
                  )}
                >
                  <span className="flex items-center gap-2 text-xs text-muted">
                    <ModeBadge mode={item.mode} />
                    <span>{PROVIDER_LABELS[item.provider]}</span>
                    <span className="ml-auto">{formatDate(item.createdAt)}</span>
                  </span>
                  <span className="truncate text-text">{item.sourcePreview}</span>
                  <span className="truncate text-muted">
                    <span aria-hidden="true">→ </span>
                    {item.outputPreview}
                  </span>
                </button>
              </li>
            ))}
          </ul>
          {loading && (
            <div className="flex justify-center py-2">
              <Spinner />
            </div>
          )}
          {hasMore && !loading && <Button onClick={loadMore}>Load more</Button>}
        </section>

        <section aria-label="Entry details" className="min-w-0">
          {detailError && <ErrorBanner error={detailError} />}
          {!detail && !detailError && (
            <p className="py-8 text-center text-muted">Select an entry to see it in full.</p>
          )}
          {detail && (
            <div className="flex flex-col gap-3 rounded-md border border-border bg-surface p-3">
              <div className="flex flex-wrap items-center gap-2 text-xs text-muted">
                <ModeBadge mode={detail.mode} />
                <span>
                  {PROVIDER_LABELS[detail.provider]} · {detail.model}
                </span>
                <span className="ml-auto">{formatDate(detail.createdAt)}</span>
              </div>

              <div>
                <h2 className="mb-1 text-xs font-semibold uppercase text-muted">Source</h2>
                <p className="whitespace-pre-wrap break-words text-sm text-text">
                  {detail.sourceText}
                </p>
              </div>
              <div>
                <h2 className="mb-1 text-xs font-semibold uppercase text-muted">Output</h2>
                <p className="whitespace-pre-wrap break-words text-sm text-text">
                  {detail.outputText}
                </p>
              </div>

              {detail.issues.length > 0 && (
                <div>
                  <h2 className="mb-1 text-xs font-semibold uppercase text-muted">Issues</h2>
                  <IssueList issues={detail.issues} />
                </div>
              )}

              {detail.tokenWarnings.length > 0 && (
                <Banner variant="warning">
                  Missing from the output: {detail.tokenWarnings.map((w) => w.token).join(", ")}
                </Banner>
              )}

              <div className="flex flex-wrap gap-2">
                <Button variant="primary" onClick={() => void onCopy()}>
                  Copy
                </Button>
                <Button onClick={onOpenInTranslate}>Open in Translate</Button>
                <Button variant="danger" className="ml-auto" onClick={() => setConfirmDelete(true)}>
                  Delete
                </Button>
              </div>
            </div>
          )}
        </section>
      </div>

      <ConfirmDialog
        open={confirmDelete}
        title="Delete this entry?"
        message="The entry and its issues are removed from the history. This can't be undone."
        confirmLabel="Delete"
        destructive
        onConfirm={() => void onDelete()}
        onCancel={() => setConfirmDelete(false)}
      />
    </div>
  );
}
