async function main() {
  const graph = await loadGraph();
  const reportData = readEmbeddedJson('report-data') || {};
  const reportConfig = readEmbeddedJson('report-config') || {};

  renderHeader(reportData);
  renderSections(graph, reportData, reportConfig);
}

async function loadGraph() {
  const embedded = document.getElementById('graph-data');
  if (embedded && embedded.textContent && embedded.textContent.trim().length > 0) {
    return JSON.parse(embedded.textContent);
  }
  // Fallback for server-based viewing.
  const graphRes = await fetch('./assets/graph.json');
  if (!graphRes.ok) throw new Error(`failed to fetch graph.json: ${graphRes.status}`);
  return await graphRes.json();
}

function readEmbeddedJson(id) {
  const el = document.getElementById(id);
  if (!el || !el.textContent) return null;
  const txt = el.textContent.trim();
  if (!txt) return null;
  return JSON.parse(txt);
}

function el(tag, attrs = {}, children = []) {
  const node = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (k === 'class') node.className = v;
    else if (k === 'text') node.textContent = v;
    else node.setAttribute(k, v);
  }
  for (const c of children) node.appendChild(c);
  return node;
}

function onVisibleOnce(element, callback) {
  if (!element) return;
  if (!('IntersectionObserver' in window)) {
    callback();
    return;
  }
  let done = false;
  const obs = new IntersectionObserver(
    (entries) => {
      for (const ent of entries) {
        if (done) break;
        if (ent.isIntersecting) {
          done = true;
          obs.disconnect();
          callback();
        }
      }
    },
    { threshold: 0.01 }
  );
  obs.observe(element);
}

function setupLazyGraphInit(sectionEl, selectEl, initGraph) {
  let initialized = false;

  function ensureInit() {
    if (initialized) return;
    initialized = true;
    requestAnimationFrame(() => {
      initGraph();
    });
  }

  // Render when the section becomes visible (if view is graph).
  onVisibleOnce(sectionEl, () => {
    if (!selectEl || selectEl.value === 'graph') {
      ensureInit();
    }
  });

  // Render when the user switches to graph view.
  if (selectEl) {
    selectEl.addEventListener('change', () => {
      if (selectEl.value === 'graph') {
        ensureInit();
      }
    });
  }
}

function renderHeader(reportData) {
  const titleEl = document.getElementById('report-title');
  const generatedEl = document.getElementById('report-generated-at');
  const meta = reportData && reportData.meta ? reportData.meta : null;

  const title = meta && meta.title ? String(meta.title) : 'Kiboku Report';
  if (titleEl) titleEl.textContent = title;
  // Also update the document title tag
  document.title = title;

  const ms = meta && typeof meta.generated_at_ms === 'number' ? meta.generated_at_ms : null;
  if (generatedEl) {
    if (ms != null) {
      const dt = new Date(ms);
      generatedEl.textContent = `Generated at: ${dt.toLocaleString()}`;
    } else {
      generatedEl.textContent = '';
    }
  }
}

