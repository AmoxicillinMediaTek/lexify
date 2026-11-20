"use client";

import CodeMirror from "@uiw/react-codemirror";
import { HighlightStyle, StreamLanguage, syntaxHighlighting } from "@codemirror/language";
import { tags } from "@lezer/highlight";
import { lua } from "@codemirror/legacy-modes/mode/lua";
import { useCallback, useEffect, useMemo, useState } from "react";

const sample = `local function greet(name)
  local message = "Hello, " .. name .. "!"
  print(message)
end

greet("Lua")`;

const levelOptions = ["basic", "medium", "strong", "extreme"] as const;
const luaLanguage = StreamLanguage.define(lua);
const luaHighlight = HighlightStyle.define([
  { tag: tags.keyword, color: "#e6bb87" },
  { tag: tags.string, color: "#b5d89b" },
  { tag: tags.number, color: "#e0c783" },
  { tag: tags.comment, color: "#85948a", fontStyle: "italic" },
  { tag: tags.operator, color: "#d2ddd4" },
  { tag: tags.variableName, color: "#dce7de" },
  { tag: tags.definition(tags.variableName), color: "#a5cfdb" },
  { tag: tags.function(tags.variableName), color: "#9cc7d2" },
]);

type Options = {
  level: string;
  minify: boolean;
  rename: boolean;
  encryptStrings: boolean;
  obfuscateNumbers: boolean;
  obfuscateBooleans: boolean;
  obfuscateTables: boolean;
  controlFlow: boolean;
  deadCode: boolean;
  splitStrings: boolean;
  lookupTables: boolean;
  randomizeLookup: boolean;
  sharedLookup: boolean;
  splitSize: number;
  vm: boolean;
  wrapper: boolean;
  envwrap: boolean;
  allowLoadstring: boolean;
  antitamper: boolean;
};

function presetOptions(level: string, splitSize = 4): Options {
  const medium = level === "medium" || level === "strong" || level === "extreme";
  const strong = level === "strong" || level === "extreme";
  const extreme = level === "extreme";
  return {
    level, splitSize,
    minify: medium,
    rename: medium,
    encryptStrings: extreme,
    obfuscateNumbers: strong,
    obfuscateBooleans: strong,
    obfuscateTables: strong,
    controlFlow: strong,
    deadCode: extreme,
    splitStrings: extreme,
    lookupTables: extreme,
    randomizeLookup: extreme,
    sharedLookup: extreme,
    // The VM and the loader wrapper are always opt-in: no preset turns them on.
    vm: false,
    wrapper: false,
    envwrap: false,
    allowLoadstring: false,
    antitamper: false,
  };
}

const defaults = presetOptions("basic");
const basePath = process.env.NEXT_PUBLIC_BASE_PATH ?? "";

// wasm-bindgen throws plain strings for Rust errors, so read those instead of hiding them.
function describeError(err: unknown) {
  if (err instanceof Error && err.message) return err.message;
  if (typeof err === "string" && err.trim()) return err.trim();
  return "Obfuscation failed.";
}

let wasmObfuscatePromise: Promise<(payload: string) => string> | undefined;
function getWasmObfuscator() {
  if (!wasmObfuscatePromise) {
    wasmObfuscatePromise = (async () => {
      const moduleUrl = new URL(`${basePath}/wasm/lexify_obfuscator.js`, window.location.origin).href;
      const wasmModule = await import(/* webpackIgnore: true */ moduleUrl) as {
        default: (input: URL) => Promise<unknown>;
        wasm_obfuscate: (payload: string) => string;
      };
      await wasmModule.default(new URL(`${basePath}/wasm/lexify_obfuscator_bg.wasm`, window.location.origin));
      return wasmModule.wasm_obfuscate;
    })();
  }
  return wasmObfuscatePromise;
}

