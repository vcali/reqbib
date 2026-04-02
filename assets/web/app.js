const state = {
  data: null,
  selectedScope: null,
  selectedTeam: null,
  selectedShelf: null,
  selectedCommand: null,
  createShelfTarget: null,
  activeTab: "response",
  expanded: {
    local: true,
    shared: true,
    teams: new Set(),
    shelves: new Set(),
  },
};

const sidebarContent = document.getElementById("sidebarContent");
const descriptionEditor = document.getElementById("descriptionEditor");
const commandEditor = document.getElementById("commandEditor");
const commandEditorShell = document.getElementById("commandEditorShell");
const commandEditorHighlight = document.getElementById("commandEditorHighlight");
const runButton = document.getElementById("runButton");
const saveButton = document.getElementById("saveButton");
const runHint = document.getElementById("runHint");
const editorStatus = document.getElementById("editorStatus");
const selectedShelfLabel = document.getElementById("selectedShelfLabel");
const selectedShelfMeta = document.getElementById("selectedShelfMeta");
const requestMethod = document.getElementById("requestMethod");
const requestUrl = document.getElementById("requestUrl");
const responseTitle = document.getElementById("responseTitle");
const responseMeta = document.getElementById("responseMeta");
const statusCode = document.getElementById("statusCode");
const contentType = document.getElementById("contentType");
const exitCode = document.getElementById("exitCode");
const responsePreview = document.getElementById("responsePreview");
const requestHeadersOutput = document.getElementById("requestHeadersOutput");
const responseHeadersOutput = document.getElementById("responseHeadersOutput");
const stderrOutput = document.getElementById("stderrOutput");
const modalBackdrop = document.getElementById("modalBackdrop");
const createShelfScopeLabel = document.getElementById("createShelfScopeLabel");
const createShelfInput = document.getElementById("createShelfInput");
const modalStatus = document.getElementById("modalStatus");
const confirmCreateShelfButton = document.getElementById("confirmCreateShelfButton");
const cancelCreateShelfButton = document.getElementById("cancelCreateShelfButton");
const closeModalButton = document.getElementById("closeModalButton");
const responseTabs = Array.from(document.querySelectorAll(".response-tab"));
const responsePanels = {
  response: document.getElementById("responseTabPanel"),
  headers: document.getElementById("headersTabPanel"),
  stderr: document.getElementById("stderrTabPanel"),
};

async function boot() {
  runButton.addEventListener("click", runCommand);
  saveButton.addEventListener("click", saveCommandToShelf);
  sidebarContent.addEventListener("click", handleSidebarClick);
  responseTabs.forEach((tab) => {
    tab.addEventListener("click", () => setActiveTab(tab.dataset.tab));
  });
  commandEditor.addEventListener("input", syncEditorState);
  commandEditor.addEventListener("scroll", syncCommandHighlightScroll);
  confirmCreateShelfButton.addEventListener("click", createShelf);
  cancelCreateShelfButton.addEventListener("click", closeCreateShelfModal);
  closeModalButton.addEventListener("click", closeCreateShelfModal);
  modalBackdrop.addEventListener("click", handleModalBackdropClick);

  clearEditor();
  clearResponse();
  setActiveTab("response");
  renderEmptySelection();
  await loadBrowse();
}

async function loadBrowse(preferredSelection = null, preferredCommand = null) {
  try {
    const data = await requestJson("/api/browse");
    applyBrowseData(data, preferredSelection, preferredCommand);
  } catch (error) {
    sidebarContent.innerHTML = `<div class="empty-state">${escapeHtml(error.message)}</div>`;
    setEditorStatus(error.message, "error");
  }
}

function applyBrowseData(data, preferredSelection = null, preferredCommand = null) {
  state.data = data;

  const selection =
    resolveSelection(preferredSelection) ||
    resolveSelection({
      scope: state.selectedScope,
      team: state.selectedTeam,
      shelf: state.selectedShelf,
    }) ||
    findFirstShelf();

  if (!selection) {
    renderSidebar();
    renderEmptySelection();
    return;
  }

  expandSelection(selection.scope, selection.team, selection.shelf);
  selectShelf(selection.scope, selection.team, selection.shelf, preferredCommand);
}