function renderSections(graph, reportData, reportConfig) {
  const contentEl = document.getElementById('content');
  if (!contentEl) return;

  const sections = Array.isArray(reportConfig.sections)
    ? reportConfig.sections
    : [
      'package_summary',
      'workspace_dependencies',
      'external_dependencies',
      'findings',
      'findings_matrix',
      'external_libraries',
    ];
  const hidden = new Set(Array.isArray(reportConfig.hidden) ? reportConfig.hidden : []);
  const sectionHeights = reportConfig && typeof reportConfig.section_heights === 'object' && reportConfig.section_heights
    ? reportConfig.section_heights
    : {};

  contentEl.innerHTML = '';

  const SECTION_TITLES = {
    package_summary: 'Package summary',
    workspace_dependencies: 'Workspace package dependencies',
    external_dependencies: 'External library dependencies',
    findings: 'Findings',
    findings_matrix: 'Findings matrix (packages × findings)',
    external_libraries: 'External libraries',
  };

  function toAnchorId(sectionId) {
    const raw = String(sectionId || '').trim();
    const slug = raw
      .toLowerCase()
      .replace(/[^a-z0-9_-]+/g, '-')
      .replace(/^-+/, '')
      .replace(/-+$/, '');
    return `section-${slug || 'unknown'}`;
  }

  // Table of contents (links to the visible sections).
  {
    const items = [];
    for (const sectionId of sections) {
      if (hidden.has(sectionId)) continue;
      const title = SECTION_TITLES[sectionId] || `Unknown section: ${sectionId}`;
      items.push({ id: sectionId, anchor: toAnchorId(sectionId), title });
    }

    if (items.length > 0) {
      const list = el('ul', { class: 'toc__list' });
      for (const it of items) {
        list.appendChild(el('li', {}, [
          el('a', { class: 'toc__link', href: `#${it.anchor}`, text: it.title }),
        ]));
      }
      const toc = el('nav', { class: 'section toc', 'aria-label': 'Table of contents' }, [
        el('h2', { text: 'Contents' }),
        list,
      ]);
      contentEl.appendChild(toc);
    }
  }

  function applySectionHeight(sectionId, sectionEl) {
    if (!sectionEl || !sectionId) return;
    const raw = sectionHeights[sectionId];
    if (raw == null) return;
    let v = null;
    if (typeof raw === 'string') v = raw.trim();
    // Rust accepts both Integer and Float. Floats are rounded to an integer pixel value.
    // JavaScript has only one numeric type (Number), so "finite number" covers both cases.
    // Note: values emitted by the Rust report generator are expected to be strings, but we
    // keep number support for robustness (e.g., if report-config is authored manually).
    else if (typeof raw === 'number' && Number.isFinite(raw)) v = `${Math.round(raw)}px`;
    if (!v) return;
    sectionEl.style.setProperty('--section-panel-height', v);
  }

  for (const sectionId of sections) {
    if (hidden.has(sectionId)) continue;

    if (sectionId === 'package_summary') {
      const s = renderPackageSummarySection(reportData.package_summary || {});
      s.id = toAnchorId(sectionId);
      applySectionHeight(sectionId, s);
      contentEl.appendChild(s);
    } else if (sectionId === 'workspace_dependencies') {
      const s = renderWorkspaceDependenciesSection(graph);
      s.id = toAnchorId(sectionId);
      applySectionHeight(sectionId, s);
      contentEl.appendChild(s);
    } else if (sectionId === 'external_dependencies') {
      const s = renderExternalDependenciesSection(graph);
      s.id = toAnchorId(sectionId);
      applySectionHeight(sectionId, s);
      contentEl.appendChild(s);
    } else if (sectionId === 'findings') {
      const s = renderFindingsSection(reportData.findings || {});
      s.id = toAnchorId(sectionId);
      applySectionHeight(sectionId, s);
      contentEl.appendChild(s);
    } else if (sectionId === 'findings_matrix') {
      const s = renderFindingsMatrixSection(reportData.findings || {});
      s.id = toAnchorId(sectionId);
      applySectionHeight(sectionId, s);
      contentEl.appendChild(s);
    } else if (sectionId === 'external_libraries') {
      const s = renderExternalLibrariesSection(reportData.external_libraries || {});
      s.id = toAnchorId(sectionId);
      applySectionHeight(sectionId, s);
      contentEl.appendChild(s);
    } else {
      const s = el('section', { class: 'section' });
      s.id = toAnchorId(sectionId);
      s.appendChild(el('h2', { text: `Unknown section: ${sectionId}` }));
      applySectionHeight(sectionId, s);
      contentEl.appendChild(s);
    }
  }
}

function buildCytoscape(container, elements) {
  return cytoscape({
    container,
    elements,
    style: [
      {
        selector: 'node',
        style: {
          'label': 'data(label)',
          'font-size': 9,
          'text-wrap': 'wrap',
          'text-max-width': 140,
          'background-color': '#4f7bd9',
          'color': '#111',
          'border-width': 0,
          'width': 12,
          'height': 12,
        },
      },
      {
        selector: 'node[kind = "external"]',
        style: {
          'background-color': '#bdbdbd',
        },
      },
      {
        selector: 'edge',
        style: {
          'width': 1,
          'line-color': '#999',
          'target-arrow-color': '#999',
          'target-arrow-shape': 'triangle',
          'curve-style': 'bezier',
          'arrow-scale': 0.8,
        },
      },
      {
        selector: 'edge[dep_type = "build"]',
        style: {
          'line-color': '#000',
          'target-arrow-color': '#000',
        },
      },
      {
        selector: 'edge[dep_type = "exec"]',
        style: {
          'line-color': '#2e7d32',
          'target-arrow-color': '#2e7d32',
        },
      },
      {
        selector: 'edge[dep_type = "test"]',
        style: {
          'line-color': '#ef6c00',
          'target-arrow-color': '#ef6c00',
        },
      },
    ],
    layout: {
      name: 'cose',
      fit: true,
      padding: 20,
      animate: false,
      randomize: true,
      nodeDimensionsIncludeLabels: true,
      componentSpacing: 60,
      idealEdgeLength: 80,
      nodeRepulsion: 4096,
      numIter: 600,
    },
    wheelSensitivity: 0.15,
  });
}

