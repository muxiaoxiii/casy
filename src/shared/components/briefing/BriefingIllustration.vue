<script setup lang="ts">
defineProps<{ variant: string }>()
</script>

<template>
  <svg viewBox="0 0 320 200" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
    <g v-if="variant === 'magic-prophet'" stroke="currentColor">
      <ellipse cx="160" cy="96" rx="85" ry="82" stroke-width="0.7" />
      <ellipse cx="160" cy="96" rx="79" ry="76" stroke-dasharray="1 4" />
      <path d="M85 168h150M103 173h114M115 158c-14-32-13-74 0-114l24 18q21-9 42 0l24-18c14 43 12 83-1 114l-44 13z" stroke-width="2" />
      <path d="M116 55l22 25M204 55l-22 25M117 109q-13 28 11 47M203 109q13 28-11 47M145 120l15 15 15-15M150 139l10 9 10-9M148 153l12 9 12-9M138 165v10m7-10v10m30-10v10m7-10v10" />
      <circle cx="137" cy="96" r="23" stroke-width="2" /><circle cx="183" cy="96" r="23" stroke-width="2" />
      <circle cx="137" cy="96" r="16" /><circle cx="183" cy="96" r="16" />
      <ellipse cx="137" cy="96" rx="5" ry="10" fill="currentColor" /><ellipse cx="183" cy="96" rx="5" ry="10" fill="currentColor" />
      <path d="M154 113l6 12 6-12z" fill="currentColor" />
      <path v-for="offset in [0, 7, 14, 21]" :key="offset" :d="`M${117 + offset / 3} ${120 + offset}l8 9M${203 - offset / 3} ${120 + offset}l-8 9`" />
      <path d="M52 73v18m-9-9h18M256 108v14m-7-7h14M228 32l3 7 7 3-7 3-3 7-3-7-7-3 7-3zM78 133l2 5 5 2-5 2-2 5-2-5-5-2 5-2z" />
      <path d="M25 181h270M32 185h256" stroke-width="0.6" />
    </g>
    <g v-else-if="variant === 'herbarium'" stroke="currentColor" stroke-width="1.3">
      <path d="M144 185Q166 112 183 24M158 132q-40-18-54-65 43 11 54 65ZM170 94q36-7 56-40-40-1-56 40ZM178 59q-29-13-33-42 29 8 33 42ZM150 161q35 1 62-26-43-12-62 26Z" />
      <path d="M112 78l42 47m25-38 37-24m-62-33 22 23m-18 104 43-17M126 180h71" stroke-width="0.6" />
      <path d="M76 39h23M87 28v23M241 150h14m-7-7v14" stroke-width="0.6" />
      <rect x="135" y="158" width="37" height="13" fill="currentColor" opacity="0.16" transform="rotate(-16 135 158)" />
    </g>
    <g v-else-if="variant === 'lunar-log'" stroke="currentColor">
      <circle cx="160" cy="98" r="67" fill="currentColor" fill-opacity="0.08" />
      <path d="M180 35c-45 36-45 91 0 126" stroke-width="0.6" />
      <ellipse cx="160" cy="98" rx="132" ry="32" transform="rotate(-23 160 98)" stroke-width="0.6" />
      <circle cx="135" cy="76" r="13" /><circle cx="177" cy="126" r="17" /><circle cx="185" cy="65" r="7" /><circle cx="128" cy="116" r="6" />
      <circle cx="135" cy="76" r="9" stroke-dasharray="1 3" /><circle cx="177" cy="126" r="12" stroke-dasharray="2 3" />
      <path d="M41 43h14m-7-7v14M255 150h18m-9-9v18M156 14v9m0 151v9M75 98h9m153 0h9" />
      <circle cx="274" cy="53" r="3" fill="currentColor" /><circle cx="76" cy="155" r="1" fill="currentColor" />
    </g>
    <g v-else-if="variant === 'airmail'" stroke="currentColor">
      <path d="M178 24h106v139H178z" stroke-width="6" stroke-dasharray="3 5" />
      <path d="M188 34h86v119h-86zM188 128l29-41 18 22 13-17 26 36M197 139h67" />
      <circle cx="249" cy="63" r="13" />
      <circle cx="131" cy="115" r="48" /><circle cx="131" cy="115" r="41" stroke-dasharray="2 3" />
      <path d="M92 105h78m-78 21h78M105 116h53M29 91q20-10 40 0m-40 12q20-10 40 0m-40 12q20-10 40 0m-40 12q20-10 40 0" />
    </g>
  </svg>
</template>