function renderSidebar() {
  const localShelves = sortShelves(enrichShelves("local", null, state.data?.local ?? []));
  const sharedTeams = (state.data?.shared ?? []).map((team) => ({
    ...team,
    shelves: sortShelves(enrichShelves("shared", team.team, team.shelves)),
  }));

  sidebarContent.innerHTML = [
    renderLocalGroup(localShelves),
    renderSharedGroup(sharedTeams),
  ].join("");
}

function renderLocalGroup(shelves) {
  return `
    <section class="tree-group">
      <div class="tree-group-header">
        <p class="tree-group-title">Local</p>
        <button
          class="icon-button"
          type="button"
          data-action="create-shelf"
          data-scope="local">
          +
        </button>
      </div>
      ${renderShelfTree(shelves)}
    </section>
  `;
}

function renderSharedGroup(teams) {
  const children = teams.length
    ? teams.map(renderTeamNode).join("")
    : `<div class="empty-state">Shared not configured.</div>`;

  return `
    <section class="tree-group">
      <div class="tree-group-header">
        <p class="tree-group-title">Shared</p>
      </div>
      <div class="tree-children">
        ${children}
      </div>
    </section>
  `;
}

function renderTeamNode(team) {
  const expanded = state.expanded.teams.has(team.team);
  return `
    <div class="tree-node">
      <div class="tree-row">
        <button
          class="tree-toggle"
          type="button"
          data-action="toggle-team"
          data-team="${escapeAttribute(team.team)}">
          ${expanded ? "▾" : "▸"}
        </button>
        <button
          class="tree-label"
          type="button"
          data-action="toggle-team"
          data-team="${escapeAttribute(team.team)}">
          ${githubIcon()}
          <span>${escapeHtml(team.team)}</span>
        </button>
        <button
          class="icon-button"
          type="button"
          data-action="create-shelf"
          data-scope="shared"
          data-team="${escapeAttribute(team.team)}">
          +
        </button>
      </div>
      ${expanded ? `<div class="tree-children">${renderShelfTree(team.shelves)}</div>` : ""}
    </div>
  `;
}

function renderShelfTree(shelves) {
  if (!shelves.length) {
    return `<div class="empty-state">No shelves.</div>`;
  }

  return shelves.map(renderShelfNode).join("");
}

function renderShelfNode(shelf) {
  const shelfKey = shelfPathKey(shelf.__scope, shelf.__team, shelf.shelf);
  const expanded = state.expanded.shelves.has(shelfKey);
  const active =
    state.selectedScope === shelf.__scope &&
    state.selectedTeam === shelf.__team &&
    state.selectedShelf === shelf.shelf;
  const mutedClass = shelf.runnable_command_count === 0 ? "tree-node-muted" : "";

  return `
    <div class="tree-node ${mutedClass}">
      <div class="tree-row">
        <button
          class="tree-toggle"
          type="button"
          data-action="toggle-shelf"
          data-scope="${escapeAttribute(shelf.__scope)}"
          data-team="${escapeAttribute(shelf.__team ?? "")}"
          data-shelf="${escapeAttribute(shelf.shelf)}">
          ${expanded ? "▾" : "▸"}
        </button>
        <button
          class="tree-label ${active ? "active" : ""}"
          type="button"
          data-action="select-shelf"
          data-scope="${escapeAttribute(shelf.__scope)}"
          data-team="${escapeAttribute(shelf.__team ?? "")}"
          data-shelf="${escapeAttribute(shelf.shelf)}">
          <span>${escapeHtml(shelf.shelf)}</span>
        </button>
        <span class="tree-meta">${shelf.commands.length}</span>
      </div>
      ${expanded ? `<div class="tree-children">${renderCommandNodes(shelf)}</div>` : ""}
    </div>
  `;
}