function graphToElements(graph, includeNode, includeEdge) {
  const elements = [];
  const allowed = new Set();

  for (const n of graph.nodes || []) {
    if (!includeNode(n)) continue;
    allowed.add(n.id);
    elements.push({
      data: {
        id: n.id,
        label: n.label,
        kind: n.kind,
      },
    });
  }

  for (const e of graph.edges || []) {
    if (!includeEdge(e)) continue;
    if (!allowed.has(e.source) || !allowed.has(e.target)) continue;
    elements.push({
      data: {
        id: `${e.source} -> ${e.target} (${e.dep_type || 'build'})`,
        source: e.source,
        target: e.target,
        dep_type: e.dep_type || 'build',
      },
    });
  }
  return elements;
}

function renderPackageSummarySection(summary) {
  const section = el('section', { class: 'section' });
  section.appendChild(el('h2', { text: 'Package summary' }));

  const kv = el('div', { class: 'kv' });
  kv.appendChild(el('div', { text: 'Packages' }));
  kv.appendChild(el('div', { text: String(summary.package_count ?? 0) }));
  kv.appendChild(el('div', { text: 'C++ related files' }));
  kv.appendChild(el('div', { text: String(summary.cpp_files ?? 0) }));
  kv.appendChild(el('div', { text: 'Python files' }));
  kv.appendChild(el('div', { text: String(summary.python_files ?? 0) }));
  kv.appendChild(el('div', { text: 'Launch files' }));
  kv.appendChild(el('div', { text: String(summary.launch_files ?? 0) }));
  kv.appendChild(el('div', { text: 'URDF files' }));
  kv.appendChild(el('div', { text: String(summary.urdf_files ?? 0) }));
  kv.appendChild(el('div', { text: 'Xacro files' }));
  kv.appendChild(el('div', { text: String(summary.xacro_files ?? 0) }));
  kv.appendChild(el('div', { text: 'Mesh files' }));
  kv.appendChild(el('div', { text: String(summary.mesh_files ?? 0) }));
  section.appendChild(kv);
  section.appendChild(el('div', { class: 'muted', text: 'Mesh extensions counted: stl/dae/obj/ply' }));
  return section;
}

