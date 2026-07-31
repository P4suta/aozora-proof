// Browser-only bridge to the wasm-pack build. The wasm module is imported
// dynamically inside ensureReady() so it never loads during prerender (SSR),
// and the checks only run client-side after init() resolves.
type WasmModule = {
	default: (module_or_path?: unknown) => Promise<unknown>;
	checkJson: (text: string) => string;
	gaijiSearchJson: (query: string) => string;
	ruleTitlesJson: () => string;
	ruleCatalogJson: () => string;
};

export type FixAlternative = {
	applicability: 'safe' | 'review';
	label: string;
	operation: string;
	edit?: {
		span: { start: number; end: number };
		replacement: string;
	};
};

export type Finding = {
	code: string;
	severity: string;
	utf8ByteSpan: { start: number; end: number };
	position: { line: number; column: number; endLine: number; endColumn: number };
	canonicalMessage: string;
	fixAlternatives: FixAlternative[];
};

export type GaijiMatch = { description: string; char: string; codepoint: string };
export type CheckState =
	| { status: 'loading' }
	| { status: 'ready'; findings: Finding[] }
	| { status: 'error'; message: string };
export type SearchState =
	| { status: 'loading' }
	| { status: 'ready'; matches: GaijiMatch[] }
	| { status: 'error'; message: string };

let readyPromise: Promise<void> | undefined;
let api: WasmModule | undefined;
let titles: Record<string, string> = {};

export function ensureReady(): Promise<void> {
	if (!readyPromise) {
		readyPromise = (async () => {
			const mod = (await import('$lib/pkg/aozora_proof_wasm.js')) as unknown as WasmModule;
			await mod.default();
			api = mod;
			titles = JSON.parse(mod.ruleTitlesJson());
		})();
	}
	return readyPromise;
}

export function check(text: string): CheckState {
	if (!api) return { status: 'loading' };
	try {
		const findings = JSON.parse(api.checkJson(text)).files?.[0]?.findings;
		if (!Array.isArray(findings)) throw new Error('校正結果の形式が不正です。');
		return { status: 'ready', findings };
	} catch (error) {
		return { status: 'error', message: errorMessage(error) };
	}
}

export function searchGaiji(query: string): SearchState {
	if (!api) return { status: 'loading' };
	try {
		const matches = JSON.parse(api.gaijiSearchJson(query)).matches;
		if (!Array.isArray(matches)) throw new Error('外字検索結果の形式が不正です。');
		return { status: 'ready', matches };
	} catch (error) {
		return { status: 'error', message: errorMessage(error) };
	}
}

// Human-readable Japanese label for a finding code, or undefined when the code
// has no documented title (e.g. notation findings).
export function ruleTitle(code: string): string | undefined {
	return titles[code];
}

function errorMessage(error: unknown): string {
	return error instanceof Error ? error.message : String(error);
}
