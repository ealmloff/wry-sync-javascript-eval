import { JSHeap } from "./heap.ts";
import "./ipc.ts";
import { DataDecoder, DataEncoder } from "./encoding.ts";
import { evaluate_from_rust_binary } from "./ipc.ts";

/**
 * Function registry - maps function IDs to their serialization/deserialization specs.
 *
 * Each function has:
 * - Argument deserialization: how to read args from decoder
 * - Return serialization: how to write return value to encoder
 */
type FunctionSpec = (decoder: DataDecoder, encoder: DataEncoder) => void;

let functionRegistry: FunctionSpec[] = [];
window.setFunctionRegistry = (registry: FunctionSpec[]) => {
  functionRegistry = registry;
};
window.evaluate_from_rust_binary = evaluate_from_rust_binary;
window.jsHeap = new JSHeap();

declare global {
  interface Window {
    functionRegistry: FunctionSpec[];
    setFunctionRegistry: (registry: FunctionSpec[]) => void;
    evaluate_from_rust_binary: (fnId: number, dataBase64: string) => unknown;
    jsHeap: JSHeap;
  }
}