function renderWorkspaceDependenciesSection(graph) {
  const section = el('section', { class: 'section' });
  section.appendChild(el('h2', { text: 'Workspace package dependencies' }));
  section.appendChild(el('div', { class: 'muted', text: 'View: matrix or graph (edges colored by build/exec/test).' }));

  const controls = el('div', { class: 'controls' });
  const select = el('select');
  select.appendChild(el('option', { value: 'graph', text: 'Graph' }));
  select.appendChild(el('option', { value: 'matrix', text: 'Matrix (〇/×)' }));
  // Default to matrix view for better overview.
  select.value = 'matrix';
  controls.appendChild(el('label', { class: 'control', text: 'View:' }, [select]));

  const filterInput = el('input', { type: 'search', placeholder: 'Filter rows…' });
  controls.appendChild(el('label', { class: 'control', text: 'Filter:' }, [filterInput]));
  section.appendChild(controls);

  const graphWrap = el('div');
  const graphEl = el('div', { class: 'graph' });
  graphWrap.appendChild(graphEl);

  const matrixWrap = el('div', { class: 'matrix dep-matrix' });

  section.appendChild(graphWrap);
  section.appendChild(matrixWrap);

  const workspaceNodes = new Set((graph.nodes || []).filter(n => n.kind === 'workspace').map(n => n.id));
  const workspaceAdj = buildAdjacency(graph, (e) => workspaceNodes.has(e.source) && workspaceNodes.has(e.target));
  const elements = graphToElements(
    graph,
    (n) => n.kind === 'workspace',
    (e) => workspaceNodes.has(e.source) && workspaceNodes.has(e.target)
  );
  setupLazyGraphInit(section, select, () => {
    buildCytoscape(graphEl, elements);
  });

  function renderMatrixIfNeeded() {
    if (matrixWrap.childNodes.length > 0) return;

    const pkgs = Array.from(workspaceNodes).sort();
    const gridEl = el('div', { class: 'ag-theme-alpine dep-grid' });
    matrixWrap.appendChild(gridEl);
    const api = renderDependencyAgGrid(
      gridEl,
      pkgs,
      pkgs,
      (src, dst) => src !== dst && hasAdjacency(workspaceAdj, src, dst),
      { pinnedRowHeaderWidth: 360 }
    );

    // Hook filter input after grid is created.
    if (api) {
      filterInput.addEventListener('input', () => setAgGridQuickFilter(api, filterInput.value));
      setAgGridQuickFilter(api, filterInput.value);
    }
  }

  function applyView() {
    const v = select.value;
    if (v === 'graph') {
      graphWrap.style.display = '';
      matrixWrap.style.display = 'none';
    } else {
      graphWrap.style.display = 'none';
      matrixWrap.style.display = '';
      renderMatrixIfNeeded();
    }
  }

  select.addEventListener('change', applyView);
  applyView();
  return section;
}

function renderExternalDependenciesSection(graph) {
  const section = el('section', { class: 'section' });
  section.appendChild(el('h2', { text: 'External library dependencies' }));
  section.appendChild(el('div', { class: 'muted', text: 'View: matrix or graph (workspace -> external).' }));

  const controls = el('div', { class: 'controls' });
  const select = el('select');
  select.appendChild(el('option', { value: 'graph', text: 'Graph' }));
  select.appendChild(el('option', { value: 'matrix', text: 'Matrix (〇/×)' }));
  // Default to matrix view for better overview.
  select.value = 'matrix';
  controls.appendChild(el('label', { class: 'control', text: 'View:' }, [select]));

  const filterInput = el('input', { type: 'search', placeholder: 'Filter rows…' });
  controls.appendChild(el('label', { class: 'control', text: 'Filter:' }, [filterInput]));
  section.appendChild(controls);

  const graphWrap = el('div');
  const graphEl = el('div', { class: 'graph' });
  graphWrap.appendChild(graphEl);

  const matrixWrap = el('div', { class: 'matrix dep-matrix' });

  section.appendChild(graphWrap);
  section.appendChild(matrixWrap);

  const workspaceNodes = new Set((graph.nodes || []).filter(n => n.kind === 'workspace').map(n => n.id));
  const externalNodes = new Set((graph.nodes || []).filter(n => n.kind === 'external').map(n => n.id));
  const externalAdj = buildAdjacency(graph, (e) => workspaceNodes.has(e.source) && externalNodes.has(e.target));

  const elements = graphToElements(
    graph,
    (n) => n.kind === 'workspace' || n.kind === 'external',
    (e) => workspaceNodes.has(e.source) && externalNodes.has(e.target)
  );
  setupLazyGraphInit(section, select, () => {
    buildCytoscape(graphEl, elements);
  });

  function renderMatrixIfNeeded() {
    if (matrixWrap.childNodes.length > 0) return;

    const rows = Array.from(workspaceNodes).sort();
    const cols = Array.from(externalNodes).sort();
    const gridEl = el('div', { class: 'ag-theme-alpine dep-grid' });
    matrixWrap.appendChild(gridEl);
    const api = renderDependencyAgGrid(
      gridEl,
      rows,
      cols,
      (src, dst) => hasAdjacency(externalAdj, src, dst),
      { pinnedRowHeaderWidth: 360 }
    );

    if (api) {
      filterInput.addEventListener('input', () => setAgGridQuickFilter(api, filterInput.value));
      setAgGridQuickFilter(api, filterInput.value);
    }
  }

  function applyView() {
    const v = select.value;
    if (v === 'graph') {
      graphWrap.style.display = '';
      matrixWrap.style.display = 'none';
    } else {
      graphWrap.style.display = 'none';
      matrixWrap.style.display = '';
      renderMatrixIfNeeded();
    }
  }

  select.addEventListener('change', applyView);
  applyView();
  return section;
}

