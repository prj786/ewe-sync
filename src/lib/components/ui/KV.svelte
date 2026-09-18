<script>
  // Key–value, as a row in a list well: the key in text-muted, the value in
  // text-primary on the right (KeyValue card). Technical values are mono.
  let { k = "", v = "", mono = false, children } = $props();
</script>

<div class="ewe-row kv-row">
  <div class="ewe-row__text"><div class="ewe-row__title kv-key">{k}</div></div>
  <div class="ewe-row__trail kv-value {mono ? 'ewe-kv__mono' : ''}" title={children ? undefined : v}>
    {#if children}{@render children()}{:else}{v}{/if}
  </div>
</div>
