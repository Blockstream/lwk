import type { WalletAbiSessionControllerCallbacks } from "lwk_wallet_abi_sdk";

import {
  addSplitRecipient,
  cloneScenario,
  isHarnessMode,
  isScenarioKind,
  removeSplitRecipient,
  replaceSplitRecipient,
  replaceTransferRecipient,
  setScenarioKind,
  setScenarioMode,
  setScenarioNetwork,
  type HarnessMode,
  type ScenarioKind,
  type ScenarioV1,
  type SplitScenario,
  type TransferScenario,
} from "./scenario.js";
import {
  createScenarioBundle,
  formatJson,
  parseRawEnvelopeJson,
} from "./request.js";
import { createHarnessSession, type HarnessSession } from "./session.js";
import {
  formatTranscriptPayload,
  formatTranscriptTimestamp,
  prependTranscript,
  type TranscriptEntry,
} from "./transcript.js";
import {
  createShareUrl,
  decodeHarnessLocation,
  encodeScenarioHash,
  encodeRawEnvelopeHash,
} from "./url.js";

interface AppState {
  scenario: ScenarioV1;
  rawEnvelopeText: string;
  rawEnvelopeError: string | null;
  compiledRequestText: string;
  compiledEnvelopeText: string;
  previewError: string | null;
  shareUrl: string;
  projectId: string;
  appUrl: string;
  storagePrefix: string;
  connectionState:
    | "disconnected"
    | "connecting"
    | "connected"
    | "disconnecting";
  sessionTopic: string | null;
  lastActionLabel: string;
  lastResponseText: string;
  lastError: string | null;
  transcript: TranscriptEntry[];
}