function setAgGridQuickFilter(api, text) {
  const t = String(text || '');
  if (typeof api.setQuickFilter === 'function') {
    api.setQuickFilter(t);
    return;
  }
  if (typeof api.setGridOption === 'function') {
    api.setGridOption('quickFilterText', t);
    return;
  }
  // Best-effort fallback.
  try {
    api.quickFilterText = t;
    if (typeof api.onFilterChanged === 'function') api.onFilterChanged();
  } catch {
    // ignore
  }
}

function renderDependencyAgGrid(container, rowNames, colNames, predicate, opts) {
  if (!window.agGrid || typeof window.agGrid.Grid !== 'function') {
    container.appendChild(el('div', { class: 'muted', text: 'AG Grid failed to load.' }));
    return null;
  }

  const pinnedRowHeaderWidth = (opts && opts.pinnedRowHeaderWidth) ? opts.pinnedRowHeaderWidth : 320;
  const rows = rowNames.map((r) => ({ rowPkg: r }));

  const colDefs = [
    {
      headerName: 'Package',
      field: 'rowPkg',
      pinned: 'left',
      headerClass: 'dep-row-header',
      width: pinnedRowHeaderWidth,
      minWidth: 200,
      sortable: true,
      filter: 'agTextColumnFilter',
      tooltipField: 'rowPkg',
    },
  ];

  for (const colPkg of colNames) {
    colDefs.push({
      headerName: colPkg,
      colId: colPkg,
      headerClass: 'dep-col-header',
      width: 60,
      minWidth: 60,
      maxWidth: 80,
      sortable: true,
      filter: false,
      headerTooltip: colPkg,
      valueGetter: (params) => {
        const rowPkg = params && params.data ? params.data.rowPkg : '';
        return predicate(rowPkg, colPkg) ? '〇' : '';
      },
      cellClassRules: {
        'dep-yes': (p) => p.value === '〇',
      },
      cellStyle: {
        textAlign: 'center',
      },
    });
  }

  // Header height: make it large enough for rotated long labels.
  // This is a heuristic (not a precise text measurement) but avoids crushed headers.
  const maxLabelLen = colNames.reduce((m, s) => Math.max(m, String(s || '').length), 0);
  // Roughly: baseline + per-character contribution, clamped.
  // With rotated column labels, the needed height scales more directly with label length.
  // Shorten by ~100px vs the previous heuristic.
  const headerHeight = Math.max(120, Math.min(260, -40 + Math.round(maxLabelLen * 8)));

  const gridOptions = {
    rowData: rows,
    columnDefs: colDefs,
    defaultColDef: {
      resizable: true,
    },
    rowHeight: 22,
    headerHeight,
    animateRows: false,
    suppressColumnVirtualisation: false,
    suppressMovableColumns: true,
    enableBrowserTooltips: true,
  };

  new window.agGrid.Grid(container, gridOptions);
  return gridOptions.api || null;
}

function buildAdjacency(graph, includeEdge) {
  const adj = new Map();
  for (const e of graph.edges || []) {
    if (!includeEdge(e)) continue;
    const src = e.source;
    const dst = e.target;
    if (!adj.has(src)) adj.set(src, new Set());
    adj.get(src).add(dst);
  }
  return adj;
}

function hasAdjacency(adj, src, dst) {
  const set = adj.get(src);
  return !!set && set.has(dst);
}

function renderDependencyMatrix(rowNames, colNames, predicate) {
  const table = el('table', { class: 'matrix-table' });
  const thead = el('thead');
  const hr = el('tr');
  hr.appendChild(el('th', { class: 'matrix-corner', text: '' }));
  for (const c of colNames) {
    hr.appendChild(el('th', { class: 'matrix-col-header' }, [
      el('div', { class: 'matrix-col-label', text: c }),
    ]));
  }
  thead.appendChild(hr);
  table.appendChild(thead);

  const tbody = el('tbody');
  for (const r of rowNames) {
    const tr = el('tr');
    tr.appendChild(el('th', { class: 'matrix-row-header', text: r }));
    for (const c of colNames) {
      const yes = predicate(r, c);
      tr.appendChild(el('td', { class: 'matrix-cell', text: yes ? '〇' : '×' }));
    }
    tbody.appendChild(tr);
  }
  table.appendChild(tbody);
  return table;
}

