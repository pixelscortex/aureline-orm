<script lang="ts">
    import "./layout.css";
    import favicon from "$lib/assets/favicon.svg";
    import Navbar from "$lib/components/navbar.svelte";

    let { children } = $props();
</script>

<svelte:head><link rel="icon" href={favicon} /></svelte:head>
<div
    class="bg-[url('texture/noise.jpg')] bg-size-[256px_256px] fixed h-screen w-full opacity-3 pointer-events-none"
></div>

<div class="min-h-screen">
    <Navbar />

    {@render children()}
</div>