function escapeHtml(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

function formatStateLabel(state: AppState["connectionState"]): string {
  switch (state) {
    case "disconnected":
      return "Idle";
    case "connecting":
      return "Pairing";
    case "connected":
      return "Connected";
    case "disconnecting":
      return "Disconnecting";
  }
}

function createInitialState(): AppState {
  const decoded = decodeHarnessLocation(window.location.hash);
  const scenario = decoded.rawEnvelope
    ? {
        ...cloneScenario(decoded.scenario),
        mode: "walletconnect" as const,
      }
    : decoded.scenario;

  return {
    scenario,
    rawEnvelopeText:
      decoded.rawEnvelope === null
        ? ""
        : formatJson(decoded.rawEnvelope).trim(),
    rawEnvelopeError: null,
    compiledRequestText: "",
    compiledEnvelopeText: "",
    previewError: null,
    shareUrl: "",
    projectId: "",
    appUrl: `${window.location.origin}${window.location.pathname}`,
    storagePrefix: "lwk-wallet-abi-harness",
    connectionState: "disconnected",
    sessionTopic: null,
    lastActionLabel: "No transport calls yet",
    lastResponseText: "",
    lastError: null,
    transcript: [],
  };
}

function renderScenarioFields(scenario: ScenarioV1): string {
  switch (scenario.kind) {
    case "transfer":
      return renderTransferFields(scenario);
    case "split":
      return renderSplitFields(scenario);
    case "issuance":
      return `
        <div class="field-grid">
          <label class="field">
            <span>Wallet input id</span>
            <input id="wallet-input-id" value="${escapeHtml(scenario.walletInputId)}" />
          </label>
          <label class="field">
            <span>Asset amount (sat)</span>
            <input id="asset-amount-sat" value="${escapeHtml(scenario.assetAmountSat)}" />
          </label>
          <label class="field">
            <span>Token amount (sat)</span>
            <input id="token-amount-sat" value="${escapeHtml(scenario.tokenAmountSat)}" />
          </label>
          <label class="field field-wide">
            <span>Entropy seed</span>
            <input id="entropy-seed" value="${escapeHtml(scenario.entropySeed)}" />
          </label>
        </div>
      `;
    case "reissuance":
      return `
        <div class="field-grid">
          <label class="field">
            <span>Wallet input id</span>
            <input id="wallet-input-id" value="${escapeHtml(scenario.walletInputId)}" />
          </label>
          <label class="field">
            <span>Token asset id</span>
            <input id="token-asset-id" value="${escapeHtml(scenario.tokenAssetId)}" />
          </label>
          <label class="field">
            <span>Asset amount (sat)</span>
            <input id="asset-amount-sat" value="${escapeHtml(scenario.assetAmountSat)}" />
          </label>
          <label class="field">
            <span>Token amount (sat)</span>
            <input id="token-amount-sat" value="${escapeHtml(scenario.tokenAmountSat)}" />
          </label>
          <label class="field field-wide">
            <span>Entropy seed</span>
            <input id="entropy-seed" value="${escapeHtml(scenario.entropySeed)}" />
          </label>
        </div>
      `;
  }
}

function renderTransferFields(scenario: TransferScenario): string {
  return `
    <div class="recipient-card">
      <div class="recipient-index">Transfer recipient</div>
      <div class="field-grid">
        <label class="field field-wide">
          <span>Recipient address</span>
          <input id="transfer-address" value="${escapeHtml(scenario.recipient.recipientAddress)}" />
        </label>
        <label class="field">
          <span>Amount (sat)</span>
          <input id="transfer-amount" value="${escapeHtml(scenario.recipient.amountSat)}" />
        </label>
        <label class="field">
          <span>Asset id</span>
          <input id="transfer-asset-id" value="${escapeHtml(scenario.recipient.assetId)}" />
        </label>
      </div>
    </div>
  `;
}

function renderSplitFields(scenario: SplitScenario): string {
  return `
    <div class="split-toolbar">
      <div>
        <h3>Recipients</h3>
        <p>Each output is compiled through the typed Wallet ABI schema.</p>
      </div>
      <button type="button" class="accent-button" id="split-add-recipient">Add output</button>
    </div>
    <div class="stack">
      ${scenario.recipients
        .map(
          (recipient, index) => `
            <div class="recipient-card">
              <div class="recipient-index">
                Output ${String(index + 1)}
                <button
                  type="button"
                  class="ghost-button compact-button"
                  data-action="remove-split-recipient"
                  data-index="${String(index)}"
                  ${scenario.recipients.length <= 2 ? "disabled" : ""}
                >
                  Remove
                </button>
              </div>
              <div class="field-grid">
                <label class="field field-wide">
                  <span>Recipient address</span>
                  <input data-field="split-address" data-index="${String(index)}" value="${escapeHtml(
                    recipient.recipientAddress,
                  )}" />
                </label>
                <label class="field">
                  <span>Amount (sat)</span>
                  <input data-field="split-amount" data-index="${String(index)}" value="${escapeHtml(
                    recipient.amountSat,
                  )}" />
                </label>
                <label class="field">
                  <span>Asset id</span>
                  <input data-field="split-asset-id" data-index="${String(index)}" value="${escapeHtml(
                    recipient.assetId,
                  )}" />
                </label>
              </div>
            </div>
          `,
        )
        .join("")}
    </div>
  `;
}