function renderCommandNodes(shelf) {
  return shelf.commands
    .map((command, index) => {
      const active =
        state.selectedScope === shelf.__scope &&
        state.selectedTeam === shelf.__team &&
        state.selectedShelf === shelf.shelf &&
        state.selectedCommand === command.command;
      const title = command.description || command.command;
      const mutedClass = command.runnable ? "" : "tree-node-muted";

      return `
        <button
          class="command-node ${active ? "active" : ""} ${mutedClass}"
          type="button"
          data-action="select-command"
          data-scope="${escapeAttribute(shelf.__scope)}"
          data-team="${escapeAttribute(shelf.__team ?? "")}"
          data-shelf="${escapeAttribute(shelf.shelf)}"
          data-command-index="${index}">
          ${escapeHtml(title)}
        </button>
      `;
    })
    .join("");
}

function handleSidebarClick(event) {
  const button = event.target.closest("[data-action]");
  if (!button) return;

  const { action } = button.dataset;
  if (action === "create-shelf") {
    openCreateShelfModal(button.dataset.scope, button.dataset.team || null);
    return;
  }

  if (action === "toggle-team") {
    toggleTeam(button.dataset.team);
    renderSidebar();
    return;
  }

  if (action === "toggle-shelf") {
    toggleShelf(button.dataset.scope, button.dataset.team || null, button.dataset.shelf);
    renderSidebar();
    return;
  }

  if (action === "select-shelf") {
    const selection = {
      scope: button.dataset.scope,
      team: button.dataset.team || null,
      shelf: button.dataset.shelf,
    };
    expandSelection(selection.scope, selection.team, selection.shelf);
    selectShelf(selection.scope, selection.team, selection.shelf, null);
    return;
  }

  if (action === "select-command") {
    selectCommandFromNode(button);
  }
}

function selectCommandFromNode(button) {
  const shelf = findShelf(button.dataset.scope, button.dataset.team || null, button.dataset.shelf);
  if (!shelf) return;

  const commandIndex = Number(button.dataset.commandIndex);
  if (Number.isNaN(commandIndex) || !shelf.commands[commandIndex]) return;

  state.selectedScope = button.dataset.scope;
  state.selectedTeam = button.dataset.team || null;
  state.selectedShelf = button.dataset.shelf;
  state.selectedCommand = shelf.commands[commandIndex].command;
  renderSidebar();
  renderSelection(shelf);
  clearResponse();
  hydrateEditorFromCommand(shelf.commands[commandIndex]);
}

function toggleTeam(team) {
  if (state.expanded.teams.has(team)) {
    state.expanded.teams.delete(team);
  } else {
    state.expanded.teams.add(team);
  }
}

function toggleShelf(scope, team, shelf) {
  const key = shelfPathKey(scope, team, shelf);
  if (state.expanded.shelves.has(key)) {
    state.expanded.shelves.delete(key);
  } else {
    state.expanded.shelves.add(key);
  }
}

function expandSelection(scope, team, shelf) {
  if (scope === "shared" && team) {
    state.expanded.teams.add(team);
  }
  state.expanded.shelves.add(shelfPathKey(scope, team, shelf));
}

function selectShelf(scope, team, shelfName, preferredCommand = null) {
  const shelf = findShelf(scope, team, shelfName);
  if (!shelf) {
    renderEmptySelection();
    return;
  }

  state.selectedScope = scope;
  state.selectedTeam = team;
  state.selectedShelf = shelfName;
  state.selectedCommand = null;

  renderSidebar();
  renderSelection(shelf);
  clearResponse();

  const selected = preferredCommand
    ? shelf.commands.find((command) => command.command === preferredCommand) || null
    : null;

  if (selected) {
    hydrateEditorFromCommand(selected);
    return;
  }

  clearEditor();
}

function renderSelection(shelf) {
  const label = shelf.__team ? `${shelf.__team} / ${shelf.shelf}` : shelf.shelf;
  selectedShelfLabel.textContent = label;
  selectedShelfMeta.textContent = `${shelf.commands.length} command${shelf.commands.length === 1 ? "" : "s"}`;
}

