import {
  handleResizeMouseDown,
  type WindowResizeDirection,
} from "../../contexts/shell-ui/application/windowDrag";

const HANDLE_DIRECTIONS: Array<{ className: string; direction: WindowResizeDirection }> = [
  { className: "linux-window-resize-handle-north", direction: "North" },
  { className: "linux-window-resize-handle-south", direction: "South" },
  { className: "linux-window-resize-handle-east", direction: "East" },
  { className: "linux-window-resize-handle-west", direction: "West" },
  { className: "linux-window-resize-handle-north-west", direction: "NorthWest" },
  { className: "linux-window-resize-handle-north-east", direction: "NorthEast" },
  { className: "linux-window-resize-handle-south-west", direction: "SouthWest" },
  { className: "linux-window-resize-handle-south-east", direction: "SouthEast" },
];

interface CustomWindowResizeHandlesProps {
  canResize: boolean;
}

export function CustomWindowResizeHandles({ canResize }: CustomWindowResizeHandlesProps) {
  if (!canResize) {
    return null;
  }

  return (
    <>
      {HANDLE_DIRECTIONS.map(({ className, direction }) => (
        <div
          key={direction}
          className={`linux-window-resize-handle ${className}`}
          onMouseDown={(event) => handleResizeMouseDown(direction, event)}
        />
      ))}
    </>
  );
}
