<script setup lang="ts">
import type { DockerContainer } from "@/lib/tauri";

withDefaults(
  defineProps<{
    container?: DockerContainer;
    pending?: boolean;
    /**
     * Show an always-visible pin toggle (used in the expanded view, where
     * pinning is the primary action — it curates the collapsed view). When
     * set, the pin is rendered persistently instead of inside the hover gutter.
     */
    persistentPin?: boolean;
  }>(),
  { pending: false, persistentPin: false },
);

const emit = defineEmits<{
  (e: "start",     id: string): void;
  (e: "stop",      id: string): void;
  (e: "restart",   id: string): void;
  (e: "logs",      id: string): void;
  (e: "exec",      id: string): void;
  (e: "open-port", container: DockerContainer): void;
  (e: "toggle-pin", name: string): void;
}>();

function dotClass(c: DockerContainer): string {
  if (c.restartLoop) return "dot dot-red dot-pulse";
  switch (c.state) {
    case "running":
      switch (c.health) {
        case "healthy":   return "dot dot-green";
        case "unhealthy": return "dot dot-red dot-pulse";
        case "starting":  return "dot dot-amber";
        default:          return "dot dot-green";
      }
    case "restarting": return "dot dot-red dot-pulse";
    case "paused":     return "dot dot-amber";
    case "created":    return "dot dot-amber";
    case "exited":
    case "dead":
    default:
      return "dot dot-grey dot-hollow";
  }
}

function isRunning(c: DockerContainer): boolean {
  return c.state === "running";
}

function hasPorts(c: DockerContainer): boolean {
  return c.ports.some((p) => p.hostPort != null);
}
</script>

<template>
  <!--
    Two-stage hover gutter (pure CSS, no JS required):
      Stage 1 — widget hovered: .action-gutter width 0→84px.
        Driven by a global rule in DockerWidgetApp.vue:
          .widget-root:hover .action-gutter { width: 84px }
        Row height stays fixed (26px); only width transitions.
      Stage 2 — this row hovered: icons fade in (opacity 0→1).
        Driven by .container-row:hover .action-icons { opacity: 1 }
  -->
  <div v-if="container" class="container-row" :class="{ pending }">
    <span :class="dotClass(container)"></span>

    <div class="name-block">
      <span class="name" :title="`${container.name} · ${container.status}`">{{ container.name }}</span>
    </div>

    <span v-if="pending" class="spinner" aria-label="pending">&#x21BB;</span>

    <!-- Action gutter: width reserved on widget hover; icons revealed on row hover -->
    <div v-if="!pending" class="action-gutter">
      <div class="action-icons">
        <button
          v-if="!isRunning(container)"
          class="ctrl-btn start-btn"
          title="Start"
          @click="emit('start', container.id)"
        >&#x25B6;</button>
        <button
          v-if="isRunning(container)"
          class="ctrl-btn stop-btn"
          title="Stop"
          @click="emit('stop', container.id)"
        >&#x25A0;</button>
        <button
          class="ctrl-btn"
          title="Restart"
          @click="emit('restart', container.id)"
        >&#x21BA;</button>
        <button
          class="ctrl-btn"
          title="Logs"
          @click="emit('logs', container.id)"
        >&#x1F4C4;</button>
        <button
          class="ctrl-btn"
          title="Open shell"
          @click="emit('exec', container.id)"
        >&#x24C4;</button>
        <button
          v-if="hasPorts(container)"
          class="ctrl-btn"
          title="Open port"
          @click="emit('open-port', container)"
        >&#x1F310;</button>
        <button
          v-if="!persistentPin"
          class="ctrl-btn pin-btn"
          :class="{ pinned: container.pinned }"
          :title="container.pinned ? 'Unpin' : 'Pin'"
          @click="emit('toggle-pin', container.name)"
        >&#x1F4CC;</button>
      </div>
    </div>

    <!-- Persistent pin (expanded view): always visible so containers can be
         pinned/unpinned without hovering — this is how the collapsed list is
         curated. -->
    <button
      v-if="persistentPin"
      class="ctrl-btn pin-btn pin-persistent"
      :class="{ pinned: container.pinned }"
      :title="container.pinned ? 'Unpin' : 'Pin'"
      @click="emit('toggle-pin', container.name)"
    >&#x1F4CC;</button>
  </div>