function Icon({ name }: { name: "spark" | "copy" | "download" | "reset" | "chevron" }) {
  const paths = {
    spark: <><path d="m12 3 1.9 5.8L20 11l-6.1 2.2L12 19l-1.9-5.8L4 11l6.1-2.2L12 3Z"/><path d="m19 14 .9 2.1L22 17l-2.1.9L19 20l-.9-2.1L16 17l2.1-.9L19 14Z"/></>,
    copy: <><rect x="8" y="8" width="12" height="13" rx="2"/><path d="M16 8V5a2 2 0 0 0-2-2H5a2 2 0 0 0-2 2v10a2 2 0 0 0 2 2h3"/></>,
    download: <><path d="M12 3v12m0 0 4-4m-4 4-4-4"/><path d="M5 17v3h14v-3"/></>,
    reset: <><path d="M3 12a9 9 0 1 0 2.6-6.4L3 8"/><path d="M3 3v5h5"/></>,
    chevron: <path d="m7 10 5 5 5-5"/>,
  };
  return <svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">{paths[name]}</svg>;
}

function EditorPanel({ title, value, onChange, readOnly, side, count, copied, onCopy, onDownload, downloaded }: {
  title: string; value: string; onChange?: (value: string) => void; readOnly?: boolean;
  side: "input" | "output"; count: string; copied?: boolean; onCopy?: () => void;
  downloaded?: boolean; onDownload?: () => void;
}) {
  return (
    <section className={`editor-panel ${side}-panel`} aria-label={`${title} editor`}>
      <header className="panel-head">
        <div className="panel-title"><span className={`file-mark ${side}`} /> <h2>{title}</h2> <span className="file-ext">.lua</span></div>
        <div className="panel-actions">
          {side === "output" && value && <button className="icon-button" type="button" onClick={onCopy} aria-label="Copy output" title="Copy output"><Icon name="copy" />{copied && <span className="tooltip">Copied</span>}</button>}
          {side === "output" && value && <button className="icon-button" type="button" onClick={onDownload} aria-label="Download output as lexifyobfuscated.lua" title="Download lexifyobfuscated.lua"><Icon name="download" />{downloaded && <span className="tooltip">Saved</span>}</button>}
          {side === "input" && <span className="saved-label"><span /> not saved</span>}
        </div>
      </header>
      <div className="editor-wrap">
        <CodeMirror
          value={value}
          onChange={onChange}
          height="100%"
          extensions={[luaLanguage, syntaxHighlighting(luaHighlight)]}
          readOnly={readOnly}
          basicSetup={{ lineNumbers: true, foldGutter: false, highlightActiveLine: !readOnly, highlightSelectionMatches: false, autocompletion: false }}
          className="lua-editor"
          aria-label={`${title} Lua source`}
        />
      </div>
      <footer className="panel-foot"><span>{count}</span><span>Lua 5.4</span></footer>
    </section>
  );
}

function Toggle({ label, checked, onChange, disabled = false }: { label: string; checked: boolean; onChange: (v: boolean) => void; disabled?: boolean }) {
  return <label className={`toggle-row${disabled ? " disabled" : ""}`}><span>{label}</span><input type="checkbox" checked={checked} disabled={disabled} onChange={(e) => onChange(e.target.checked)} /><i aria-hidden="true" /></label>;
}

