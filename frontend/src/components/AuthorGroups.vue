<!-- Renders items as one list, or as one list per author when `grouped`. -->
<template>
  <template v-if="grouped">
    <div v-for="group in groups" :key="group.author" class="author-group">
      <h3 class="author-heading">{{ group.author }} <span class="author-count">{{ group.items.length }}</span></h3>
      <div :class="listClass">
        <template v-for="item in group.items" :key="item.id">
          <slot :item="item" :grouped="true" />
        </template>
      </div>
    </div>
  </template>
  <div v-else :class="listClass">
    <template v-for="item in items" :key="item.id">
      <slot :item="item" :grouped="false" />
    </template>
  </div>
</template>

<script setup lang="ts" generic="T extends { id: number; author: string }">
import { computed } from 'vue';
import { groupByAuthor } from '../utils/format';

const props = defineProps<{
  items: T[];
  grouped: boolean;
  /** Class of the element that wraps each list, e.g. `books-grid`. */
  listClass: string;
}>();

const groups = computed(() => groupByAuthor(props.items));
</script>
