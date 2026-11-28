import { effect, platform, reduceMotion, signal, View, windowSize } from "nsh";
import { Text } from "./Text";
import type { ViewRef } from "nsh";
import { Dialog } from "nsh/base";
import { t } from "../preferences";
import { Action } from "./Action";

// GTK can blur a widget subtree, while the native dialog is drawn in a separate overlay.
export const blurWindowContent = signal(false);

interface ViewerProps {
  identity: string;
  title: string;
  closeId: string;
  close: () => void;
  photo: () => ReturnType<typeof View>;
  details: () => ReturnType<typeof View>;
  actions: () => ReturnType<typeof View>;
}

export function PhotoViewer(props: ViewerProps) {
  effect(() => {
    if (platform.os !== "linux") return;
    blurWindowContent.set(true);
    return () => blurWindowContent.set(false);
  });
  let close: ViewRef | undefined;
  const zoom = signal(1),
    offset = signal({ x: 0, y: 0 });
  let drag: { x: number; y: number } | undefined;
  const width = () =>
    Math.max(1, Math.min(1400, windowSize().width - 32, ((windowSize().height - 108) * 16) / 9));
  const height = () => (width() * 9) / 16;
  function pan(x: number, y: number, scale: number) {
    const clamp = (v: number, size: number) => Math.max(-size, Math.min(size, v));
    offset.set({ x: clamp(x, (width() * (scale - 1)) / 2), y: clamp(y, (height() * (scale - 1)) / 2) });
  }
  function magnify(value: number, x = width() / 2, y = height() / 2) {
    const next = Math.min(4, Math.max(1, value)),
      ratio = next / zoom();
    pan(
      offset().x * ratio + (x - width() / 2) * (1 - ratio),
      offset().y * ratio + (y - height() / 2) * (1 - ratio),
      next,
    );
    zoom.set(next);
  }
  effect(() => {
    void props.identity;
    zoom.set(1);
    offset.set({ x: 0, y: 0 });
    drag = undefined;
  });
  return (
    <Dialog
      open
      animation={reduceMotion() ? "none" : "fade"}
      class="m-auto"
      style={{ width: width(), alignSelf: "center" }}
      accessibilityLabel={props.title}
      backdropStyle={{ backdropFilter: "blur(18px)", backgroundColor: "rgba(100,108,123,0.23)" }}
      onOpen={() => close?.focus()}
      onOpenChange={({ open }) => {
        if (!open) props.close();
      }}
    >
      <View class="items-center gap-2">
        <View
          testID="preview-image"
          class="overflow-hidden rounded-xl"
          style={{ width: width(), height: height() }}
          onWheel={(e) =>
            magnify(
              zoom() * Math.exp(-e.delta.y * (e.deltaMode === "lines" ? 0.12 : 0.009)),
              e.position.x,
              e.position.y,
            )
          }
          onMouseMove={(e) => {
            if (e.pressedButton !== "left" || zoom() === 1) {
              drag = undefined;
              return;
            }
            if (drag) pan(offset().x + e.position.x - drag.x, offset().y + e.position.y - drag.y, zoom());
            drag = e.position;
          }}
          onMouseUp={() => {
            drag = undefined;
          }}
          onMouseLeave={() => {
            drag = undefined;
          }}
        >
          <View
            style={{
              width: "100%",
              height: "100%",
              transform: [{ translateX: offset().x }, { translateY: offset().y }, { scale: zoom() }],
            }}
          >
            {props.photo()}
          </View>
          <View
            class="absolute bottom-0 inset-x-0 items-center px-4 pt-5 pb-3"
            style={{
              pointerEvents: "none",
              backgroundImage: "linear-gradient(180deg, rgba(0,0,0,0), rgba(0,0,0,0.65))",
            }}
          >
            <Text class="text-sm text-white font-semibold text-center line-clamp-1">{props.title}</Text>
            {props.details()}
          </View>
          <View
            class="absolute top-2 right-2 rounded-full border border-white/50 overflow-hidden"
            style={{ backgroundColor: "rgba(35,41,48,0.38)", backdropFilter: "blur(12px)" }}
          >
            <Action
              id={props.closeId}
              icon="x"
              onGlass
              label={t("Close")}
              press={props.close}
              buttonRef={(node) => {
                close = node;
              }}
            />
          </View>
        </View>
        <View class="flex-row items-center justify-center rounded-full bg-card px-2 py-1">
          {props.actions()}
        </View>
      </View>
    </Dialog>
  );
}
