<script setup lang="ts">
// Outil « Matériel » (600 × 248) : CPU, RAM, GPU, ping — détail, courbe d'une minute,
// valeur. Monochrome sauf tension (≥ 70 orange, ≥ 90 rouge ; ping hors ligne = rouge).
import { computed } from "vue";
import { PhCpu, PhGraphicsCard, PhMemory, PhWifiHigh } from "@phosphor-icons/vue";
import Sparkline from "../../ui/Sparkline.vue";
import { hw, pingScale, tone } from "./store";
import { t } from "../../notch/i18n";

const gb = (b: number) => (b / 1024 ** 3).toFixed(1);

const rows = computed(() => {
  const s = hw.last;
  const ram = s && s.memTotal ? (s.memUsed / s.memTotal) * 100 : null;
  const ping = s?.ping ?? null;
  return [
    { key: "cpu", icon: PhCpu, label: "CPU", detail: s?.cpuName ?? "", series: hw.cpu, max: 100, value: s ? `${Math.round(s.cpu)} %` : "–", color: tone(s?.cpu) },
    {
      key: "ram",
      icon: PhMemory,
      label: "RAM",
      detail: s ? `${gb(s.memUsed)} / ${gb(s.memTotal)} Go` : "",
      series: hw.ram,
      max: 100,
      value: ram === null ? "–" : `${Math.round(ram)} %`,
      color: tone(ram),
    },
    {
      key: "gpu",
      icon: PhGraphicsCard,
      label: "GPU",
      detail: [s?.gpuName, s?.vramTotal ? `${gb(s.vramUsed ?? 0)} / ${gb(s.vramTotal)} Go` : null].filter(Boolean).join(" · "),
      series: hw.gpu,
      max: 100,
      value: s?.gpu == null ? "–" : `${Math.round(s.gpu)} %`,
      color: tone(s?.gpu),
    },
    {
      key: "ping",
      icon: PhWifiHigh,
      label: "Ping",
      detail: "1.1.1.1",
      series: hw.ping,
      max: pingScale.value,
      value: hw.offline ? t("system.offline") : ping === null ? "–" : `${ping} ms`,
      color: hw.offline ? "var(--sys-red)" : tone(ping, "ping"),
    },
  ];
});
</script>

<template>
  <div class="h-full px-4">
    <div class="divide-y divide-separator rounded-[14px] bg-fill-1 px-3 py-1">
      <div v-for="(r, i) in rows" :key="r.key" class="flex h-[38px] animate-rise items-center gap-2.5" :style="{ animationDelay: `${i * 40}ms` }">
        <component :is="r.icon" :size="15" weight="bold" class="shrink-0 text-label-2" />
        <span class="type-footnote w-9 font-medium">{{ r.label }}</span>
        <span class="type-caption w-[190px] truncate text-label-3">{{ r.detail }}</span>
        <Sparkline :values="r.series" :max="r.max" :tick="hw.tick" :color="r.color" />
        <span class="type-footnote w-[58px] text-right font-semibold tabular-nums transition-[color] duration-300 ease-out" :style="{ color: r.color }">{{ r.value }}</span>
      </div>
    </div>
  </div>
</template>