</template>

<style scoped>
.container-row {
  display: flex;
  align-items: center;
  gap: 5px;
  padding: 3px 6px;
  border-radius: 3px;
  height: 26px;           /* Fixed — hover must NOT change this */
  box-sizing: border-box;
  transition: background 0.12s;
  overflow: hidden;
}

.container-row:hover {
  background: #242424;
}

.container-row.pending {
  opacity: 0.6;
}

/* Status dots */
.dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  flex-shrink: 0;
  display: inline-block;
}

.dot-green { background: #3fb950; }
.dot-amber { background: #d29922; }
.dot-red   { background: #f85149; }

.dot-grey.dot-hollow {
  background: transparent;
  border: 1.5px solid #6e7681;
}
.dot-grey:not(.dot-hollow) { background: #6e7681; }

@keyframes pulse {
  0%, 100% { opacity: 1; }
  50%       { opacity: 0.3; }
}
.dot-pulse { animation: pulse 1.2s ease-in-out infinite; }

/* Name block: flex: 1 so it yields space to the gutter */
.name-block {
  display: flex;
  align-items: baseline;
  gap: 4px;
  flex: 1;
  min-width: 0;
  overflow: hidden;
}

.name {
  font-size: 12px;
  color: #e0e0e0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.spinner {
  font-size: 11px;
  color: #6e7681;
  animation: spin 1s linear infinite;
  display: inline-block;
  flex-shrink: 0;
}
@keyframes spin {
  from { transform: rotate(0deg); }
  to   { transform: rotate(360deg); }
}

/*
  Action gutter:
  - width 0 by default (no gutter in idle state).
  - Stage 1: .widget-root:hover expands width to 84px (global rule in DockerWidgetApp).
  - Stage 2: .container-row:hover reveals icons via opacity.
  - Row height never changes — overflow:hidden clips the fixed-width inner content.
*/
.action-gutter {
  width: 0;
  flex-shrink: 0;
  overflow: hidden;
  transition: width 0.12s ease;
  display: flex;
  align-items: center;
}

.action-icons {
  display: flex;
  align-items: center;
  gap: 2px;
  opacity: 0;
  transition: opacity 0.1s ease;
  white-space: nowrap;
  padding-left: 2px;
}

/* Hovering the row reveals its actions: open the gutter + fade icons in.
   No widget-level hover — only the row under the cursor shows controls. */
.container-row:hover .action-gutter {
  width: 84px;
}

.container-row:hover .action-icons {
  opacity: 1;
}

.ctrl-btn {
  width: 18px;
  height: 18px;
  padding: 0;
  background: #2a2a2a;
  border: 1px solid #3a3a3a;
  color: #888;
  border-radius: 3px;
  cursor: pointer;
  font-size: 9px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  transition: background 0.1s, color 0.1s, border-color 0.1s;
  line-height: 1;
  flex-shrink: 0;
}

.ctrl-btn:hover {
  background: #383838;
  color: #ddd;
}

.start-btn:hover { color: #3fb950; border-color: #3fb950; }
.stop-btn:hover  { color: #f85149; border-color: #f85149; }

.pin-btn        { color: #6e7681; }
.pin-btn.pinned { color: #d29922; border-color: #d29922; }

/* Always-visible pin in the expanded view: dim until hovered/pinned. */
.pin-persistent {
  margin-left: 4px;
  flex-shrink: 0;
  opacity: 0.5;
  transition: opacity 0.1s, background 0.1s, color 0.1s, border-color 0.1s;
}
.container-row:hover .pin-persistent { opacity: 1; }
.pin-persistent.pinned { opacity: 1; }
</style>
