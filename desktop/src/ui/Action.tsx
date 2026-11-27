import { Text } from "nsh";
import type { ViewRef } from "nsh";
import { Button } from "nsh/base";
interface ActionProps {
  id?: string;
  label: string;
  press: () => void;
  primary?: boolean;
  disabled?: boolean;
  buttonRef?: (button: ViewRef) => void;
}
export function Action(props: ActionProps) {
  return (
    <Button
      ref={props.buttonRef}
      testID={props.id}
      accessibilityLabel={props.label}
      disabled={props.disabled}
      onPress={props.press}
      class={
        props.primary
          ? "rounded-lg bg-primary px-4 py-2 hover:opacity-90 focus-visible:ring-2 focus-visible:ring-ring active:opacity-70 disabled:opacity-40"
          : "rounded-lg border border-border bg-card px-4 py-2 hover:bg-accent focus-visible:ring-2 focus-visible:ring-ring active:opacity-70 disabled:opacity-40"
      }
    >
      <Text class={props.primary ? "text-primary-foreground" : "text-foreground"}>{props.label}</Text>
    </Button>
  );
}
