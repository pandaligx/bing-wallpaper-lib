import { Text as NativeText } from "nsh";
import type { TextProps } from "nsh";
import { uiFontSize } from "../preferences";

export function Text(props: TextProps) {
  const size = () =>
    uiFontSize() +
    (/\btext-base\b/.test(String(props.class)) ? 4 : /\btext-sm\b/.test(String(props.class)) ? 2 : 0);
  return (
    <NativeText {...props} style={[props.style, { fontSize: size() }]}>
      {props.children}
    </NativeText>
  );
}