export default function Obfuscator() {
  const [source, setSource] = useState(sample);
  const [output, setOutput] = useState("");
  const [options, setOptions] = useState<Options>(defaults);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [copied, setCopied] = useState(false);
  const [downloaded, setDownloaded] = useState(false);

  const inputLines = useMemo(() => source.split("\n").length, [source]);
  const outputLines = useMemo(() => output ? output.split("\n").length : 0, [output]);
  const update = <K extends keyof Options>(key: K, value: Options[K]) => setOptions((old) => ({ ...old, [key]: value }));
  const selectPreset = (level: string) => setOptions((old) => ({ ...presetOptions(level, old.splitSize), vm: old.vm, wrapper: old.wrapper, envwrap: old.envwrap, allowLoadstring: old.allowLoadstring, antitamper: old.antitamper }));

  const obfuscate = useCallback(async () => {
    setBusy(true); setError(""); setOutput("");
    // Yield once so the busy state paints before the synchronous wasm call blocks the thread.
    await new Promise((resolve) => setTimeout(resolve, 0));
    try {
      const runObfuscator = await getWasmObfuscator();
      const payload = {
        source,
        ...options,
        level: "basic",
      };
      const result = runObfuscator(JSON.stringify(payload));
      setOutput(result);
    } catch (err) {
      setError(describeError(err));
    } finally {
      setBusy(false);
    }
  }, [source, options]);

  useEffect(() => {
    void getWasmObfuscator().catch((err: unknown) => {
      setError(err instanceof Error ? `Could not load the browser obfuscator: ${err.message}` : "Could not load the browser obfuscator.");
    });
  }, []);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key === "Enter" && !busy) {
        event.preventDefault();
        void obfuscate();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [busy, obfuscate]);

  async function copyOutput() {
    try { await navigator.clipboard.writeText(output); setCopied(true); window.setTimeout(() => setCopied(false), 1600); }
    catch { setError("Clipboard access was blocked by the browser."); }
  }

  function downloadOutput() {
    const url = URL.createObjectURL(new Blob([output], { type: "text/plain;charset=utf-8" }));
    const link = document.createElement("a");
    link.href = url;
    link.download = "lexifyobfuscated.lua";
    document.body.appendChild(link);
    link.click();
    link.remove();
    setDownloaded(true);
    window.setTimeout(() => setDownloaded(false), 1600);
    // Revoked late so slower browsers can still finish writing the file.
    window.setTimeout(() => URL.revokeObjectURL(url), 10000);
  }

  function reset() {
    setSource(sample); setOutput(""); setOptions(defaults); setError(""); setDownloaded(false);
  }


  return (
    <main className="studio-shell">
      <header className="topbar">
        <a className="brand" href="#top" aria-label="Lexify home"><span className="brand-glyph"><i /><i /><i /></span><span>lexify</span></a>

      </header>

      <div id="top" className="workspace">
        <div className="toolbar">
          <div className="toolbar-left" />
          <div className="toolbar-right"><button className="text-button" type="button" onClick={reset}><Icon name="reset" /> Reset</button><button className="run-button" type="button" onClick={obfuscate} disabled={busy || !source.trim()}><Icon name="spark" />{busy ? "Working…" : "Obfuscate"}<kbd>⌘ ↵</kbd></button></div>
        </div>

        <div className="workbench">
          <div className="editors-grid">
            <EditorPanel title="Input" value={source} onChange={setSource} side="input" count={`${inputLines} lines · ${new Blob([source]).size} bytes`} />
            <div className="flow-rail" aria-hidden="true"><span><Icon name="chevron" /></span></div>
            <EditorPanel title="Output" value={output} readOnly side="output" count={output ? `${outputLines} lines · ${new Blob([output]).size} bytes` : "Waiting for source"} copied={copied} onCopy={copyOutput} downloaded={downloaded} onDownload={downloadOutput} />
          </div>
          <aside className="options-panel" aria-label="Obfuscation settings">
            <div className="options-heading"><div><h2>Transform</h2><p>Choose how the source changes</p></div><span className="settings-mark">⌘</span></div>
            <label className="field-label" htmlFor="level">Preset</label>
            <div className="select-wrap"><select id="level" value={options.level} onChange={(e) => selectPreset(e.target.value)}>{levelOptions.map((level) => <option key={level} value={level}>{level[0].toUpperCase() + level.slice(1)}</option>)}</select><Icon name="chevron" /></div>
            <div className="settings-separator" />
            <div className="group-head"><span>Source passes</span><span>01</span></div>
            <Toggle label="Minify output" checked={options.minify} onChange={(v) => update("minify", v)} />
            <Toggle label="Rename locals" checked={options.rename} onChange={(v) => update("rename", v)} />
            <Toggle label="Encode strings" checked={options.encryptStrings} onChange={(v) => update("encryptStrings", v)} />
            <Toggle label="Obfuscate numbers" checked={options.obfuscateNumbers} onChange={(v) => update("obfuscateNumbers", v)} />
            <Toggle label="Obfuscate booleans" checked={options.obfuscateBooleans} onChange={(v) => update("obfuscateBooleans", v)} />
            <Toggle label="Wrap table constructors" checked={options.obfuscateTables} onChange={(v) => update("obfuscateTables", v)} />
            <div className="settings-separator" />
            <div className="group-head"><span>String splitting</span><span>02</span></div>
            <Toggle label="Split string literals" checked={options.splitStrings} onChange={(v) => update("splitStrings", v)} />
            <div className="sub-options">
              <Toggle label="Use lookup table" checked={options.lookupTables} disabled={!options.splitStrings} onChange={(v) => update("lookupTables", v)} />
              <Toggle label="Randomize fragments" checked={options.randomizeLookup} disabled={!options.splitStrings} onChange={(v) => update("randomizeLookup", v)} />
              <Toggle label="Share one table" checked={options.sharedLookup} disabled={!options.splitStrings || !options.lookupTables} onChange={(v) => update("sharedLookup", v)} />
              <label className={`range-row${!options.splitStrings ? " disabled" : ""}`}><span>Max fragment</span><span>{options.splitSize} bytes</span><input aria-label="Maximum fragment size" type="range" min="1" max="16" step="1" disabled={!options.splitStrings} value={options.splitSize} onChange={(e) => update("splitSize", Number(e.target.value))} /></label>
            </div>
            <div className="settings-separator" />
            <div className="group-head"><span>Control flow</span><span>03</span></div>
            <Toggle label="Normalize conditions" checked={options.controlFlow} onChange={(v) => update("controlFlow", v)} />
            <Toggle label="Add dead code" checked={options.deadCode} onChange={(v) => update("deadCode", v)} />
            <div className="settings-separator" />
            <div className="group-head"><span>Virtual machine</span><span>04</span></div>
            <Toggle label="Virtualize to bytecode" checked={options.vm} onChange={(v) => update("vm", v)} />
            {options.vm && <p className="notice" role="note"><strong>Not real protection.</strong> The VM only reshapes code and can be bypassed by hand. Each run is random — press Obfuscate again to get a new layout.</p>}
            <Toggle label="Loader wrapper" checked={options.wrapper} onChange={(v) => update("wrapper", v)} />
            <Toggle label="Protect global environment" checked={options.envwrap} onChange={(v) => update("envwrap", v)} />
            {options.envwrap && <p className="notice" role="note">Global reads and writes are routed through a proxy; raw access to the backing table is blocked. This raises the cost of casual inspection but is not a sandbox against a hostile host or debug library.</p>}
            <Toggle label="Allow load / loadstring" checked={options.allowLoadstring} onChange={(v) => update("allowLoadstring", v)} />
            <div className="settings-separator" />
            <div className="group-head"><span>Guard</span><span>05</span></div>
            <Toggle label="Anti-tamper check" checked={options.antitamper} onChange={(v) => update("antitamper", v)} />
            {options.antitamper && <p className="notice" role="note">The chunk fingerprints its own file and re-checks the interpreter at startup, then stops with an integrity error if either changed. Only checks when the script is run from a file — stdin and REPL runs skip it.</p>}
          </aside>
        </div>

        {(error || busy) && <div className={`status-bar${error ? " is-error" : ""}`} role="status" aria-live="polite"><span className="status-led" />{error || "Running obfuscation…"}</div>}
      </div>
    </main>
  );
}
