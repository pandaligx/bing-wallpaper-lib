import { computed, List, signal, View } from "nsh";
import { Text } from "./Text";
import { DatePicker, Dialog, Progress } from "nsh/base";
import { enqueue, jobs, removeJobs, pauseJobs, month, stopJob, photos, saved, transfers } from "../library";
import type { Job, Photo } from "../library";
import { t, uiFontSize } from "../preferences";
import { Action } from "./Action";

const range = signal<[string | null, string | null]>([null, null]);
function Task(props: { job: Job }) {
  const job = () => props.job;
  const transfer = () => transfers().find((tr) => tr.date === job().photo.date && tr.state === "active");
  return (
    <View class="gap-1 rounded-lg bg-card px-3 py-2">
      <View class="flex-row items-center gap-2">
        <View class="flex-1 gap-1">
          <Text class="text-xs font-semibold text-foreground">
            {job().photo.headline || job().photo.date}
          </Text>
          <Text class="text-xs text-muted-foreground">
            {job().photo.date} · {t(job().state)}
          </Text>
        </View>
        <View class="flex-row items-center gap-1">
          <If when={job().state === "Waiting" || job().state === "Active"}>
            <Action
              icon="pause"
              id={"pause-" + job().photo.date}
              label={t("Pause")}
              press={() => void stopJob(job(), false)}
            />
          </If>
          <If when={job().state === "Waiting" || job().state === "Active" || job().state === "Pause"}>
            <Action
              icon="x"
              id={"cancel-" + job().photo.date}
              label={t("Cancel")}
              press={() => void stopJob(job(), true)}
            />
          </If>
          <If when={job().state === "Failed" || job().state === "Pause" || job().state === "Cancelled"}>
            <Action
              icon={job().state === "Pause" ? "play" : "rotate-ccw"}
              id={"retry-" + job().photo.date}
              label={t(job().state === "Pause" ? "Resume" : "Retry")}
              press={() => enqueue([job().photo], job().apply)}
            />
          </If>
          <If when={!["Active", "Waiting"].includes(job().state)}>
            <Action
              id={"remove-" + job().photo.date}
              icon="trash-2"
              label={t("Remove task")}
              press={() => removeJobs([job().photo.date])}
            />
          </If>
        </View>
      </View>
      <If when={job().path || job().error}>
        <Text class={job().error ? "text-xs text-destructive" : "text-xs text-muted-foreground"}>
          {job().error || job().path}
        </Text>
      </If>
      <If when={transfer()}>
        {(tr) => (
          <View class="flex-row items-center gap-2">
            <Progress
              class="flex-1"
              value={(100 * tr().received) / (tr().total || 1)}
              indeterminate={!tr().total}
              accessibilityLabel={t("Download progress")}
              trackClass="h-2 rounded bg-muted"
              indicatorClass="h-2 rounded bg-primary"
            />
            <Text class="text-xs">
              {(tr().received / 1048576).toFixed(1)} / {tr().total ? (tr().total! / 1048576).toFixed(1) : "?"}{" "}
              MiB · {Math.round(tr().speed / 1024)} KiB/s
            </Text>
          </View>
        )}
      </If>
    </View>
  );
}
export function DownloadCenter() {
  const scope = signal<"all" | "month" | "favorites" | "range" | null>(null);
  const removing = signal<string[]>([]);
  const min = () => photos()[photos().length - 1]?.date ?? "";
  const max = () => photos()[0]?.date ?? "";
  const currentMonth = () => month() || max().slice(0, 7);
  const span = () => range();
  const valid = () =>
    !!(span()[0] && span()[1] && span()[0]! >= min() && span()[1]! <= max() && span()[0]! <= span()[1]!);
  const selection = computed<Photo[]>(() => {
    switch (scope()) {
      case "all":
        return photos();
      case "month":
        return photos().filter((p) => p.date.startsWith(currentMonth()));
      case "favorites": {
        const dates = new Set(saved());
        return photos().filter((p) => dates.has(p.date));
      }
      case "range":
        return valid() ? photos().filter((p) => p.date >= span()[0]! && p.date <= span()[1]!) : [];
      default:
        return [];
    }
  });
  const selectedTasks = computed(() => {
    const dates = new Set(selection().map((p) => p.date));
    return jobs().filter((j) => dates.has(j.photo.date));
  });
  const removable = computed(() =>
    selectedTasks()
      .filter((j) => j.state !== "Active" && j.state !== "Waiting")
      .map((j) => j.photo.date),
  );
  return (
    <View class="flex-1 gap-2">
      <View class="flex-row flex-wrap gap-2">
        <Action
          icon="archive"
          id="batch-all"
          label={t("All history")}
          disabled={!max()}
          primary={scope() === "all"}
          row
          press={() => scope.set("all")}
        />
        <Action
          icon="calendar-days"
          id="batch-month"
          label={t("Current month") + " · " + currentMonth()}
          disabled={!max()}
          primary={scope() === "month"}
          row
          press={() => scope.set("month")}
        />
        <Action
          icon="heart"
          id="batch-favorites"
          label={t("Favorites") + " · " + saved().length}
          disabled={!saved().length}
          row
          primary={scope() === "favorites"}
          press={() => scope.set("favorites")}
        />
      </View>
      <View class="flex-row flex-wrap items-center gap-2">
        <DatePicker
          testID="range-picker"
          range
          popupClass="rounded-xl border border-border bg-popover p-3 shadow-lg"
          calendarClass="gap-1"
          headerClass="flex-row items-center justify-between pb-1"
          itemClass="min-w-7 min-h-7 items-center justify-center rounded text-foreground hover:bg-accent selected:bg-primary selected:text-primary-foreground data-range-middle:bg-accent disabled:opacity-35 focus-visible:ring-2 focus-visible:ring-ring"
          itemStyle={{ fontSize: uiFontSize() }}
          value={range()}
          disabled={!max()}
          disabledMatcher={(date) => date < min() || date > max()}
          yearRange={[Number(min().slice(0, 4)), Number(max().slice(0, 4)) + 1]}
          accessibilityLabel={t("Select a date range")}
          onChange={({ value }) => {
            range.set(Array.isArray(value) ? value : [null, null]);
            scope.set("range");
          }}
          trigger={
            <Text class="text-xs rounded-lg border border-border p-2 text-foreground">
              {span()[0] ? span()[0] + " — " + (span()[1] || "…") : t("Select a date range")}
            </Text>
          }
        />
        <Action
          row
          primary
          icon="play"
          id="start-batch"
          disabled={!selection().length}
          label={t("Start download") + (selection().length ? " · " + selection().length : "")}
          press={() => enqueue(selection())}
        />
        <Action
          icon="pause"
          id="pause-batch"
          label={t("Pause")}
          disabled={!selectedTasks().some((j) => j.state === "Waiting" || j.state === "Active")}
          press={() => pauseJobs(selectedTasks())}
        />
        <Action
          icon="trash-2"
          id="remove-batch"
          label={t("Remove task")}
          disabled={!removable().length}
          press={() => removing.set(removable())}
        />
      </View>
      <Text class="text-xs text-muted-foreground">
        {jobs().length
          ? ["Waiting", "Active", "Done", "Skipped", "Failed", "Pause", "Cancelled"]
              .map((state) => [state, jobs().filter((j) => j.state === state).length] as const)
              .filter(([, count]) => count > 0)
              .map(([state, count]) => t(state) + " " + count)
              .join("  ·  ")
          : t("Empty")}
      </Text>
      <List
        smoothWheel={false}
        each={jobs()}
        keyExtractor={(j) => j.photo.date}
        estimatedItemSize={75}
        class="flex-1"
        contentStyle={{ gap: 4 }}
      >
        {(j) => <Task job={j()} />}
      </List>
      <If when={removing().length > 0}>
        <Dialog
          open
          class="m-auto rounded-xl bg-card p-4"
          onOpenChange={({ open }) => {
            if (!open) removing.set([]);
          }}
        >
          <View class="gap-3">
            <Text class="text-sm font-semibold">
              {t("Remove task")} · {removing().length}
            </Text>
            <View class="flex-row gap-2">
              <Action label={t("Cancel")} press={() => removing.set([])} />
              <Action
                id="confirm-remove-tasks"
                primary
                label={t("Delete")}
                press={() => {
                  removeJobs(removing());
                  removing.set([]);
                }}
              />
            </View>
          </View>
        </Dialog>
      </If>
    </View>
  );
}
