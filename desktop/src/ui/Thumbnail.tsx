import { effect, Image } from "nsh";
import { Library } from "wallpaper-native";

// Keep track of mounted cards and evict idle native decodes when the working set crosses 120.
const visible = new Map<string, number>();
const recent = new Set<string>();
export function releaseUnusedImages() {
  Library.trimImages();
  recent.clear();
  for (const url of visible.keys()) recent.add(url);
}
export function Thumbnail(props: { src: string; failed?: () => void }) {
  effect(() => {
    const url = props.src;
    visible.set(url, (visible.get(url) ?? 0) + 1);
    recent.delete(url);
    recent.add(url);
    if (recent.size > 120) releaseUnusedImages();
    return () => {
      const count = visible.get(url) ?? 1;
      if (count === 1) visible.delete(url);
      else visible.set(url, count - 1);
    };
  });
  return <Image src={props.src} onError={props.failed} style={{ width: "100%", aspectRatio: 16 / 9 }} />;
}