function renderEmptySelection() {
  selectedShelfLabel.textContent = "Choose a shelf";
  selectedShelfMeta.textContent = "No shelf selected.";
  clearResponse();
  clearEditor();
}

function hydrateEditorFromCommand(command) {
  descriptionEditor.value = command.description || "";
  commandEditor.value = command.command;
  syncEditorState();
}

function clearEditor() {
  state.selectedCommand = null;
  descriptionEditor.value = "";
  commandEditor.value = "";
  syncEditorState();
}

function syncEditorState() {
  const rawCommand = commandEditor.value;
  const command = rawCommand.trim();
  const hasShelf = Boolean(state.selectedShelf);
  const isRunnable = isCurlCommand(command);

  renderCommandHighlight(rawCommand);
  syncCommandHighlightScroll();

  saveButton.disabled = !(hasShelf && command);
  runButton.disabled = !isRunnable;

  if (!command) {
    runHint.textContent = "";
    return;
  }

  if (isRunnable) {
    runHint.textContent = "";
    return;
  }

  runHint.textContent = "Run supports curl requests only.";
}

async function saveCommandToShelf() {
  const command = commandEditor.value.trim();
  if (!command || !state.selectedShelf) return;

  try {
    const payload = await requestJson("/api/commands", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        scope: state.selectedScope,
        team: state.selectedTeam,
        shelf: state.selectedShelf,
        original_command: state.selectedCommand,
        command,
        description: descriptionEditor.value.trim() || null,
      }),
    });

    setEditorStatus(payload.message, "success");
    await loadBrowse(
      {
        scope: state.selectedScope,
        team: state.selectedTeam,
        shelf: state.selectedShelf,
      },
      command,
    );
    state.selectedCommand = command;
  } catch (error) {
    setEditorStatus(error.message, "error");
  }
}

async function runCommand() {
  const command = commandEditor.value.trim();
  if (!command) return;

  try {
    const payload = await requestJson("/api/run", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ command }),
    });
    renderResponse(payload);
  } catch (error) {
    renderError(error.message);
  }
}

function renderResponse(payload) {
  requestMethod.textContent = payload.request?.method || "-";
  requestUrl.textContent = payload.request?.url || "-";
  responseTitle.textContent = payload.success ? "Run Output" : "Run Output";
  responseMeta.textContent = payload.body_note || "";
  statusCode.textContent = payload.http_status ?? "-";
  contentType.textContent = payload.content_type ?? "-";
  exitCode.textContent = payload.exit_code;
  renderHeaders(requestHeadersOutput, payload.request?.headers || [], "No explicit request headers in command.");
  renderHeaders(responseHeadersOutput, payload.headers || [], "No response headers captured yet.");
  stderrOutput.textContent = payload.stderr || "-";
  setActiveTab("response");

  if (payload.body_kind === "image" && payload.preview_url) {
    responsePreview.className = "response-preview";
    responsePreview.innerHTML = `<img alt="Response preview" src="${payload.preview_url}" />`;
    return;
  }

  if (payload.body_kind === "video" && payload.preview_url) {
    responsePreview.className = "response-preview";
    responsePreview.innerHTML = `<video controls playsinline src="${payload.preview_url}"></video>`;
    return;
  }

  if (payload.body_text) {
    responsePreview.className = "response-preview";
    responsePreview.innerHTML = `<pre>${escapeHtml(payload.body_text)}</pre>`;
    return;
  }

  responsePreview.className = "response-preview empty-state";
  responsePreview.textContent = payload.body_note || "No body returned.";
}

function renderError(message) {
  responseTitle.textContent = "Run Output";
  responseMeta.textContent = message;
  statusCode.textContent = "-";
  contentType.textContent = "-";
  exitCode.textContent = "-";
  requestMethod.textContent = "-";
  requestUrl.textContent = "-";
  renderHeaders(requestHeadersOutput, [], "No explicit request headers in command.");
  renderHeaders(responseHeadersOutput, [], "No response headers captured yet.");
  stderrOutput.textContent = "-";
  responsePreview.className = "response-preview empty-state";
  responsePreview.textContent = message;
  setActiveTab("stderr");
}