function renderApp(state: AppState): string {
  const transcriptMarkup =
    state.transcript.length === 0
      ? `<div class="empty-state">WalletConnect requests and responses appear here.</div>`
      : state.transcript
          .map(
            (entry) => `
              <article class="transcript-entry ${entry.status === "error" ? "transcript-entry-error" : ""}">
                <div class="transcript-head">
                  <div>
                    <strong>${escapeHtml(entry.direction.toUpperCase())}</strong>
                    <span>${escapeHtml(entry.method)}</span>
                  </div>
                  <div class="transcript-meta">
                    <span>${escapeHtml(formatTranscriptTimestamp(entry))}</span>
                    <span>rpc ${escapeHtml(entry.rpcId)}</span>
                    ${
                      entry.elapsedMs === null
                        ? ""
                        : `<span>${escapeHtml(`${String(entry.elapsedMs)} ms`)}</span>`
                    }
                  </div>
                </div>
                <pre>${escapeHtml(formatTranscriptPayload(entry.payload))}</pre>
              </article>
            `,
          )
          .join("");

  return `
    <main class="shell">
      <section class="hero-panel">
        <div class="hero-copy">
          <p class="eyebrow">Liquid Wallet Kit</p>
          <h1>Wallet ABI Harness</h1>
          <p class="lede">
            Builder-grade request previews on the left, transport-grade WalletConnect inspection on the right.
            The UI is intentionally blunt: what matters is the exact request, the exact response, and the raw transcript in between.
          </p>
        </div>
        <div class="hero-status">
          <div class="status-card">
            <span>Mode</span>
            <strong>${escapeHtml(state.scenario.mode)}</strong>
          </div>
          <div class="status-card">
            <span>Transport</span>
            <strong>${escapeHtml(formatStateLabel(state.connectionState))}</strong>
          </div>
          <div class="status-card">
            <span>Session topic</span>
            <strong>${escapeHtml(state.sessionTopic ?? "none")}</strong>
          </div>
        </div>
      </section>

      <section class="workspace-grid">
        <section class="panel panel-form">
          <div class="panel-header">
            <div>
              <p class="panel-kicker">Scenario</p>
              <h2>Builder Surface</h2>
            </div>
            <div class="mode-toggle">
              ${(["builder", "walletconnect"] as const)
                .map(
                  (mode) => `
                    <button
                      type="button"
                      class="toggle-button ${state.scenario.mode === mode ? "toggle-button-active" : ""}"
                      data-action="set-mode"
                      data-mode="${mode}"
                    >
                      ${mode}
                    </button>
                  `,
                )
                .join("")}
            </div>
          </div>

          <div class="field-grid">
            <label class="field">
              <span>Scenario kind</span>
              <select id="scenario-kind">
                ${(["transfer", "split", "issuance", "reissuance"] as const)
                  .map(
                    (kind) => `
                      <option value="${kind}" ${state.scenario.kind === kind ? "selected" : ""}>
                        ${kind}
                      </option>
                    `,
                  )
                  .join("")}
              </select>
            </label>
            <label class="field">
              <span>Transport network</span>
              <select id="scenario-network" ${state.connectionState === "connected" ? "disabled" : ""}>
                ${(["liquid", "testnet-liquid", "localtest-liquid"] as const)
                  .map(
                    (network) => `
                      <option value="${network}" ${state.scenario.network === network ? "selected" : ""}>
                        ${network}
                      </option>
                    `,
                  )
                  .join("")}
              </select>
            </label>
            <label class="field">
              <span>Fee rate (sat/kvB)</span>
              <input id="fee-rate" value="${state.scenario.feeRateSatKvb ?? ""}" />
            </label>
            <label class="field field-checkbox">
              <input id="broadcast-toggle" type="checkbox" ${state.scenario.broadcast ? "checked" : ""} />
              <span>Broadcast</span>
            </label>
            <label class="field field-checkbox">
              <input id="auto-connect-toggle" type="checkbox" ${state.scenario.autoConnect ? "checked" : ""} />
              <span>Auto connect</span>
            </label>
            <label class="field field-checkbox">
              <input id="auto-send-toggle" type="checkbox" ${state.scenario.autoSend ? "checked" : ""} />
              <span>Auto send</span>
            </label>
          </div>

          ${renderScenarioFields(state.scenario)}

          <div class="raw-toolbar">
            <div>
              <h3>Expert raw envelope</h3>
              <p>Paste a full JSON-RPC request when you want transport parity instead of typed builder output.</p>
            </div>
            <div class="button-row">
              <button type="button" class="ghost-button" id="load-compiled-raw">Load compiled envelope</button>
              <button type="button" class="ghost-button" id="clear-raw">Clear raw</button>
            </div>
          </div>
          <textarea id="raw-envelope" class="code-input" spellcheck="false">${escapeHtml(state.rawEnvelopeText)}</textarea>
          ${
            state.rawEnvelopeError === null
              ? ""
              : `<p class="message error-message">${escapeHtml(state.rawEnvelopeError)}</p>`
          }

          <div class="share-row">
            <button type="button" class="accent-button" id="copy-share-url">Copy share URL</button>
            <code>${escapeHtml(state.shareUrl)}</code>
          </div>
        </section>

        <section class="panel panel-preview">
          <div class="panel-header">
            <div>
              <p class="panel-kicker">Preview</p>
              <h2>Request JSON</h2>
            </div>
          </div>
          ${
            state.previewError === null
              ? ""
              : `<p class="message error-message">${escapeHtml(state.previewError)}</p>`
          }
          <div class="preview-grid">
            <div class="code-panel">
              <h3>Typed request</h3>
              <pre>${escapeHtml(state.compiledRequestText)}</pre>
            </div>
            <div class="code-panel">
              <h3>JSON-RPC envelope</h3>
              <pre>${escapeHtml(state.compiledEnvelopeText)}</pre>
            </div>
          </div>

          <div class="panel-header">
            <div>
              <p class="panel-kicker">WalletConnect</p>
              <h2>Transport Controls</h2>
            </div>
          </div>
          <div class="field-grid">
            <label class="field field-wide">
              <span>WalletConnect project id</span>
              <input id="project-id" value="${escapeHtml(state.projectId)}" ${state.connectionState === "connected" ? "disabled" : ""} />
            </label>
            <label class="field">
              <span>App URL</span>
              <input id="app-url" value="${escapeHtml(state.appUrl)}" ${state.connectionState === "connected" ? "disabled" : ""} />
            </label>
            <label class="field">
              <span>Storage prefix</span>
              <input id="storage-prefix" value="${escapeHtml(state.storagePrefix)}" ${state.connectionState === "connected" ? "disabled" : ""} />
            </label>
          </div>

          <div class="button-row">
            <button type="button" class="accent-button" id="connect-wallet" ${state.connectionState === "connected" || state.connectionState === "connecting" ? "disabled" : ""}>
              Connect wallet
            </button>
            <button type="button" class="ghost-button" id="disconnect-wallet" ${state.connectionState !== "connected" ? "disabled" : ""}>
              Disconnect
            </button>
            <button type="button" class="ghost-button" id="send-get-receive">
              get_signer_receive_address
            </button>
            <button type="button" class="ghost-button" id="send-get-xonly">
              get_raw_signing_x_only_pubkey
            </button>
            <button type="button" class="accent-button" id="send-scenario">
              Send scenario
            </button>
            <button type="button" class="ghost-button" id="send-raw">
              Send raw
            </button>
          </div>

          <div class="response-card">
            <div class="response-head">
              <strong>${escapeHtml(state.lastActionLabel)}</strong>
              <span>${escapeHtml(formatStateLabel(state.connectionState))}</span>
            </div>
            ${
              state.lastError === null
                ? ""
                : `<p class="message error-message">${escapeHtml(state.lastError)}</p>`
            }
            <pre>${escapeHtml(state.lastResponseText)}</pre>
          </div>

          <div class="panel-header">
            <div>
              <p class="panel-kicker">Transcript</p>
              <h2>Raw JSON-RPC log</h2>
            </div>
          </div>
          <div class="transcript-list">${transcriptMarkup}</div>
        </section>
      </section>
    </main>
  `;
}

