/* tslint:disable */
/* eslint-disable */

/**
 * Get project metadata without full scheduling (for quick preview)
 */
export function get_project_info(project_source: string): string;

/**
 * Initialize panic hook for better error messages in console
 */
export function init(): void;

/**
 * Schedule a project from DSL string and return JSON result
 */
export function schedule(project_source: string): string;

/**
 * Update a task's completion percentage in the project source
 */
export function update_task_progress(project_source: string, task_id: string, new_percent: number): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
  readonly memory: WebAssembly.Memory;
  readonly get_project_info: (a: number, b: number, c: number) => void;
  readonly schedule: (a: number, b: number, c: number) => void;
  readonly update_task_progress: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
  readonly init: () => void;
  readonly __wbindgen_export: (a: number, b: number, c: number) => void;
  readonly __wbindgen_export2: (a: number, b: number) => number;
  readonly __wbindgen_export3: (a: number, b: number, c: number, d: number) => number;
  readonly __wbindgen_add_to_stack_pointer: (a: number) => number;
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
