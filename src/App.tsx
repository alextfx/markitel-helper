import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api } from "./lib/api";
import type { HelperStatus, InstallEaResult, PairedEvent } from "./lib/types";

type Phase =
  | "loading"
  | "unpaired"
  | "pairing"
  | "paired"
  | "installing"
  | "installed"
  | "error";

export function App() {
  const [phase, setPhase] = useState<Phase>("loading");
  const [status, setStatus] = useState<HelperStatus | null>(null);
  const [paired, setPaired] = useState<PairedEvent | null>(null);
  const [install, setInstall] = useState<InstallEaResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [manualCode, setManualCode] = useState("");

  const refreshStatus = useCallback(async () => {
    try {
      const s = await api.status();
      setStatus(s);
      setPhase(s.paired ? "paired" : "unpaired");
    } catch (e) {
      setError(String(e));
      setPhase("error");
    }
  }, []);

  useEffect(() => {
    refreshStatus();
  }, [refreshStatus]);

  useEffect(() => {
    const un1 = listen<PairedEvent>("helper://paired", (e) => {
      setPaired(e.payload);
      setPhase("paired");
      refreshStatus();
    });
    const un2 = listen<string>("helper://pair-error", (e) => {
      setError(e.payload);
      setPhase("error");
    });
    const un3 = listen<void>("helper://rotate-key", async () => {
      try {
        await api.rotateKey();
        refreshStatus();
      } catch (err) {
        setError(String(err));
      }
    });
    const un4 = listen<void>("helper://reinstall-ea", () => {
      handleInstall();
    });
    return () => {
      un1.then((f) => f());
      un2.then((f) => f());
      un3.then((f) => f());
      un4.then((f) => f());
    };
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
      await api.pairWithCode(manualCode.trim());
    } catch (e) {
      setError(String(e));
    }
  };

  const handleInstall = async () => {
    setError(null);
    setPhase("installing");
    try {
      const running = await api.isMt5Running();
      if (running) {
        setError("MT5 is running — please close it first, then try again.");
        setPhase("paired");
        return;
      }
      const result = await api.installEa();
      setInstall(result);
      setPhase("installed");
    } catch (e) {
      setError(String(e));
      setPhase("paired");
    }
  };

  const handleLaunchMt5 = async () => {
    try {
      await api.launchMt5();
    } catch (e) {
      setError(String(e));
    }
  };

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
            Click below to open Markitel in your browser. You'll click
            "Connect MT5" and the helper takes over from there.
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
            to reopen the helper with your pairing code.
          </p>
        </section>
      )}

      {(phase === "paired" || phase === "installing") && (
        <section>
          <h2>Ready to install the bridge</h2>
          <p className="muted">
            {paired?.userEmail ? `Paired as ${paired.userEmail}.` : "Paired."} Key:{" "}
            <code>{status?.apiKeyPrefix ?? "…"}…</code>
          </p>
          <button
            className="primary"
            onClick={handleInstall}
            disabled={phase === "installing"}
          >
            {phase === "installing" ? "Installing…" : "Install MT5 Bridge"}
          </button>
          <p className="tiny muted">
            Make sure MT5 is closed. We'll write the EA, whitelist the
            markitel.com URL, and (when ready) create a "Markitel"
            chart profile.
          </p>
        </section>
      )}

      {phase === "installed" && install && (
        <section>
          <h2>Installed!</h2>
          <p>
            Wrote EA to <strong>{install.writtenTo.length}</strong>{" "}
            terminal{install.writtenTo.length === 1 ? "" : "s"}.
          </p>
          {install.profileResults.some(
            (p) => p.outcome === "NeedsManualDrag",
          ) && (
            <div className="warn">
              One or more terminals still need you to drag{" "}
              <strong>Markitel_Bridge</strong> from the Navigator onto
              any chart after you open MT5. (We'll auto-attach in an
              upcoming release.)
            </div>
          )}
          <button className="primary" onClick={handleLaunchMt5}>
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

      {error && phase !== "error" && (
        <div className="error-inline">{error}</div>
      )}
    </div>
  );
}
