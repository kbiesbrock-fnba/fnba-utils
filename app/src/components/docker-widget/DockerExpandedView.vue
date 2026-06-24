<script setup lang="ts">
import { computed } from "vue";
import type { DockerContainer } from "@/lib/tauri";
import DockerContainerRow from "./DockerContainerRow.vue";

const props = defineProps<{
  containers: DockerContainer[];
  pending: Set<string>;
}>();

const emit = defineEmits<{
  (e: "start",     id: string): void;
  (e: "stop",      id: string): void;
  (e: "restart",   id: string): void;
  (e: "logs",      id: string): void;
  (e: "exec",      id: string): void;
  (e: "open-port", container: DockerContainer): void;
  (e: "toggle-pin", name: string): void;
}>();

interface Group {
  key: string;
  label: string;
  containers: DockerContainer[];
  hasRunning: boolean;
}

function sortContainersInGroup(cs: DockerContainer[]): DockerContainer[] {
  // Running first, then non-running, stable within each tier.
  const running    = cs.filter((c) => c.state === "running");
  const nonRunning = cs.filter((c) => c.state !== "running");
  return [...running, ...nonRunning];
}

const groups = computed<Group[]>(() => {
  // Build map: composeProject → containers
  const map = new Map<string, DockerContainer[]>();
  for (const c of props.containers) {
    const key = c.composeProject ?? "__standalone__";
    if (!map.has(key)) map.set(key, []);
    map.get(key)!.push(c);
  }

  const raw: Group[] = Array.from(map.entries()).map(([key, cs]) => ({
    key,
    label: key === "__standalone__" ? "Standalone" : key,
    containers: sortContainersInGroup(cs),
    hasRunning: cs.some((c) => c.state === "running"),
  }));

  // Groups with running containers first, then fully-stopped groups.
  // Within each tier, alphabetical by label; standalone goes last.
  raw.sort((a, b) => {
    const aStandalone = a.key === "__standalone__";
    const bStandalone = b.key === "__standalone__";
    if (aStandalone !== bStandalone) return aStandalone ? 1 : -1;
    if (a.hasRunning !== b.hasRunning) return a.hasRunning ? -1 : 1;
    return a.label.localeCompare(b.label);
  });

  return raw;
});

function runningCount(g: Group): number {
  return g.containers.filter((c) => c.state === "running").length;
}
</script>

<template>
  <!--
    Expanded view: all containers grouped by compose project.
    No per-group collapse/disclosure — every group is always fully shown.
    Groups with running containers come before fully-stopped groups.
    The scrollable region has a max-height so long lists stay inside the widget.
  -->
  <div class="expanded-view">
    <div v-for="group in groups" :key="group.key" class="group-block">
      <!-- Group header: small uppercase label + running/total count -->
      <div class="group-header">
        <span class="group-rail"></span>
        <span class="group-name">{{ group.label }}</span>
        <span class="group-count" :class="{ 'count-partial': runningCount(group) < group.containers.length }">
          {{ runningCount(group) }}<span class="count-sep">/</span>{{ group.containers.length }}
        </span>
      </div>
      <!-- Group rows: indented with left accent rail -->
      <div class="group-rows">
        <DockerContainerRow
          v-for="c in group.containers"
          :key="c.id"
          :container="c"
          :pending="props.pending.has(c.id)"
          :persistent-pin="true"
          @start="emit('start', $event)"
          @stop="emit('stop', $event)"
          @restart="emit('restart', $event)"
          @logs="emit('logs', $event)"
          @exec="emit('exec', $event)"
          @open-port="emit('open-port', $event)"
          @toggle-pin="emit('toggle-pin', $event)"
        />
      </div>
    </div>
    <div v-if="groups.length === 0" class="empty-msg">No containers</div>
  </div>
</template>

<style scoped>
.expanded-view {
  overflow-y: auto;
  /*
    The window resizes to fit content up to the monitor height. The cap below
    is a MONITOR-based pixel constant set from JS (syncSizeToContent), NOT a
    viewport unit — using 100vh here would feed back into the measured content
    height and stop the window from ever growing. Fallback `none` = no clamp
    (natural height) until JS sets the variable on first resize.
  */
  max-height: var(--docker-list-max, none);
  padding: 2px 4px 6px;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

/* === Group block: header + rows visually bracketed together === */
.group-block {
  border-radius: 4px;
  overflow: hidden;
  /* Faint group background tint — distinguishes members from inter-group space */
  background: rgba(255, 255, 255, 0.025);
  border: 1px solid rgba(255, 255, 255, 0.04);
}

/* Group header */
.group-header {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 4px 6px 3px 0;
  background: rgba(255, 255, 255, 0.03);
  border-bottom: 1px solid rgba(255, 255, 255, 0.04);
  user-select: none;
  cursor: default;
}

/* Left accent rail on the header (2px coloured bar) */
.group-rail {
  width: 2px;
  align-self: stretch;
  background: #4a4a4a;
  flex-shrink: 0;
  border-radius: 1px 0 0 1px;
}

.group-name {
  flex: 1;
  font-size: 10px;
  font-weight: 600;
  color: #8b949e;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.group-count {
  font-size: 10px;
  font-variant-numeric: tabular-nums;
  color: #3fb950;   /* green when all running */
  flex-shrink: 0;
  padding-right: 4px;
}

/* Partial = not all running → amber */
.group-count.count-partial {
  color: #d29922;
}

.count-sep {
  color: #4a4a4a;
}

/* Group rows: left indent + continuous left rail via border-left */
.group-rows {
  border-left: 2px solid #3a3a3a;
  margin-left: 0;
  padding-left: 4px;
}

/* Empty state */
.empty-msg {
  text-align: center;
  color: #6e7681;
  font-size: 11px;
  padding: 16px 0;
}
</style>