function renderFindingsSection(findings) {
  const section = el('section', { class: 'section' });
  section.appendChild(el('h2', { text: 'Findings' }));
  section.appendChild(el('div', { class: 'muted', text: 'View: by package (detailed list) or by finding (counts per package).' }));

  const controls = el('div', { class: 'controls' });
  const select = el('select');
  select.appendChild(el('option', { value: 'by_package', text: 'By package' }));
  select.appendChild(el('option', { value: 'by_finding', text: 'By finding (rule)' }));
  controls.appendChild(el('label', { class: 'control', text: 'View:' }, [select]));
  section.appendChild(controls);

  const byPkg = el('div');
  const byRule = el('div');
  section.appendChild(byPkg);
  section.appendChild(byRule);

  function renderByPackage() {
    if (byPkg.childNodes.length > 0) return;
    const items = Array.isArray(findings.items) ? findings.items : [];
    const groups = new Map();
    for (const it of items) {
      const k = String(it.package || '(unknown)');
      if (!groups.has(k)) groups.set(k, []);
      groups.get(k).push(it);
    }
    const keys = Array.from(groups.keys()).sort();

    for (const pkg of keys) {
      byPkg.appendChild(el('h3', { text: pkg }));
      const tbl = el('table', { class: 'table' });
      tbl.appendChild(el('thead', {}, [
        el('tr', {}, [
          el('th', { text: 'Rule' }),
          el('th', { text: 'Severity' }),
          el('th', { text: 'File' }),
          el('th', { text: 'Line' }),
          el('th', { text: 'Message' }),
        ]),
      ]));
      const tb = el('tbody');
      for (const it of groups.get(pkg)) {
        tb.appendChild(el('tr', {}, [
          el('td', { text: String(it.rule_id || '') }),
          el('td', { text: String(it.severity || '') }),
          el('td', { text: String(it.file || '') }),
          el('td', { text: it.line != null ? String(it.line) : '' }),
          el('td', { text: String(it.message || '') }),
        ]));
      }
      tbl.appendChild(tb);
      byPkg.appendChild(tbl);
    }
  }

  function renderByFinding() {
    if (byRule.childNodes.length > 0) return;
    const counts = Array.isArray(findings.counts) ? findings.counts : [];
    const rules = new Map();
    for (const c of counts) {
      const rule = String(c.rule_id || '');
      if (!rules.has(rule)) rules.set(rule, []);
      rules.get(rule).push(c);
    }
    const ruleIds = Array.from(rules.keys()).sort();
    for (const ruleId of ruleIds) {
      byRule.appendChild(el('h3', { text: ruleId }));
      const tbl = el('table', { class: 'table' });
      tbl.appendChild(el('thead', {}, [
        el('tr', {}, [
          el('th', { text: 'Package' }),
          el('th', { text: 'Count' }),
        ]),
      ]));
      const tb = el('tbody');
      const rows = rules.get(ruleId).slice().sort((a, b) => {
        const cb = Number(b.count || 0) - Number(a.count || 0);
        if (cb !== 0) return cb;
        return String(a.package || '').localeCompare(String(b.package || ''));
      });
      for (const r of rows) {
        tb.appendChild(el('tr', {}, [
          el('td', { text: String(r.package || '') }),
          el('td', { text: String(r.count || 0) }),
        ]));
      }
      tbl.appendChild(tb);
      byRule.appendChild(tbl);
    }
  }

  function applyView() {
    const v = select.value;
    if (v === 'by_package') {
      byPkg.style.display = '';
      byRule.style.display = 'none';
      renderByPackage();
    } else {
      byPkg.style.display = 'none';
      byRule.style.display = '';
      renderByFinding();
    }
  }

  select.addEventListener('change', applyView);
  applyView();
  return section;
}

