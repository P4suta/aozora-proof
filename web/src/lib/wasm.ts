// Browser-only bridge to the wasm-pack build. The wasm module is imported
// dynamically inside ensureReady() so it never loads during prerender (SSR),
// and the checks only run client-side after init() resolves.
type WasmModule = {
	default: (module_or_path?: unknown) => Promise<unknown>;
	checkJson: (text: string) => string;
	gaijiSearchJson: (query: string) => string;
	ruleTitlesJson: () => string;
};

export type Finding = {
	code: string;
	severity: string;
	span: { start: number; end: number };
	message: string;
};

export type GaijiMatch = { description: string; char: string; codepoint: string };

let readyPromise: Promise<void> | undefined;
let api: WasmModule | undefined;
let titles: Record<string, string> = {};

export function ensureReady(): Promise<void> {
	if (!readyPromise) {
		readyPromise = (async () => {
			const mod = (await import('$lib/pkg/aozora_proof_wasm.js')) as unknown as WasmModule;
			await mod.default();
			api = mod;
			try {
				titles = JSON.parse(mod.ruleTitlesJson());
			} catch {
				titles = {};
			}
		})();
	}
	return readyPromise;
}

// Findings for `text`, or [] before the wasm module is ready.
export function check(text: string): Finding[] {
	if (!api) return [];
	try {
		return JSON.parse(api.checkJson(text)).data ?? [];
	} catch {
		return [];
	}
}

// 外字注記辞書 matches for `query`, or [] before the wasm module is ready.
export function searchGaiji(query: string): GaijiMatch[] {
	if (!api) return [];
	try {
		return JSON.parse(api.gaijiSearchJson(query)).matches ?? [];
	} catch {
		return [];
	}
}

// Human-readable Japanese label for a finding code, or undefined when the code
// has no documented title (e.g. notation findings).
export function ruleTitle(code: string): string | undefined {
	return titles[code];
}
