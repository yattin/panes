import { destroyCachedEditor } from "../../../components/editor/CodeMirrorEditor";

export function destroyEditorRuntimeCache(cacheKey: string): void {
  destroyCachedEditor(cacheKey);
}
