import { ipc } from "../../../lib/ipc";
import type { FileNavigationGateway } from "../application/fileNavigationGateway";

export const fileNavigationGateway: FileNavigationGateway = {
  resolveEditorFileReference: ipc.resolveEditorFileReference,
};