function clearResponse() {
  responseTitle.textContent = "Run Output";
  responseMeta.textContent = "Awaiting request.";
  requestMethod.textContent = "-";
  requestUrl.textContent = "-";
  statusCode.textContent = "-";
  contentType.textContent = "-";
  exitCode.textContent = "-";
  renderHeaders(requestHeadersOutput, [], "No explicit request headers in command.");
  renderHeaders(responseHeadersOutput, [], "No response headers captured yet.");
  stderrOutput.textContent = "-";
  responsePreview.className = "response-preview empty-state";
  responsePreview.textContent = "No response yet.";
  setActiveTab("response");
}

function renderHeaders(container, headers, emptyMessage) {
  if (!headers.length) {
    container.className = "headers-list empty-state";
    container.textContent = emptyMessage;
    return;
  }

  container.className = "headers-list";
  container.innerHTML = headers
    .map(
      (header) => `
        <div class="header-row">
          <span class="header-name">${escapeHtml(header.name)}</span>
          <span class="header-separator">:</span>
          <span class="header-value">${escapeHtml(header.value)}</span>
        </div>
      `,
    )
    .join("");
}

function renderCommandHighlight(command) {
  commandEditorShell.classList.toggle("empty", !command);
  commandEditorHighlight.innerHTML = buildCommandHighlightHtml(command);
}

