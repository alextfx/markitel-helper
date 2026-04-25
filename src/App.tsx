import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api } from "./lib/api";
import type { HelperStatus, InstallEaResult, PairedEvent } from "./lib/types";

// Phase semantics in v0.0.6+:
//   loading           — initial status() call in flight
//   unpaired          — keychain has no key, show pair button
//   pairing           — user clicked "Pair with Markitel" + we're waiting
//                       on the OS to bring this app back with a code
//   installing        — pair exchange done, install_ea_inner running
//                       (auto, kicked off by exchange_and_persist)
//   needs-mt5-closed  — pair done but MT5 was running, user must close
//                       MT5 + click Install to retry
//   install-error     — pair done, auto-install failed for a non-MT5
//                       reason (e.g. couldn't reach ea-source endpoint)
//   installed         — EA written, summary available
//   paired-stale      — keychain has key on cold-start, no recent install
//                       result in memory; show Reinstall / Open MT5
//   error             — any other terminal error
type Phase =
  | "loading"
  | "unpaired"
  | "pairing"
  | "installing"
  | "needs-mt5-closed"
  | "install-error"
  | "installed"
  | "paired-stale"
  | "error";

export function App() {
  const [phase, setPhase] = useState<Phase>("loading");
  const [status, setStatus] = useState<HelperStatus | null>(null);
  const [paired, setPaired] = useState<PairedEvent | null>(null);
  const [install, setInstall] = useState<InstallEaResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [manualCode, setManualCode] = useState("");

  // Reads keychain via the `status` Tauri command. On cold-launch we
  // only know "paired or not" — if paired and no in-memory install
  // result, we show the paired-stale screen with a Reinstall button.
  const refreshStatus = useCallback(async () => {
    try {
      const s = await api.status();
      setStatus(s);
      setPhase((current) => {
        // If the user is mid-flow (installing, needs-mt5-closed, etc.)
        // don't clobber it with a status-driven phase.
        if (
          current === "installing" ||
          current === "installed" ||
          current === "needs-mt5-closed" ||
          current === "install-error"
        ) {
          return current;
        }
        return s.paired ? "paired-stale" : "unpaired";
      });
    } catch (e) {
      setError(String(e));
      setPhase("error");
    }
  }, []);

  useEffect(() => {
    refreshStatus();
  }, [refreshStatus]);

  useEffect(() => {
    // helper://paired fires immediately after keychain save, before
    // install runs. We use it to flip the UI into "installing" so the
    // user sees progress, then a second event resolves to either
    // installed / needs-mt5-closed / install-error.
    const un1 = listen<PairedEvent>("helper://paired", (e) => {
      setPaired(e.payload);
      setError(null);
      setPhase("installing");
      refreshStatus();
    });

    // helper://paired-and-installed carries the InstallEaResult — the
    // happy path of auto-install on pair.
    const un2 = listen<InstallEaResult>(
      "helper://paired-and-installed",
      (e) => {
        setInstall(e.payload);
        setPhase("installed");
      },
    );

    // helper://needs-mt5-closed: paired succeeded, but MT5 was running
    // so the install was deferred. UI tells user to close MT5 + click
    // Install Now.
    const un3 = listen<void>("helper://needs-mt5-closed", () => {
      setPhase("needs-mt5-closed");
    });

    // helper://install-error: paired succeeded, but install failed for
    // some reason (network, EA source 500, etc.). Show error + manual
    // Install Now button.
    const un4 = listen<string>("helper://install-error", (e) => {
      setError(e.payload);
      setPhase("install-error");
    });

    // helper://pair-error: pair exchange itself failed (bad code,
    // expired, network, etc.). Bounce back to unpaired with error.
    const un5 = listen<string>("helper://pair-error", (e) => {
      setError(e.payload);
      setPhase("error");
    });

    // Tray menu items.
    const un6 = listen<void>("helper://rotate-key", async () => {
      try {
        await api.rotateKey();
        refreshStatus();
      } catch (err) {
        setError(String(err));
      }
    });
    const un7 = listen<void>("helper://reinstall-ea", () => {
      handleInstallNow();
    });

    return () => {
      un1.then((f) => f());
      un2.then((f) => f());
      un3.then((f) => f());
      un4.then((f) => f());
      un5.then((f) => f());
      un6.then((f) => f());
      un7.then((f) => f());
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [refreshStatus]);

  const handlePairWeb = async () => {
    setError(null);
    try {
      await api.startPairing();
      setPhase("pairing");
    } catch (e) {
      setError(String(e));
      setPhase("error");
    }
  };

  const handlePairManual = async () => {
    setError(null);
    if (!/^[A-Z0-9]{6}$/i.test(manualCode.trim())) {
      setError("Codes are 6 letters/numbers. Check the website for your current code.");
      return;
    }
    try {
      // exchange_and_persist will auto-chain into install — UI will
      // resolve via the event listeners above.
      await api.pairWithCode(manualCode.trim());
    } catch (e) {
      setError(String(e));
    }
  };

  // Used when:
  //   1. Auto-install was deferred (MT5 running) and user has now closed
  //      MT5 — they click "Install Now."
  //   2. Auto-install errored — they want to retry.
  //   3. Cold-launch into paired-stale — they want a fresh write.
  //   4. Tray menu "Reinstall EA."
  const handleInstallNow = async () => {
    setError(null);
    setPhase("installing");
    try {
      const running = await api.isMt5Running();
      if (running) {
        setPhase("needs-mt5-closed");
        return;
      }
      const result = await api.installEa();
      setInstall(result);
      setPhase("installed");
    } catch (e) {
      setError(String(e));
      setPhase("install-error");
    }
  };

  const handleLaunchMt5 = async () => {
    try {
      await api.launchMt5();
    } catch (e) {
      setError(String(e));
    }
  };

  const writtenTo = install?.writtenTo ?? [];
  const profileNeedsManualDrag = (install?.profileResults ?? []).some(
    (p) => p.outcome === "NeedsManualDrag",
  );

  return (
    <div className="panel">
      <header>
        <h1>Markitel Helper</h1>
        {status && <span className="version">v{status.version}</span>}
      </header>

      {phase === "loading" && <p>Loading…</p>}

      {phase === "unpaired" && (
        <section>
          <h2>Connect your Markitel account</h2>
          <p className="muted">
            Click below to open Markitel in your browser. Pick "Markitel
            Helper" on the Connect MT5 page and the helper takes over —
            no copying API keys, no dragging files.
          </p>
          <button className="primary" onClick={handlePairWeb}>
            Pair with Markitel
          </button>
          <details className="manual-pair">
            <summary>Pair manually with a code</summary>
            <input
              type="text"
              placeholder="ABC123"
              maxLength={6}
              value={manualCode}
              onChange={(e) => setManualCode(e.target.value.toUpperCase())}
            />
            <button onClick={handlePairManual}>Pair</button>
          </details>
        </section>
      )}

      {phase === "pairing" && (
        <section>
          <h2>Waiting for pairing…</h2>
          <p className="muted">
            Complete the flow in your browser. Markitel will ask your OS
            to reopen the helper with your pairing code, and the EA will
            install automatically.
          </p>
        </section>
      )}

      {phase === "installing" && (
        <section>
          <h2>Installing the bridge…</h2>
          <p className="muted">
            {paired?.userEmail ? `Paired as ${paired.userEmail}.` : "Paired."}{" "}
            Writing the EA into MT5 and whitelisting the URL.
          </p>
          <div className="spinner" aria-hidden />
        </section>
      )}

      {phase === "needs-mt5-closed" && (
        <section>
          <h2>Close MT5 to finish install</h2>
          <p className="muted">
            We paired your account, but MT5 is still running. We can't
            replace a running EA file safely. Quit MT5 (File → Exit),
            then click below.
          </p>
          <button className="primary" onClick={handleInstallNow}>
            I closed MT5 — install now
          </button>
          <p className="tiny muted">
            We'll write <strong>Markitel_Bridge.mq5</strong> into every
            MT5 install we find, plus add{" "}
            <code>https://markitel.com</code> to the WebRequest
            whitelist.
          </p>
        </section>
      )}

      {phase === "install-error" && (
        <section>
          <h2>Pair worked — install didn't</h2>
          <p className="muted">
            Your account is paired, but writing the EA into MT5 failed.
            Most common cause: the helper couldn't reach{" "}
            <code>markitel.com</code>. Check your network and try again.
          </p>
          {error && <pre className="error">{error}</pre>}
          <button className="primary" onClick={handleInstallNow}>
            Try install again
          </button>
        </section>
      )}

      {phase === "installed" && install && (
        <section>
          <h2>Bridge installed</h2>
          <p>
            Wrote <strong>Markitel_Bridge.mq5</strong> to{" "}
            <strong>{writtenTo.length}</strong> MT5{" "}
            terminal{writtenTo.length === 1 ? "" : "s"}.
          </p>
          {writtenTo.length > 0 && (
            <details className="install-details">
              <summary>What we wrote</summary>
              <ul className="paths">
                {writtenTo.map((p) => (
                  <li key={p}>
                    <code>{p}</code>
                  </li>
                ))}
              </ul>
              <p className="tiny muted">
                URL whitelist:{" "}
                {install.whitelistResults
                  .map(
                    (w) =>
                      `${w.terminal.split("/").slice(-2).join("/")} ${w.edited ? "✓ edited" : w.alreadyPresent ? "✓ present" : "✗ failed"}`,
                  )
                  .join(" · ")}
              </p>
            </details>
          )}
          {writtenTo.length === 0 && (
            <div className="warn">
              We didn't find any MT5 installs to write to. If MT5 is
              installed in a custom location, this fallback is on our
              roadmap — for now, drop{" "}
              <strong>Markitel_Bridge.mq5</strong> into your{" "}
              <code>MQL5/Experts/</code> folder manually.
            </div>
          )}
          {profileNeedsManualDrag && (
            <div className="warn">
              One last manual step inside MT5: drag{" "}
              <strong>Markitel_Bridge</strong> from the Navigator panel
              onto any chart, then click the green AutoTrading button
              up top. (Auto-attach is on the roadmap as Phase 0.)
            </div>
          )}
          <button className="primary" onClick={handleLaunchMt5}>
            Open MT5
          </button>
          <button className="link" onClick={handleInstallNow}>
            Reinstall EA
          </button>
        </section>
      )}

      {phase === "paired-stale" && (
        <section>
          <h2>Already paired</h2>
          <p className="muted">
            {status?.apiKeyPrefix && (
              <>
                Key: <code>{status.apiKeyPrefix}…</code>
                <br />
              </>
            )}
            We've got your API key. If MT5 isn't picking up the EA, run
            the install again below.
          </p>
          <button className="primary" onClick={handleInstallNow}>
            Reinstall EA
          </button>
          <button className="link" onClick={handleLaunchMt5}>
            Open MT5
          </button>
        </section>
      )}

      {phase === "error" && (
        <section>
          <h2>Something went wrong</h2>
          <pre className="error">{error}</pre>
          <button onClick={refreshStatus}>Retry</button>
        </section>
      )}

      {error && phase !== "error" && phase !== "install-error" && (
        <div className="error-inline">{error}</div>
      )}
    </div>
  );
}