export async function mountApp(root: HTMLElement): Promise<void> {
  const state = createInitialState();
  let session: HarnessSession | null = null;
  let releaseSubscription: (() => void) | null = null;
  let previewSequence = 0;
  let autoFlowRan = false;

  const sessionCallbacks: WalletAbiSessionControllerCallbacks = {
    onConnected() {
      state.connectionState = "connected";
      state.sessionTopic = session?.sessionTopic() ?? null;
      render();
    },
    onDisconnected() {
      state.connectionState = "disconnected";
      state.sessionTopic = null;
      render();
    },
    onUpdated() {
      state.sessionTopic = session?.sessionTopic() ?? null;
      render();
    },
  };

  function syncShareUrl(): void {
    const rawEnvelope =
      state.rawEnvelopeText.trim().length === 0
        ? null
        : (() => {
            try {
              return parseRawEnvelopeJson(state.rawEnvelopeText);
            } catch {
              return null;
            }
          })();

    state.shareUrl = createShareUrl(window.location.href, {
      scenario: state.scenario,
      ...(rawEnvelope === null ? {} : { rawEnvelope }),
    });

    window.history.replaceState(
      null,
      "",
      rawEnvelope === null
        ? `#${encodeScenarioHash(state.scenario)}`
        : `#${encodeRawEnvelopeHash(rawEnvelope)}`,
    );
  }

  function render(): void {
    root.innerHTML = renderApp(state);

    bindButton("copy-share-url", async () => {
      await navigator.clipboard.writeText(state.shareUrl);
      state.lastActionLabel = "Share URL copied";
      render();
    });

    bindToggleModeButtons();
    bindSelect("scenario-kind", (value) => {
      if (!isScenarioKind(value)) {
        return;
      }

      state.scenario = setScenarioKind(state.scenario, value as ScenarioKind);
      syncShareUrl();
      render();
      void refreshPreview();
    });
    bindSelect("scenario-network", (value) => {
      if (
        value !== "liquid" &&
        value !== "testnet-liquid" &&
        value !== "localtest-liquid"
      ) {
        return;
      }

      state.scenario = setScenarioNetwork(state.scenario, value);
      syncShareUrl();
      render();
      void refreshPreview();
    });
    bindInput("fee-rate", (value) => {
      state.scenario = cloneScenario({
        ...state.scenario,
        feeRateSatKvb: value.trim().length === 0 ? null : Number(value),
      });
      syncShareUrl();
      render();
      void refreshPreview();
    });
    bindCheckbox("broadcast-toggle", (checked) => {
      state.scenario = cloneScenario({
        ...state.scenario,
        broadcast: checked,
      });
      syncShareUrl();
      render();
      void refreshPreview();
    });
    bindCheckbox("auto-connect-toggle", (checked) => {
      state.scenario = cloneScenario({
        ...state.scenario,
        autoConnect: checked,
      });
      syncShareUrl();
      render();
      void refreshPreview();
    });
    bindCheckbox("auto-send-toggle", (checked) => {
      state.scenario = cloneScenario({
        ...state.scenario,
        autoSend: checked,
      });
      syncShareUrl();
      render();
      void refreshPreview();
    });

    bindScenarioFields();

    bindInput("raw-envelope", (value) => {
      state.rawEnvelopeText = value;
      validateRawEnvelope();
      syncShareUrl();
      render();
    });
    bindButton("load-compiled-raw", () => {
      state.rawEnvelopeText = state.compiledEnvelopeText.trim();
      validateRawEnvelope();
      syncShareUrl();
      render();
    });
    bindButton("clear-raw", () => {
      state.rawEnvelopeText = "";
      state.rawEnvelopeError = null;
      syncShareUrl();
      render();
    });

    bindInput("project-id", (value) => {
      state.projectId = value.trim();
      render();
    });
    bindInput("app-url", (value) => {
      state.appUrl = value.trim();
      render();
    });
    bindInput("storage-prefix", (value) => {
      state.storagePrefix = value.trim();
      render();
    });

    bindButton("connect-wallet", async () => {
      await connectWallet();
    });
    bindButton("disconnect-wallet", async () => {
      await disconnectWallet();
    });
    bindButton("send-get-receive", async () => {
      await callTransport("get_signer_receive_address", async () => {
        const sessionHandle = await ensureSession();
        const result = await sessionHandle.getSignerReceiveAddress();
        state.lastResponseText = formatJson(result.response).trim();
      });
    });
    bindButton("send-get-xonly", async () => {
      await callTransport("get_raw_signing_x_only_pubkey", async () => {
        const sessionHandle = await ensureSession();
        const result = await sessionHandle.getRawSigningXOnlyPubkey();
        state.lastResponseText = formatJson(result.response).trim();
      });
    });
    bindButton("send-scenario", async () => {
      await callTransport(`${state.scenario.kind} scenario`, async () => {
        const bundle = await createScenarioBundle(state.scenario);
        const sessionHandle = await ensureSession();
        const result = await sessionHandle.processRequest(bundle.request);
        state.lastResponseText = formatJson(result.response).trim();
      });
    });
    bindButton("send-raw", async () => {
      await callTransport("raw envelope", async () => {
        const envelope = parseRawEnvelopeJson(state.rawEnvelopeText);
        const sessionHandle = await ensureSession();
        const result = await sessionHandle.sendRawEnvelope(envelope);
        state.lastResponseText = formatJson(result.response).trim();
      });
    });
  }

  function validateRawEnvelope(): void {
    if (state.rawEnvelopeText.trim().length === 0) {
      state.rawEnvelopeError = null;
      return;
    }

    try {
      parseRawEnvelopeJson(state.rawEnvelopeText);
      state.rawEnvelopeError = null;
    } catch (error) {
      state.rawEnvelopeError =
        error instanceof Error ? error.message : String(error);
    }
  }

  async function refreshPreview(): Promise<void> {
    previewSequence += 1;
    const currentSequence = previewSequence;

    try {
      const bundle = await createScenarioBundle(state.scenario);
      if (currentSequence !== previewSequence) {
        return;
      }

      state.compiledRequestText = bundle.requestJson.trim();
      state.compiledEnvelopeText = bundle.envelopeJson.trim();
      state.previewError = null;
    } catch (error) {
      if (currentSequence !== previewSequence) {
        return;
      }

      state.previewError =
        error instanceof Error ? error.message : String(error);
      state.compiledRequestText = "";
      state.compiledEnvelopeText = "";
    }

    validateRawEnvelope();
    syncShareUrl();
    render();
    void maybeRunAutoFlow();
  }

  async function ensureSession(): Promise<HarnessSession> {
    if (session !== null) {
      return session;
    }

    if (state.projectId.trim().length === 0) {
      throw new Error("WalletConnect project id is required before pairing.");
    }

    const createdSession = await createHarnessSession({
      projectId: state.projectId,
      network: state.scenario.network,
      appUrl: state.appUrl,
      storagePrefix: state.storagePrefix,
      onTranscript(entry) {
        state.transcript = prependTranscript(state.transcript, entry);
        render();
      },
    });

    releaseSubscription?.();
    releaseSubscription = createdSession.subscribe(sessionCallbacks);
    session = createdSession;
    state.sessionTopic = createdSession.sessionTopic();
    return createdSession;
  }

  async function connectWallet(): Promise<void> {
    state.lastError = null;
    state.connectionState = "connecting";
    state.lastActionLabel = "Opening WalletConnect pairing";
    render();

    try {
      const nextSession = await ensureSession();
      await nextSession.connect();
      state.connectionState = "connected";
      state.sessionTopic = nextSession.sessionTopic();
      state.lastActionLabel = "Wallet paired";
    } catch (error) {
      state.connectionState = "disconnected";
      state.lastError = error instanceof Error ? error.message : String(error);
      state.lastActionLabel = "Pairing failed";
    }

    render();
  }

  async function disconnectWallet(): Promise<void> {
    if (session === null) {
      return;
    }

    state.lastError = null;
    state.connectionState = "disconnecting";
    state.lastActionLabel = "Disconnecting session";
    render();

    try {
      await session.disconnect();
      state.connectionState = "disconnected";
      state.sessionTopic = null;
      state.lastActionLabel = "Session disconnected";
      session = null;
      releaseSubscription?.();
      releaseSubscription = null;
    } catch (error) {
      state.connectionState = "connected";
      state.lastError = error instanceof Error ? error.message : String(error);
      state.lastActionLabel = "Disconnect failed";
    }

    render();
  }

  async function callTransport(
    label: string,
    callback: () => Promise<void>,
  ): Promise<void> {
    state.lastActionLabel = `Running ${label}`;
    state.lastError = null;
    render();

    try {
      await callback();
      state.lastActionLabel = `${label} completed`;
      state.sessionTopic = session?.sessionTopic() ?? null;
    } catch (error) {
      state.lastError = error instanceof Error ? error.message : String(error);
      state.lastActionLabel = `${label} failed`;
    }

    render();
  }

  async function maybeRunAutoFlow(): Promise<void> {
    if (autoFlowRan || state.scenario.mode !== "walletconnect") {
      return;
    }

    if (!state.scenario.autoConnect) {
      return;
    }

    autoFlowRan = true;
    await connectWallet();

    if (!state.scenario.autoSend) {
      return;
    }

    if (state.rawEnvelopeText.trim().length > 0) {
      await callTransport("auto raw envelope", async () => {
        const envelope = parseRawEnvelopeJson(state.rawEnvelopeText);
        const sessionHandle = await ensureSession();
        const result = await sessionHandle.sendRawEnvelope(envelope);
        state.lastResponseText = formatJson(result.response).trim();
      });
      return;
    }

    await callTransport(`auto ${state.scenario.kind}`, async () => {
      const bundle = await createScenarioBundle(state.scenario);
      const sessionHandle = await ensureSession();
      const result = await sessionHandle.processRequest(bundle.request);
      state.lastResponseText = formatJson(result.response).trim();
    });
  }

  function bindButton(id: string, handler: () => void | Promise<void>): void {
    const element = document.getElementById(id);
    if (!(element instanceof HTMLButtonElement)) {
      return;
    }

    element.addEventListener("click", () => {
      void handler();
    });
  }

  function bindInput(id: string, handler: (value: string) => void): void {
    const element = document.getElementById(id);
    if (
      !(
        element instanceof HTMLInputElement ||
        element instanceof HTMLTextAreaElement
      )
    ) {
      return;
    }

    element.addEventListener("input", () => {
      handler(element.value);
    });
  }

  function bindSelect(id: string, handler: (value: string) => void): void {
    const element = document.getElementById(id);
    if (!(element instanceof HTMLSelectElement)) {
      return;
    }

    element.addEventListener("change", () => {
      handler(element.value);
    });
  }

  function bindCheckbox(id: string, handler: (checked: boolean) => void): void {
    const element = document.getElementById(id);
    if (!(element instanceof HTMLInputElement)) {
      return;
    }

    element.addEventListener("change", () => {
      handler(element.checked);
    });
  }

  function bindToggleModeButtons(): void {
    for (const button of root.querySelectorAll<HTMLButtonElement>(
      "[data-action='set-mode']",
    )) {
      button.addEventListener("click", () => {
        const mode = button.dataset.mode;
        if (!isHarnessMode(mode)) {
          return;
        }

        state.scenario = setScenarioMode(state.scenario, mode as HarnessMode);
        autoFlowRan = false;
        syncShareUrl();
        render();
        void refreshPreview();
      });
    }
  }

  function bindScenarioFields(): void {
    switch (state.scenario.kind) {
      case "transfer":
        bindInput("transfer-address", (value) => {
          state.scenario = replaceTransferRecipient(
            state.scenario as TransferScenario,
            {
              recipientAddress: value,
            },
          );
          syncShareUrl();
          render();
          void refreshPreview();
        });
        bindInput("transfer-amount", (value) => {
          state.scenario = replaceTransferRecipient(
            state.scenario as TransferScenario,
            {
              amountSat: value,
            },
          );
          syncShareUrl();
          render();
          void refreshPreview();
        });
        bindInput("transfer-asset-id", (value) => {
          state.scenario = replaceTransferRecipient(
            state.scenario as TransferScenario,
            {
              assetId: value,
            },
          );
          syncShareUrl();
          render();
          void refreshPreview();
        });
        break;
      case "split":
        bindButton("split-add-recipient", () => {
          state.scenario = addSplitRecipient(state.scenario as SplitScenario);
          syncShareUrl();
          render();
          void refreshPreview();
        });

        for (const input of root.querySelectorAll<HTMLInputElement>(
          "[data-field='split-address']",
        )) {
          input.addEventListener("input", () => {
            const index = Number(input.dataset.index);
            state.scenario = replaceSplitRecipient(
              state.scenario as SplitScenario,
              index,
              {
                recipientAddress: input.value,
              },
            );
            syncShareUrl();
            render();
            void refreshPreview();
          });
        }
        for (const input of root.querySelectorAll<HTMLInputElement>(
          "[data-field='split-amount']",
        )) {
          input.addEventListener("input", () => {
            const index = Number(input.dataset.index);
            state.scenario = replaceSplitRecipient(
              state.scenario as SplitScenario,
              index,
              {
                amountSat: input.value,
              },
            );
            syncShareUrl();
            render();
            void refreshPreview();
          });
        }
        for (const input of root.querySelectorAll<HTMLInputElement>(
          "[data-field='split-asset-id']",
        )) {
          input.addEventListener("input", () => {
            const index = Number(input.dataset.index);
            state.scenario = replaceSplitRecipient(
              state.scenario as SplitScenario,
              index,
              {
                assetId: input.value,
              },
            );
            syncShareUrl();
            render();
            void refreshPreview();
          });
        }
        for (const button of root.querySelectorAll<HTMLButtonElement>(
          "[data-action='remove-split-recipient']",
        )) {
          button.addEventListener("click", () => {
            const index = Number(button.dataset.index);
            state.scenario = removeSplitRecipient(
              state.scenario as SplitScenario,
              index,
            );
            syncShareUrl();
            render();
            void refreshPreview();
          });
        }
        break;
      case "issuance":
        bindInput("wallet-input-id", (value) => {
          state.scenario = cloneScenario({
            ...state.scenario,
            walletInputId: value,
          });
          syncShareUrl();
          render();
          void refreshPreview();
        });
        bindInput("asset-amount-sat", (value) => {
          state.scenario = cloneScenario({
            ...state.scenario,
            assetAmountSat: value,
          });
          syncShareUrl();
          render();
          void refreshPreview();
        });
        bindInput("token-amount-sat", (value) => {
          state.scenario = cloneScenario({
            ...state.scenario,
            tokenAmountSat: value,
          });
          syncShareUrl();
          render();
          void refreshPreview();
        });
        bindInput("entropy-seed", (value) => {
          state.scenario = cloneScenario({
            ...state.scenario,
            entropySeed: value,
          });
          syncShareUrl();
          render();
          void refreshPreview();
        });
        break;
      case "reissuance":
        bindInput("wallet-input-id", (value) => {
          state.scenario = cloneScenario({
            ...state.scenario,
            walletInputId: value,
          });
          syncShareUrl();
          render();
          void refreshPreview();
        });
        bindInput("token-asset-id", (value) => {
          state.scenario = cloneScenario({
            ...state.scenario,
            tokenAssetId: value,
          });
          syncShareUrl();
          render();
          void refreshPreview();
        });
        bindInput("asset-amount-sat", (value) => {
          state.scenario = cloneScenario({
            ...state.scenario,
            assetAmountSat: value,
          });
          syncShareUrl();
          render();
          void refreshPreview();
        });
        bindInput("token-amount-sat", (value) => {
          state.scenario = cloneScenario({
            ...state.scenario,
            tokenAmountSat: value,
          });
          syncShareUrl();
          render();
          void refreshPreview();
        });
        bindInput("entropy-seed", (value) => {
          state.scenario = cloneScenario({
            ...state.scenario,
            entropySeed: value,
          });
          syncShareUrl();
          render();
          void refreshPreview();
        });
        break;
    }
  }

  render();
  await refreshPreview();
}
