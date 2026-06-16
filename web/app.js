// Static web app for aozora-proof — loads the wasm-pack module and runs the
// character-level checks entirely in the browser.
import init, { checkJson, gaijiSearchJson } from "./pkg/aozora_proof_wasm.js";

const SEV = {
  error: { order: 0, label: "エラー" },
  warning: { order: 1, label: "警告" },
  note: { order: 2, label: "注意" },
};

function el(tag, cls, text) {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text !== undefined) e.textContent = text;
  return e;
}

// 1-based line number for a UTF-8 byte offset (the engine reports byte spans).
function lineOf(bytes, byteOffset) {
  let line = 1;
  const n = Math.min(byteOffset, bytes.length);
  for (let i = 0; i < n; i++) if (bytes[i] === 0x0a) line += 1;
  return line;
}

function renderFindings(text) {
  const out = document.getElementById("findings");
  out.replaceChildren();
  let data;
  try {
    data = JSON.parse(checkJson(text)).data || [];
  } catch {
    data = [];
  }
  if (data.length === 0) {
    out.appendChild(el("li", "ok", "✓ 指摘はありません"));
    return;
  }
  const bytes = new TextEncoder().encode(text);
  data.sort(
    (a, b) =>
      (SEV[a.severity]?.order ?? 9) - (SEV[b.severity]?.order ?? 9) || a.span.start - b.span.start,
  );
  for (const f of data) {
    const li = el("li", `finding ${f.severity}`);
    li.appendChild(el("span", "badge", SEV[f.severity]?.label ?? f.severity));
    li.appendChild(el("span", "loc", `L${lineOf(bytes, f.span.start)}`));
    li.appendChild(el("span", "msg", f.message));
    li.appendChild(el("code", "code", f.code));
    out.appendChild(li);
  }
}

function renderGaiji(query) {
  const out = document.getElementById("gaiji-results");
  out.replaceChildren();
  if (!query) return;
  let matches;
  try {
    matches = JSON.parse(gaijiSearchJson(query)).matches || [];
  } catch {
    matches = [];
  }
  if (matches.length === 0) {
    out.appendChild(el("li", "ok", "該当なし"));
    return;
  }
  for (const m of matches.slice(0, 60)) {
    const li = el("li", "gaiji-row");
    li.appendChild(el("span", "ch", m.char));
    li.appendChild(el("code", "cp", m.codepoint));
    li.appendChild(el("span", "desc", m.description));
    out.appendChild(li);
  }
}

function debounce(fn, ms) {
  let timer;
  return (...args) => {
    clearTimeout(timer);
    timer = setTimeout(() => fn(...args), ms);
  };
}

(async () => {
  await init();
  const input = document.getElementById("input");
  input.addEventListener(
    "input",
    debounce(() => renderFindings(input.value), 200),
  );
  renderFindings(input.value);

  const query = document.getElementById("gaiji-query");
  query.addEventListener(
    "input",
    debounce(() => renderGaiji(query.value.trim()), 200),
  );
})();