function renderFindingsMatrixSection(findings) {
  const section = el('section', { class: 'section' });
  section.appendChild(el('h2', { text: 'Findings matrix (packages × findings)' }));
  section.appendChild(el('div', { class: 'muted', text: 'Click a header to sort.' }));

  const packages = Array.isArray(findings.packages) ? findings.packages.slice() : [];
  const rules = Array.isArray(findings.rules) ? findings.rules.slice() : [];
  const counts = Array.isArray(findings.counts) ? findings.counts : [];

  const map = new Map();
  for (const c of counts) {
    map.set(`${c.package}::${c.rule_id}`, Number(c.count || 0));
  }

  const table = el('table', { class: 'table' });
  const thead = el('thead');
  const hr = el('tr');
  const thPkg = el('th', { text: 'Package' });
  thPkg.className = 'sortable';
  hr.appendChild(thPkg);
  for (const r of rules) {
    const th = el('th', { text: r });
    th.className = 'sortable';
    hr.appendChild(th);
  }
  thead.appendChild(hr);
  table.appendChild(thead);

  const tbody = el('tbody');
  table.appendChild(tbody);

  const rows = packages.map((p) => {
    const row = { package: p, cells: [] };
    for (const r of rules) {
      row.cells.push(map.get(`${p}::${r}`) || 0);
    }
    return row;
  });

  function renderBody(sorted) {
    tbody.innerHTML = '';
    for (const row of sorted) {
      const tr = el('tr');
      tr.appendChild(el('td', { text: row.package }));
      for (const v of row.cells) {
        tr.appendChild(el('td', { text: String(v) }));
      }
      tbody.appendChild(tr);
    }
  }

  let sortCol = -1;
  let sortAsc = true;

  function sortAndRender() {
    const sorted = rows.slice();
    sorted.sort((a, b) => {
      if (sortCol < 0) {
        const c = a.package.localeCompare(b.package);
        return sortAsc ? c : -c;
      }
      const av = a.cells[sortCol];
      const bv = b.cells[sortCol];
      const c = Number(av) - Number(bv);
      if (c !== 0) return sortAsc ? c : -c;
      return a.package.localeCompare(b.package);
    });
    renderBody(sorted);
  }

  thPkg.addEventListener('click', () => {
    if (sortCol === -1) sortAsc = !sortAsc;
    sortCol = -1;
    sortAndRender();
  });
  const ths = hr.querySelectorAll('th');
  for (let i = 1; i < ths.length; i++) {
    const idx = i - 1;
    ths[i].addEventListener('click', () => {
      if (sortCol === idx) sortAsc = !sortAsc;
      else {
        sortCol = idx;
        sortAsc = false;
      }
      sortAndRender();
    });
  }

  sortAndRender();
  section.appendChild(el('div', { class: 'matrix' }, [table]));
  return section;
}

function renderExternalLibrariesSection(externalLibraries) {
  const section = el('section', { class: 'section' });
  section.appendChild(el('h2', { text: 'External libraries' }));

  const items = Array.isArray(externalLibraries.items) ? externalLibraries.items : [];
  if (items.length === 0) {
    section.appendChild(el('div', { class: 'muted', text: 'No external libraries found.' }));
    return section;
  }

  section.appendChild(el('div', { class: 'muted', text: 'Repository URLs are filled from report config (external_repos).' }));

  const table = el('table', { class: 'table' });
  table.appendChild(el('thead', {}, [
    el('tr', {}, [
      el('th', { text: 'Name' }),
      el('th', { text: 'Usage count' }),
      el('th', { text: 'Repository' }),
    ]),
  ]));
  const tbody = el('tbody');
  for (const it of items) {
    const repo = it.repository ? String(it.repository) : '';
    const repoCell = repo
      ? el('a', { href: repo, target: '_blank', rel: 'noreferrer', text: repo })
      : el('span', { class: 'muted', text: '' });
    const tr = el('tr', {}, [
      el('td', { text: String(it.name || '') }),
      el('td', { text: String(it.usage_count ?? 0) }),
      el('td', {}, [repoCell]),
    ]);
    tbody.appendChild(tr);
  }
  table.appendChild(tbody);
  section.appendChild(table);
  return section;
}

main().catch((e) => {
  const generatedEl = document.getElementById('report-generated-at');
  if (generatedEl) generatedEl.textContent = `Error: ${e.message}`;
  console.error(e);
});
