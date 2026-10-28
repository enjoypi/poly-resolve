import type { connect } from "./client";
import { type Column, inferColumns, type Row } from "./columns";
import { MAX_ROWS, PAGE_LIMIT, TEXT, TIP_DELAY_MS } from "./config";
import { openDetail } from "./detail";
import type { Span } from "./range";
import { buildStatus, type Status } from "./status";
import { buildBody, buildHead, markSort, rowOf, tipOf } from "./table";
import { attachTip } from "./tip";
import { buildToolbar, type Toolbar } from "./toolbar";
import { applyView, nextSort, type State } from "./view";

type Client = ReturnType<typeof connect>;

function area(tag: string, className: string, ...children: Node[]): HTMLElement {
  const element = document.createElement(tag);
  element.className = className;
  element.append(...children);
  return element;
}

function merge(current: Row[], incoming: Row[]): Row[] {
  const seen = new Set(current.map((item) => item.key));
  return [...current, ...incoming.filter((item) => !seen.has(item.key))].slice(0, MAX_ROWS);
}

export class Board {
  private columns: Column[] = [];
  private cursor: string | undefined;
  private readonly detail = document.createElement("dialog");
  private headKey = "";
  private readonly head = document.createElement("thead");
  private menuKey = "";
  private readonly overrides = new Map<string, boolean>();
  private rows: Row[] = [];
  private readonly shell = area("div", "app");
  private span: Span = {};
  private state: State = { filters: {}, search: "" };
  private readonly status: Status;
  private readonly table = document.createElement("tbody");
  private readonly tip = area("div", "tip");
  private readonly toolbar: Toolbar;
  private visible: Column[] = [];

  constructor(
    private readonly client: Client,
    root: HTMLElement,
    private readonly timeZone?: string,
  ) {
    this.status = buildStatus({
      onAll: () => void this.loadAll(),
      onClear: () => this.clear(),
      onMore: () => void this.load(true),
    });
    this.toolbar = buildToolbar(
      {
        onColumn: (key, shown) => this.toggle(key, shown),
        onPanel: () => this.panel(),
        onRange: (span) => void this.ranged(span),
        onRefresh: () => void this.load(false),
        onSearch: (text) => this.search(text),
      },
      timeZone,
    );
    this.assemble(root);
  }

  private assemble(root: HTMLElement): void {
    const grid = document.createElement("table");
    grid.append(this.head, this.table);
    const scroller = area("div", "scroller", grid);
    this.detail.className = "detail";
    this.tip.hidden = true;
    this.tip.setAttribute("role", "tooltip");
    this.table.onclick = (event) => this.open(event.target);
    this.table.onkeydown = (event) => {
      if (event.key === "Enter" && event.target instanceof HTMLElement) this.open(event.target);
    };
    attachTip(
      scroller,
      this.tip,
      (cell) => tipOf(cell, this.visible, this.timeZone),
      TIP_DELAY_MS,
    );
    this.shell.append(this.toolbar.root, scroller, this.status.root, this.detail, this.tip);
    root.replaceChildren(this.shell);
  }

  private clear(): void {
    this.state = { filters: {}, search: "", sort: this.state.sort };
    this.toolbar.search.value = "";
    this.headKey = "";
    this.redraw();
  }

  private filtered(): boolean {
    const { filters, search } = this.state;
    return search.trim() !== "" || Object.values(filters).some((text) => text.trim() !== "");
  }

  private async load(append: boolean): Promise<boolean> {
    this.lock(true);
    let done = false;
    try {
      const body = {
        limit: PAGE_LIMIT,
        ...(append && this.cursor ? { cursor: this.cursor } : {}),
        ...this.span,
      };
      const { data, error } = await this.client.POST("/messages/list", { body });
      this.status.error.textContent = data ? "" : (error?.code ?? "");
      if (data) {
        this.rows = merge(append ? this.rows : [], data.items);
        this.cursor = data.next_cursor ?? undefined;
        done = true;
      }
    } catch {
      this.status.error.textContent = TEXT.unreachable;
    }
    this.redraw();
    this.lock(false);
    return done;
  }

  private async loadAll(): Promise<void> {
    this.lock(true);
    while (this.cursor !== undefined && this.rows.length < MAX_ROWS) {
      if (!(await this.load(true))) break;
    }
    this.lock(false);
  }

  private lock(locked: boolean): void {
    this.toolbar.refresh.disabled = locked;
    this.status.lock(locked);
  }

  private open(target: EventTarget | null): void {
    const element = target instanceof Element ? target.closest("tr") : null;
    const item = rowOf(element);
    if (element instanceof HTMLElement && item) {
      openDetail(this.detail, item, this.columns, element, this.timeZone);
    }
  }

  private panel(): void {
    const open = this.shell.classList.toggle("panel-open");
    this.toolbar.panel.setAttribute("aria-expanded", String(open));
  }

  private async ranged(span: Span): Promise<void> {
    this.span = span;
    await this.load(false);
  }

  private redraw(): void {
    this.columns = inferColumns(this.rows);
    this.visible = this.columns.filter((column) => this.shown(column));
    const headKey = this.visible.map((column) => column.key).join("\n");
    if (headKey !== this.headKey) {
      buildHead(
        this.head,
        this.visible,
        this.state,
        {
          onFilter: (key, text) => this.filter(key, text),
          onSort: (key) => this.sort(key),
        },
        this.timeZone,
      );
      this.headKey = headKey;
    }
    const menuKey = this.columns.map((column) => column.key).join("\n");
    if (menuKey !== this.menuKey) {
      this.toolbar.fill(this.columns, (column) => this.shown(column));
      this.menuKey = menuKey;
    }
    const view = applyView(this.rows, this.columns, this.state, this.timeZone);
    buildBody(this.table, this.visible, view, this.timeZone);
    this.status.update(
      {
        capped: this.cursor !== undefined && this.rows.length >= MAX_ROWS,
        filtered: this.filtered(),
        loaded: this.rows.length,
        more: this.cursor !== undefined,
        shown: view.length,
        span: this.span,
      },
      this.timeZone,
    );
  }

  private search(text: string): void {
    this.state = { ...this.state, search: text };
    this.redraw();
  }

  private shown(column: Column): boolean {
    return this.overrides.get(column.key) ?? !column.hidden;
  }

  private sort(key: string): void {
    this.state = { ...this.state, sort: nextSort(this.state.sort, key) };
    markSort(this.head, this.state);
    this.redraw();
  }

  private filter(key: string, text: string): void {
    this.state = { ...this.state, filters: { ...this.state.filters, [key]: text } };
    this.redraw();
  }

  private toggle(key: string, shown: boolean): void {
    this.overrides.set(key, shown);
    this.redraw();
  }

  start(): Promise<void> {
    return this.load(false).then(() => undefined);
  }
}
