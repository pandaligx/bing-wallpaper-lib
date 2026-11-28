import { Text, View, windowSize } from "nsh";
import { Icon } from "./Icon";
import { Select, Switch } from "nsh/base";

export function FieldLabel(props: { label: string }) {
  return (
    <Text
      class="text-xs text-foreground"
      style={{ width: windowSize().width < 640 ? 130 : 188, flexShrink: 1 }}
    >
      {props.label}
    </Text>
  );
}

export type Choice = { title: string; value: string; disabled?: boolean };
export function ChoiceField(props: {
  id?: string;
  label: string;
  hideLabel?: boolean;
  items: Choice[];
  value: string;
  choose: (v: string) => void;
}) {
  return (
    <View class="min-h-7 flex-row flex-wrap items-center gap-2">
      <If when={!props.hideLabel}>
        <FieldLabel label={props.label} />
      </If>
      <Select
        testID={props.id}
        items={props.items}
        value={props.value}
        accessibilityLabel={props.label}
        onChange={({ value }) => {
          if (value !== null) props.choose(value);
        }}
        popupClass="rounded border border-border bg-card p-1"
        itemClass="rounded px-2 py-1 hover:bg-accent"
        trigger={
          <View class="h-7 flex-row items-center gap-2 rounded border border-input px-2 hover:bg-accent">
            <Text class="text-xs text-foreground">
              {props.items.find((i) => i.value === props.value)?.title ?? props.value}
            </Text>
            <Icon name="chevron-down" ink="muted-foreground" />
          </View>
        }
        renderItem={(item) => <Text class="text-xs text-foreground">{item.title}</Text>}
      />
    </View>
  );
}
export function Toggle(props: {
  id?: string;
  label: string;
  value: boolean;
  disabled?: boolean;
  change: (v: boolean) => void;
}) {
  return (
    <View class="min-h-7 flex-row items-center gap-2">
      <FieldLabel label={props.label} />
      <Switch
        testID={props.id}
        checked={props.value}
        disabled={props.disabled}
        accessibilityLabel={props.label}
        onChange={({ checked }) => props.change(checked)}
        trackClass="h-4 w-8 rounded-full bg-muted checked:bg-primary"
        thumbClass="h-4 w-4 rounded-full bg-foreground checked:translate-x-4 checked:bg-primary-foreground"
      />
    </View>
  );
}
