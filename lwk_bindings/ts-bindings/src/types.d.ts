declare module '*.wasm' {
  const content: string;
  export default content;
}

declare module '*wasm-bindgen/index.js' {
  interface InitInput {
    module_or_path?: string | URL | Request | Response | ArrayBuffer | WebAssembly.Module;
  }
  export default function init(input?: InitInput): Promise<void>;
}
