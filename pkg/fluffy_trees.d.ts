/* tslint:disable */
/* eslint-disable */

export function look(dx: number, dy: number): void;

export function reset_view(): void;

export function run(): Promise<void>;

export function set_billboard(amount: number): void;

export function set_speed(kmh: number): void;

export function set_style(toon: number): void;

export function set_tree_count(count: number): void;

export function set_wind(strength: number): void;

export function toggle_pause(): boolean;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly look: (a: number, b: number) => void;
    readonly reset_view: () => void;
    readonly run: () => any;
    readonly set_billboard: (a: number) => void;
    readonly set_speed: (a: number) => void;
    readonly set_style: (a: number) => void;
    readonly set_tree_count: (a: number) => void;
    readonly set_wind: (a: number) => void;
    readonly toggle_pause: () => number;
    readonly wasm_bindgen_e53c4c9346335196___convert__closures_____invoke___f64______true_: (a: number, b: number, c: number) => void;
    readonly wasm_bindgen_e53c4c9346335196___convert__closures_____invoke___js_sys_75ab61f67c10e5dd___Function_fn_wasm_bindgen_e53c4c9346335196___JsValue_____wasm_bindgen_e53c4c9346335196___sys__Undefined___js_sys_75ab61f67c10e5dd___Function_fn_wasm_bindgen_e53c4c9346335196___JsValue_____wasm_bindgen_e53c4c9346335196___sys__Undefined_______true_: (a: number, b: number, c: any, d: any) => void;
    readonly wasm_bindgen_e53c4c9346335196___convert__closures_____invoke___wasm_bindgen_e53c4c9346335196___JsValue__core_37ee0ce0c4cd57d5___result__Result_____wasm_bindgen_e53c4c9346335196___JsError___true_: (a: number, b: number, c: any) => [number, number];
    readonly wasm_bindgen_e53c4c9346335196___convert__closures_____invoke___wasm_bindgen_e53c4c9346335196___JsValue______true_: (a: number, b: number, c: any) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_destroy_closure: (a: number, b: number) => void;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