function buildCommandHighlightHtml(command) {
  if (!command) {
    return "";
  }

  const tokens = command.match(/"[^"\\]*(?:\\.[^"\\]*)*"|'[^'\\]*(?:\\.[^'\\]*)*'|\s+|[^\s"'`]+/g) || [];
  let previousMeaningfulToken = null;

  return tokens
    .map((token, index) => {
      if (/^\s+$/.test(token)) {
        return escapeHtml(token);
      }

      let tokenClass = "token-plain";
      if (index === 0 && token === "curl") {
        tokenClass = "token-executable";
      } else if (token.startsWith("--") || (/^-[A-Za-z]/.test(token) && token !== "-")) {
        tokenClass = "token-flag";
      } else if (token.startsWith("http://") || token.startsWith("https://")) {
        tokenClass = "token-url";
      } else if ((token.startsWith("'") && token.endsWith("'")) || (token.startsWith('"') && token.endsWith('"'))) {
        tokenClass = previousMeaningfulToken && previousMeaningfulToken.startsWith("-")
          ? "token-argument"
          : "token-string";
      } else if (previousMeaningfulToken && previousMeaningfulToken.startsWith("-")) {
        tokenClass = "token-argument";
      }

      previousMeaningfulToken = token;
      return `<span class="${tokenClass}">${escapeHtml(token)}</span>`;
    })
    .join("");
}

function syncCommandHighlightScroll() {
  commandEditorHighlight.scrollTop = commandEditor.scrollTop;
  commandEditorHighlight.scrollLeft = commandEditor.scrollLeft;
}

function setActiveTab(tabName) {
  state.activeTab = tabName;

  responseTabs.forEach((tab) => {
    const active = tab.dataset.tab === tabName;
    tab.classList.toggle("active", active);
    tab.setAttribute("aria-selected", active ? "true" : "false");
  });

  Object.entries(responsePanels).forEach(([name, panel]) => {
    const active = name === tabName;
    panel.classList.toggle("active", active);
    panel.hidden = !active;
  });
}

function openCreateShelfModal(scope, team) {
  state.createShelfTarget = { scope, team };
  createShelfScopeLabel.textContent =
    scope === "shared" && team
      ? `Shared team: ${team}`
      : "Local shelf";
  createShelfInput.value = "";
  setModalStatus("", "subtle");
  modalBackdrop.hidden = false;
  modalBackdrop.classList.remove("hidden");
  createShelfInput.focus();
}

function closeCreateShelfModal() {
  state.createShelfTarget = null;
  modalBackdrop.hidden = true;
  modalBackdrop.classList.add("hidden");
  createShelfInput.value = "";
  setModalStatus("", "subtle");
}

function handleModalBackdropClick(event) {
  if (event.target === modalBackdrop) {
    closeCreateShelfModal();
  }
}

async function createShelf() {
  const target = state.createShelfTarget;
  const shelf = createShelfInput.value.trim();
  if (!target || !shelf) {
    setModalStatus("Enter a shelf name.", "error");
    return;
  }

  try {
    const payload = await requestJson("/api/shelves", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        scope: target.scope,
        team: target.team,
        shelf,
      }),
    });

    closeCreateShelfModal();
    setEditorStatus(payload.message, "success");
    await loadBrowse(
      { scope: target.scope, team: target.team, shelf },
      null,
    );
  } catch (error) {
    setModalStatus(error.message, "error");
  }
}

async function requestJson(url, options = {}) {
  const response = await fetch(url, options);
  const payload = await response.json().catch(() => ({}));

  if (!response.ok) {
    throw new Error(payload.error || "Request failed.");
  }

  return payload;
}

function findFirstShelf() {
  if (!state.data) return null;

  const shelves = [
    ...sortShelves(enrichShelves("local", null, state.data.local)),
    ...state.data.shared.flatMap((team) =>
      sortShelves(enrichShelves("shared", team.team, team.shelves)),
    ),
  ];
  const preferred = shelves.find((shelf) => shelf.runnable_command_count > 0) || shelves[0];

  return preferred
    ? { scope: preferred.__scope, team: preferred.__team, shelf: preferred.shelf }
    : null;
}

function resolveSelection(selection) {
  if (!selection?.scope || !selection?.shelf) return null;
  const shelf = findShelf(selection.scope, selection.team ?? null, selection.shelf);
  return shelf
    ? { scope: selection.scope, team: selection.team ?? null, shelf: selection.shelf }
    : null;
}

function findShelf(scope, team, shelfName) {
  if (!state.data) return null;

  if (scope === "local") {
    return (
      enrichShelves("local", null, state.data.local).find((shelf) => shelf.shelf === shelfName) ||
      null
    );
  }

  const teamEntry = state.data.shared.find((entry) => entry.team === team);
  if (!teamEntry) return null;

  return (
    enrichShelves("shared", teamEntry.team, teamEntry.shelves).find(
      (shelf) => shelf.shelf === shelfName,
    ) || null
  );
}

function enrichShelves(scope, team, shelves) {
  return shelves.map((shelf) => ({
    ...shelf,
    __scope: scope,
    __team: team,
  }));
}

function sortShelves(shelves) {
  return [...shelves].sort((left, right) => {
    const leftRunnable = left.runnable_command_count > 0 ? 0 : 1;
    const rightRunnable = right.runnable_command_count > 0 ? 0 : 1;

    if (leftRunnable !== rightRunnable) {
      return leftRunnable - rightRunnable;
    }

    return left.shelf.localeCompare(right.shelf);
  });
}

function shelfPathKey(scope, team, shelf) {
  return `${scope}:${team || ""}:${shelf}`;
}

function setEditorStatus(message, tone = "subtle") {
  editorStatus.textContent = message;
  editorStatus.className = `status-line ${tone}`;
}

function setModalStatus(message, tone = "subtle") {
  modalStatus.textContent = message;
  modalStatus.className = `status-line ${tone}`;
}

function isCurlCommand(command) {
  return /^curl(?:\s|$)/.test(command);
}

function githubIcon() {
  return `
    <svg class="github-mark" viewBox="0 0 16 16" aria-hidden="true">
      <path
        fill="currentColor"
        d="M8 0C3.58 0 0 3.58 0 8a8 8 0 0 0 5.47 7.59c.4.07.55-.17.55-.38
        0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13
        -.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.5-1.07
        -1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12
        0 0 .67-.21 2.2.82a7.5 7.5 0 0 1 4 0c1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12
        .51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48
        0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.01 8.01 0 0 0 16 8c0-4.42-3.58-8-8-8Z"
      />
    </svg>
  `;
}

function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}

function escapeAttribute(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll('"', "&quot;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}

boot();
