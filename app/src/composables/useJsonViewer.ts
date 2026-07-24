import { ref, computed, Ref } from "vue";

export type CopyFormat = "pretty" | "minified" | "jsonpath" | "jsonpath-wildcard" | "value" | "branch";
export type ViewMode = "tree" | "flatten" | "schema" | "diff" | "query" | "format";
export type FormatStyle = "pretty2" | "pretty4" | "minified" | "compact";

export interface QueryResult {
  path: string[];
  value: unknown;
  rootObject: unknown; // Full parent object/array for context
}

interface DiffNode {
  type: "added" | "removed" | "changed" | "unchanged";
  key?: string;
  value?: unknown;
  oldValue?: unknown;
}

export function useJsonViewer() {
  const input = ref<string>("");
  const parsed: Ref<unknown | null> = ref(null);
  const parseError = ref<string | null>(null);
  const search = ref<string>("");
  const sortKeys = ref<boolean>(false);
  const mode: Ref<ViewMode> = ref("format");
  const formatStyle: Ref<FormatStyle> = ref("pretty2");
  const diffInput = ref<string>("");
  const diffParsed: Ref<unknown | null> = ref(null);
  const selectedPath: Ref<string[]> = ref([]);
  const expanded: Ref<Set<string>> = ref(new Set());

  function serializePath(path: string[]): string {
    return JSON.stringify(path);
  }

  function parse() {
    parseError.value = null;
    try {
      if (!input.value.trim()) {
        parsed.value = null;
        return;
      }
      parsed.value = JSON.parse(input.value);
      selectedPath.value = [];
      expanded.value = new Set();
    } catch (e) {
      parseError.value = e instanceof Error ? e.message : String(e);
      parsed.value = null;
    }
  }

  function clearAll() {
    input.value = "";
    parsed.value = null;
    parseError.value = null;
    search.value = "";
    diffInput.value = "";
    diffParsed.value = null;
    selectedPath.value = [];
    expanded.value = new Set();
  }

  function toggleExpand(path: string[]) {
    const key = serializePath(path);
    if (expanded.value.has(key)) {
      expanded.value.delete(key);
    } else {
      expanded.value.add(key);
    }
  }

  function selectNode(path: string[]) {
    selectedPath.value = [...path];
  }

  function getNodeValue(path: string[]): unknown {
    let current = parsed.value;
    for (const segment of path) {
      if (typeof current === "object" && current !== null) {
        const key = isNaN(Number(segment)) ? segment : Number(segment);
        current = (current as Record<string, unknown>)[key];
      } else {
        return undefined;
      }
    }
    return current;
  }

  function matchesSearch(path: string[], value: unknown): boolean {
    if (!search.value) return true;
    const query = search.value.toLowerCase();
    const pathStr = path.join(".").toLowerCase();
    if (pathStr.includes(query)) return true;
    const valueStr = JSON.stringify(value).toLowerCase();
    return valueStr.includes(query);
  }

  function jsonPath(path: string[], wildcard = false): string {
    let result = "$";
    for (const segment of path) {
      if (isNaN(Number(segment))) {
        result += `.${segment}`;
      } else {
        const idx = Number(segment);
        result += wildcard ? "[]" : `[${idx}]`;
      }
    }
    return result;
  }

  function flatten(value: unknown = parsed.value, prefix = ""): string[] {
    const lines: string[] = [];
    if (value === null) {
      if (prefix) lines.push(`${prefix} = null`);
    } else if (Array.isArray(value)) {
      if (value.length === 0) {
        if (prefix) lines.push(`${prefix} = []`);
      } else {
        value.forEach((v, i) => {
          const newPrefix = prefix ? `${prefix}[${i}]` : `[${i}]`;
          lines.push(...flatten(v, newPrefix));
        });
      }
    } else if (typeof value === "object") {
      const obj = value as Record<string, unknown>;
      const keys = Object.keys(obj).sort();
      if (keys.length === 0) {
        if (prefix) lines.push(`${prefix} = {}`);
      } else {
        keys.forEach((k) => {
          const newPrefix = prefix ? `${prefix}.${k}` : k;
          lines.push(...flatten(obj[k], newPrefix));
        });
      }
    } else if (typeof value === "string") {
      if (prefix) lines.push(`${prefix} = "${value}"`);
    } else {
      if (prefix) lines.push(`${prefix} = ${JSON.stringify(value)}`);
    }
    return lines;
  }

  function generateSchema(value: unknown): object {
    if (value === null) {
      return { type: "null" };
    }
    if (Array.isArray(value)) {
      const itemSchemas = value.map((v) => generateSchema(v));
      const merged = mergeSchemas(itemSchemas);
      return { type: "array", items: merged };
    }
    if (typeof value === "object") {
      const obj = value as Record<string, unknown>;
      const properties: Record<string, object> = {};
      const required: string[] = [];
      Object.keys(obj).forEach((k) => {
        properties[k] = generateSchema(obj[k]);
        required.push(k);
      });
      return { type: "object", properties, required };
    }
    const typeMap: Record<string, string> = {
      string: "string",
      number: "number",
      boolean: "boolean",
    };
    const type = typeMap[typeof value] || "unknown";
    return { type };
  }

  function mergeSchemas(schemas: object[]): object {
    if (schemas.length === 0) return { type: "unknown" };
    const first = schemas[0] as Record<string, unknown>;
    if (schemas.length === 1) return first;
    // For simplicity, just return the first schema. A full implementation
    // would merge properties, union types, etc.
    return first;
  }

  function computeDiff(a: unknown, b: unknown): Record<string, DiffNode> {
    const result: Record<string, DiffNode> = {};
    // Simplified diff: just mark everything as unchanged.
    // A full implementation would recursively diff objects/arrays.
    result["root"] = { type: "unchanged", value: a };
    return result;
  }

  function copyAs(format: CopyFormat): string {
    const value = parsed.value !== null ? parsed.value : null;
    if (value === null) {
      return "null";
    }

    switch (format) {
      case "pretty":
        return JSON.stringify(value, null, 2);
      case "minified":
        return JSON.stringify(value);
      case "jsonpath":
        return jsonPath(selectedPath.value, false);
      case "jsonpath-wildcard":
        return jsonPath(selectedPath.value, true);
      case "value": {
        const nodeValue = getNodeValue(selectedPath.value);
        return typeof nodeValue === "string" ? nodeValue : JSON.stringify(nodeValue);
      }
      case "branch": {
        const nodeValue = getNodeValue(selectedPath.value);
        return JSON.stringify(nodeValue, null, 2);
      }
      default:
        return JSON.stringify(value, null, 2);
    }
  }

  function parseDiffInput() {
    try {
      if (!diffInput.value.trim()) {
        diffParsed.value = null;
        return;
      }
      diffParsed.value = JSON.parse(diffInput.value);
    } catch (e) {
      console.error("Diff input parse error:", e);
      diffParsed.value = null;
    }
  }

  function isJsonPathQuery(query: string): boolean {
    // Detect JSONPath-like patterns: contains [] or starts with root/$ and has multiple segments
    return /\[\]|\[\d+\]|\.\w+/.test(query);
  }

  function formatJson(style: FormatStyle): string {
    if (parsed.value === null) return "";
    switch (style) {
      case "pretty2":
        return JSON.stringify(parsed.value, null, 2);
      case "pretty4":
        return JSON.stringify(parsed.value, null, 4);
      case "minified":
        return JSON.stringify(parsed.value);
      case "compact":
        // One object/array per line, minified contents
        return JSON.stringify(parsed.value, null, 0).replace(/\{/g, "{\n").replace(/\}/g, "\n}").replace(/\[/g, "[\n").replace(/\]/g, "\n]").replace(/,/g, ",\n");
      default:
        return JSON.stringify(parsed.value, null, 2);
    }
  }

  function evaluateJsonPath(query: string): QueryResult[] {
    if (!parsed.value || typeof parsed.value !== "object") return [];

    const results: QueryResult[] = [];

    // Simple JSONPath parser for patterns like: root[].field or root[].arr[].field or results[].field
    // Remove 'root' prefix if present, also handle $ and plain key start
    let path = query.replace(/^root/, "").replace(/^\$/, "");
    if (!path) path = ".";

    // Ensure path starts with . or [ for consistent parsing
    if (!path.startsWith(".") && !path.startsWith("[")) {
      path = "." + path;
    }

    // Parse the path into segments
    const segments: Array<{ type: "index" | "key"; value: string | number }> = [];

    // Match patterns like [] (wildcard), [0] (index), .key, or [key] (object key access)
    const regex = /(\[\]|\[\d+\]|\[[\w]+\]|\.\w+)/g;
    let match;
    while ((match = regex.exec(path)) !== null) {
      const segment = match[0];
      if (segment === "[]") {
        segments.push({ type: "index", value: "*" });
      } else if (segment.startsWith("[") && segment.endsWith("]")) {
        const inner = segment.slice(1, -1);
        const idx = parseInt(inner);
        if (!isNaN(idx)) {
          // Numeric index
          segments.push({ type: "index", value: idx });
        } else {
          // Object key in bracket notation [key]
          segments.push({ type: "key", value: inner });
        }
      } else if (segment.startsWith(".")) {
        segments.push({ type: "key", value: segment.slice(1) });
      }
    }

    if (segments.length === 0) return [];

    // Recursively walk the tree and collect matches
    function walk(current: unknown, segmentIndex: number, currentPath: string[]): void {
      if (segmentIndex >= segments.length) {
        results.push({ path: currentPath, value: current, rootObject: parsed.value });
        return;
      }

      const segment = segments[segmentIndex];

      if (segment.type === "index" && segment.value === "*") {
        // Wildcard: iterate all array elements
        if (Array.isArray(current)) {
          current.forEach((item, idx) => {
            walk(item, segmentIndex + 1, [...currentPath, String(idx)]);
          });
        }
      } else if (segment.type === "index" && typeof segment.value === "number") {
        // Specific index
        if (Array.isArray(current) && current[segment.value] !== undefined) {
          walk(current[segment.value], segmentIndex + 1, [...currentPath, String(segment.value)]);
        }
      } else if (segment.type === "key") {
        // Object key
        if (typeof current === "object" && current !== null) {
          const obj = current as Record<string, unknown>;
          if (segment.value in obj) {
            walk(obj[segment.value as string], segmentIndex + 1, [...currentPath, segment.value as string]);
          }
        }
      }
    }

    walk(parsed.value, 0, []);
    return results;
  }

  // Plain-text search: collect nodes whose key matches the query, plus scalar
  // leaves whose value contains it. Containers are matched by key only so a
  // hit near the root doesn't flood the results with every descendant.
  function searchResults(query: string): QueryResult[] {
    const results: QueryResult[] = [];
    const q = query.trim().toLowerCase();
    if (!q || parsed.value === null) return results;

    function walk(current: unknown, path: string[]): void {
      const key = path.length > 0 ? path[path.length - 1].toLowerCase() : "";
      const isContainer = typeof current === "object" && current !== null;
      if (key.includes(q)) {
        results.push({ path, value: current, rootObject: parsed.value });
      } else if (!isContainer) {
        const valueStr =
          typeof current === "string" ? current : JSON.stringify(current);
        if (valueStr !== undefined && valueStr.toLowerCase().includes(q)) {
          results.push({ path, value: current, rootObject: parsed.value });
        }
      }
      if (Array.isArray(current)) {
        current.forEach((v, i) => walk(v, [...path, String(i)]));
      } else if (isContainer) {
        const obj = current as Record<string, unknown>;
        for (const k of Object.keys(obj)) {
          walk(obj[k], [...path, k]);
        }
      }
    }

    walk(parsed.value, []);
    return results;
  }

  return {
    input,
    parsed,
    parseError,
    search,
    sortKeys,
    mode,
    formatStyle,
    diffInput,
    diffParsed,
    selectedPath,
    expanded,
    parse,
    clearAll,
    toggleExpand,
    selectNode,
    getNodeValue,
    matchesSearch,
    jsonPath,
    flatten,
    generateSchema,
    computeDiff,
    copyAs,
    parseDiffInput,
    serializePath,
    isJsonPathQuery,
    evaluateJsonPath,
    searchResults,
    formatJson,
  };
}
