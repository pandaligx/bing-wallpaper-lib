import { Text } from "./Text";
import { View } from "nsh";
import type { ViewRef } from "nsh";
import { Button, Tooltip } from "nsh/base";
import { Icon, type IconName } from "./Icon";

interface ActionProps {
  id?: string;
  count?: number;
  small?: boolean;
  onGlass?: boolean;
  label: string;
  shortLabel?: string;
  press: () => void;
  icon?: IconName;
  row?: boolean;
  primary?: boolean;
  filled?: boolean;
  tint?: boolean;
  disabled?: boolean;
  buttonRef?: (button: ViewRef) => void;
}

export function Action(props: ActionProps) {
  const hasText = () => !props.icon || !!props.row;
  const base =
    "flex-row items-center rounded-lg transition-colors duration-150 focus-visible:ring-2 focus-visible:ring-ring active:opacity-70 disabled:opacity-40 ";
  return (
    <Button
      ref={props.buttonRef}
      testID={props.id}
      accessibilityLabel={props.label}
      selected={props.row ? props.primary : props.filled}
      disabled={props.disabled}
      onPress={props.press}
      tooltip={
        !hasText() ? (
          <Tooltip class="rounded border border-border bg-popover px-2 py-1">
            <Text class="text-xs text-popover-foreground">{props.label}</Text>
          </Tooltip>
        ) : undefined
      }
      class={
        base +
        (props.row
          ? "h-7 min-w-0 gap-2 px-2 "
          : hasText()
            ? "min-h-7 justify-center px-2 "
            : "h-7 w-7 justify-center ") +
        (props.primary
          ? props.row
            ? "bg-sidebar-accent"
            : "bg-primary"
          : props.row || !hasText()
            ? "hover:bg-accent"
            : "border border-border bg-card hover:bg-accent")
      }
    >
      {props.icon ? (
        <Icon
          name={props.icon}
          size={props.small ? 14 : 16}
          filled={props.filled}
          ink={
            props.onGlass
              ? "white"
              : props.primary && !props.row
                ? "primary-foreground"
                : props.filled || props.tint
                  ? "primary"
                  : "foreground"
          }
        />
      ) : null}
      {hasText() ? (
        <Text
          class={
            (props.primary && !props.row ? "text-primary-foreground" : "text-foreground") +
            " min-w-0 shrink text-xs line-clamp-1"
          }
        >
          {props.shortLabel ?? props.label}
        </Text>
      ) : null}
      {props.count !== undefined ? (
        <View class="ml-auto min-w-5 items-center rounded-full bg-muted px-1">
          <Text class="text-xs text-muted-foreground">{props.count}</Text>
        </View>
      ) : null}
    </Button>
  );
}
